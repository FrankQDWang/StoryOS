//! The stream sink of one exchange. It commits Model Stream Events in order, and a contended
//! commit keeps them for the next commit in place of a stop.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Duration;

use storyos_core::NativeStreamItem;

use crate::model_gateway_ports::{
    CommittedCancellation, ContractFaultObserver, ContractFaultPoint, DispatchClaim, GatewayClock,
    ModelDispatchStore, ModelStreamSink, ResponseReference, StreamControl, StreamStop,
};
use crate::{ClaimedAgentRun, CompleteAgentRunError, DiagnosticField, SqlState};

/// The transactions of one commit before the sink defers its events to the next commit.
const COMMIT_ATTEMPTS: u32 = 3;
/// The first retry delay. Each later retry doubles it, and a jitter adds less than one more.
const RETRY_DELAY: Duration = Duration::from_millis(/*millis*/ 20);
/// The consecutive deferred commits that the stream continues through. The next one stops it.
const DEFERRED_COMMITS: u64 = 4;

pub(crate) struct GatewaySink<'a, S, O, C> {
    store: &'a S,
    observer: &'a O,
    clock: &'a C,
    claim: &'a ClaimedAgentRun,
    dispatch: &'a DispatchClaim,
    cancellation: Option<CommittedCancellation>,
    /// The events and the reference that are not durable yet, in stream order.
    pending: Vec<NativeStreamItem>,
    pending_reference: Option<ResponseReference>,
    deferred: u64,
}

impl<'a, S: ModelDispatchStore, O: ContractFaultObserver, C: GatewayClock>
    GatewaySink<'a, S, O, C>
{
    pub(crate) fn new(
        store: &'a S,
        observer: &'a O,
        clock: &'a C,
        claim: &'a ClaimedAgentRun,
        dispatch: &'a DispatchClaim,
    ) -> Self {
        Self {
            store,
            observer,
            clock,
            claim,
            dispatch,
            cancellation: None,
            pending: Vec::new(),
            pending_reference: None,
            deferred: 0,
        }
    }

    /// Commits the deferred events after the exchange, before its observation is recorded.
    /// Returns the cancellation that stopped the stream, if any.
    pub(crate) async fn finish(mut self) -> Option<CommittedCancellation> {
        if self.pending.is_empty() && self.pending_reference.is_none() {
            return self.cancellation;
        }
        match self.commit_pending().await {
            Ok(None | Some(StreamStop::StaleFence)) => {}
            Ok(Some(StreamStop::Cancelled(cancellation))) => self.cancellation = Some(cancellation),
            Err(error) => append_failed(&error),
        }
        self.cancellation
    }

    async fn commit(
        &mut self,
        events: &[NativeStreamItem],
        reference: Option<&ResponseReference>,
    ) -> StreamControl {
        self.pending.extend_from_slice(events);
        if let Some(reference) = reference {
            self.pending_reference = Some(reference.clone());
        }
        let stop = match self.commit_pending().await {
            Ok(None) => {
                self.deferred = 0;
                self.observer
                    .reached(ContractFaultPoint::StreamCommitted)
                    .await;
                return StreamControl::Continue;
            }
            Err(error @ CompleteAgentRunError::Contended(_))
                if self.deferred < DEFERRED_COMMITS =>
            {
                self.deferred += 1;
                tracing::warn!(
                    attempt = self.deferred.diagnostic(),
                    reason = error.diagnostic(),
                    sql_state = SqlState(&error).diagnostic(),
                    "Model Stream Event append deferred"
                );
                return StreamControl::Continue;
            }
            Ok(Some(stop)) => Some(stop),
            Err(error) => {
                append_failed(&error);
                None
            }
        };
        if let Some(StreamStop::Cancelled(cancellation)) = stop {
            self.cancellation = Some(cancellation);
        }
        // A stopped exchange commits nothing more.
        self.pending.clear();
        self.pending_reference = None;
        StreamControl::Stop
    }

    /// Commits all pending values in one transaction. A contended transaction runs again, after a
    /// delay, until the attempts end.
    async fn commit_pending(&mut self) -> Result<Option<StreamStop>, CompleteAgentRunError> {
        let mut attempt = 1;
        loop {
            match self
                .store
                .append_model_stream_events(
                    self.claim,
                    self.dispatch,
                    &self.pending,
                    self.pending_reference.as_ref(),
                )
                .await
            {
                Ok(None) => {
                    self.pending.clear();
                    self.pending_reference = None;
                    return Ok(None);
                }
                Err(CompleteAgentRunError::Contended(_)) if attempt < COMMIT_ATTEMPTS => {
                    self.clock.wait(self.retry_delay(attempt)).await;
                    attempt += 1;
                }
                result => return result,
            }
        }
    }

    /// The jitter comes from the Run and the attempt, so that two Workers do not retry in step.
    fn retry_delay(&self, attempt: u32) -> Duration {
        let mut hasher = DefaultHasher::new();
        (&self.claim.run_id, &self.dispatch.model_attempt_id, attempt).hash(&mut hasher);
        let jitter = u32::try_from(hasher.finish() % 1024).unwrap_or_default();
        RETRY_DELAY * 2_u32.pow(attempt - 1) + RETRY_DELAY * jitter / 1024
    }
}

impl<S: ModelDispatchStore, O: ContractFaultObserver, C: GatewayClock> ModelStreamSink
    for GatewaySink<'_, S, O, C>
{
    async fn append(&mut self, events: &[NativeStreamItem]) -> StreamControl {
        self.commit(events, /*reference*/ None).await
    }

    async fn retain_reference(&mut self, reference: &ResponseReference) -> StreamControl {
        self.commit(&[], Some(reference)).await
    }
}

fn append_failed(error: &CompleteAgentRunError) {
    tracing::warn!(
        reason = error.diagnostic(),
        sql_state = SqlState(error).diagnostic(),
        "Model Stream Event append failed"
    );
}

#[cfg(test)]
#[path = "model_stream_commit_tests.rs"]
mod tests;
