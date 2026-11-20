//! The shared test support from `stream_commit_contention_tests.rs`.

use crate::PostgresProjectReader;
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    ClaimedAgentRun, DispatchClaim, ModelDispatchStore, ModelProviderAdapter, NextDispatchWork,
};

/// Claims the next Create of `claim`, as the Gateway does before the exchange.
pub(crate) async fn dispatched(
    store: &PostgresProjectReader,
    claim: &ClaimedAgentRun,
) -> DispatchClaim {
    let NextDispatchWork::Request(request) = store
        .next_dispatch_work(claim, FakeDestination::ADAPTERS)
        .await
        .unwrap()
    else {
        panic!("The claimed AgentRun has no request");
    };
    let prepared = FakeDestination.prepare(&request).await.unwrap();
    store
        .commit_dispatch_claim(claim, &request, &prepared.projection)
        .await
        .unwrap()
        .unwrap()
}
