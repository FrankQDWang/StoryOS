//! The Model Gateway: the only owner of the dispatch order for model destination requests.

use storyos_core::NativeStreamItem;

use crate::model_gateway_ports::{
    AbortRequest, AbortTicket, ContractFaultObserver, ContractFaultPoint, DestinationRequest,
    DispatchClaim, DispatchRecord, ModelDispatchStore, ModelProviderAdapter, ModelStreamSink,
    NextDispatchWork, RequestAttempt, StreamControl, StreamStop,
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
    let gateway = Gateway {
        store,
        adapter,
        observer,
        claim,
    };
    loop {
        let request = match store.next_dispatch_work(claim).await? {
            NextDispatchWork::Settled(result) => return Ok(result),
            NextDispatchWork::Request(request) => request,
        };
        if let Some(ticket) = gateway.dispatch(request).await? {
            let abort = DestinationRequest::Abort(AbortRequest {
                attempt: RequestAttempt::New,
                ticket,
                response_reference: None,
            });
            gateway.dispatch(abort).await?;
        }
    }
}

struct Gateway<'a, S, A, O> {
    store: &'a S,
    adapter: &'a A,
    observer: &'a O,
    claim: &'a ClaimedAgentRun,
}

impl<S: ModelDispatchStore, A: ModelProviderAdapter, O: ContractFaultObserver>
    Gateway<'_, S, A, O>
{
    /// Sends one request in order. Returns the abort ticket that a durable cancellation issued.
    async fn dispatch(
        &self,
        request: DestinationRequest,
    ) -> Result<Option<AbortTicket>, CompleteAgentRunError> {
        let Gateway {
            store,
            adapter,
            observer,
            claim,
        } = *self;
        let prepared = match adapter.prepare(&request).await {
            Ok(prepared) => prepared,
            Err(refusal) => {
                store
                    .record(claim, DispatchRecord::Refusal(refusal))
                    .await?;
                return Ok(None);
            }
        };
        let dispatch = match request.attempt() {
            RequestAttempt::Claimed(dispatch) => dispatch.clone(),
            RequestAttempt::New => {
                let Some(dispatch) = store
                    .commit_dispatch_claim(claim, &request, &prepared.projection)
                    .await?
                else {
                    return Ok(None);
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
            ticket: None,
        };
        let observation = adapter.exchange(prepared.prepared, &mut sink).await;
        let ticket = sink.ticket;
        store
            .record(
                claim,
                DispatchRecord::Exchange {
                    claim: &dispatch,
                    request: &request,
                    observation,
                },
            )
            .await?;
        Ok(ticket)
    }
}

struct GatewaySink<'a, S, O> {
    store: &'a S,
    observer: &'a O,
    claim: &'a ClaimedAgentRun,
    dispatch: &'a DispatchClaim,
    ticket: Option<AbortTicket>,
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
            Ok(Some(StreamStop::Cancelled(ticket))) => {
                self.ticket = Some(ticket);
                StreamControl::Stop
            }
            Ok(Some(StreamStop::StaleFence)) | Err(_) => StreamControl::Stop,
        }
    }
}
