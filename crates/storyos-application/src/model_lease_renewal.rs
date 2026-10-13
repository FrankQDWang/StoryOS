//! Keeps the Run Lease of a claimed AgentRun while one destination request is in progress.

use std::future::{Future, poll_fn};
use std::pin::pin;
use std::task::Poll;
use std::time::Duration;

use crate::model_gateway_ports::{GatewayClock, ModelDispatchStore};
use crate::{ClaimedAgentRun, DiagnosticField, SqlState};

/// The delay before the next renewal after a renewal failed. A failed renewal reports no delay.
const FAILED_RENEWAL_DELAY: Duration = Duration::from_secs(/*secs*/ 1);

/// Renews the lease of `claim`, then runs `work` and renews the lease again until `work`
/// completes. The first renewal commits before `work` starts, so that no renewal transaction is
/// open when the request goes out. The renewal stops when the claim has lost its lease, or when
/// the caller drops the returned future.
pub(crate) async fn renew_while<T>(
    store: &impl ModelDispatchStore,
    clock: &impl GatewayClock,
    claim: &ClaimedAgentRun,
    work: impl Future<Output = T>,
) -> T {
    let Some(delay) = renew_once(store, claim).await else {
        return work.await;
    };
    let mut work = pin!(work);
    let mut renewal = pin!(renew(store, clock, claim, delay));
    let mut renewing = true;
    poll_fn(|context| {
        if let Poll::Ready(output) = work.as_mut().poll(context) {
            return Poll::Ready(output);
        }
        if renewing && renewal.as_mut().poll(context).is_ready() {
            renewing = false;
        }
        Poll::Pending
    })
    .await
}

/// Renews after each delay. Completes when the claim has lost its lease.
async fn renew(
    store: &impl ModelDispatchStore,
    clock: &impl GatewayClock,
    claim: &ClaimedAgentRun,
    mut delay: Duration,
) {
    loop {
        clock.wait(delay).await;
        match renew_once(store, claim).await {
            Some(next) => delay = next,
            None => return,
        }
    }
}

/// Returns the delay until the next renewal, or `None` when the claim has lost its lease.
async fn renew_once(store: &impl ModelDispatchStore, claim: &ClaimedAgentRun) -> Option<Duration> {
    match store.renew_agent_run_lease(claim).await {
        Ok(delay) => delay,
        Err(error) => {
            tracing::warn!(
                reason = error.diagnostic(),
                sql_state = SqlState(&error).diagnostic(),
                "Run Lease renewal failed"
            );
            Some(FAILED_RENEWAL_DELAY)
        }
    }
}

#[cfg(test)]
#[path = "model_lease_renewal_tests.rs"]
mod tests;
