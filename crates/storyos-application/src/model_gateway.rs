//! The Model Gateway: the only owner of the dispatch order for model destination requests.

use storyos_core::NativeStreamItem;

use crate::model_gateway_ports::{
    ContractFaultObserver, ContractFaultPoint, CreateAttempt, DestinationRequest, DispatchClaim,
    DispatchRecord, ModelDispatchStore, ModelProviderAdapter, ModelStreamSink, NextDispatchWork,
    StreamControl,
};
use crate::{ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError};

/// Gets the next work, prepares, claims, exchanges with no open transaction, and records,
/// until the claimed AgentRun needs no destination I/O.
pub async fn complete_agent_run(
    store: &impl ModelDispatchStore,
    adapter: &impl ModelProviderAdapter,
    observer: &impl ContractFaultObserver,
    claim: &ClaimedAgentRun,
) -> Result<CompleteAgentRun, CompleteAgentRunError> {
    loop {
        let request = match store.next_dispatch_work(claim).await? {
            NextDispatchWork::Settled(result) => return Ok(result),
            NextDispatchWork::Request(request) => request,
        };
        let prepared = match adapter.prepare(&request).await {
            Ok(prepared) => prepared,
            Err(refusal) => {
                store
                    .record(claim, DispatchRecord::Refusal(refusal))
                    .await?;
                continue;
            }
        };
        let DestinationRequest::Create(create) = &request;
        let dispatch = match &create.attempt {
            CreateAttempt::Claimed(dispatch) => dispatch.clone(),
            CreateAttempt::New => {
                let Some(dispatch) = store
                    .commit_dispatch_claim(claim, create, &prepared.projection)
                    .await?
                else {
                    continue;
                };
                observer.reached(ContractFaultPoint::DispatchClaimed).await;
                dispatch
            }
        };
        let mut sink = GatewaySink {
            store,
            observer,
            claim,
            dispatch: &dispatch,
        };
        let observation = adapter.exchange(prepared.prepared, &mut sink).await;
        store
            .record(
                claim,
                DispatchRecord::Create {
                    claim: &dispatch,
                    request: create,
                    observation,
                },
            )
            .await?;
    }
}

struct GatewaySink<'a, S, O> {
    store: &'a S,
    observer: &'a O,
    claim: &'a ClaimedAgentRun,
    dispatch: &'a DispatchClaim,
}

impl<S: ModelDispatchStore, O: ContractFaultObserver> ModelStreamSink for GatewaySink<'_, S, O> {
    async fn append(&mut self, events: &[NativeStreamItem]) -> StreamControl {
        match self
            .store
            .append_model_stream_events(self.claim, self.dispatch, events)
            .await
        {
            Ok(None) => {
                self.observer
                    .reached(ContractFaultPoint::StreamCommitted)
                    .await;
                StreamControl::Continue
            }
            Ok(Some(_)) | Err(_) => StreamControl::Stop,
        }
    }
}
