//! The Model Gateway: the only owner of the dispatch order for model destination requests.

use storyos_core::NativeStreamItem;

use crate::model_gateway_ports::{
    CommittedCancellation, ContractFaultObserver, ContractFaultPoint, DestinationRequest,
    DispatchClaim, DispatchRecord, ModelDispatchStore, ModelProviderAdapter, ModelStreamSink,
    ModelUsage, NextDispatchWork, Observation, RequestAttempt, StreamControl, StreamStop,
};
use crate::{
    ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError, DiagnosticField, DiagnosticId,
    SqlState,
};

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
            NextDispatchWork::Abort(cancellation) => cancellation.into_abort(),
        };
        if let Some(cancellation) = gateway.dispatch(request).await? {
            gateway.dispatch(cancellation.into_abort()).await?;
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
    /// Sends one request in order. Returns the cancellation that stopped its stream, if any.
    #[tracing::instrument(skip_all, fields(
        project_id = self.claim.project_scope.project_id.diagnostic(),
        run_id = DiagnosticId(&self.claim.run_id).diagnostic(),
        request_kind = RequestKind(&request).diagnostic(),
        adapter = std::any::type_name::<A>().diagnostic(),
        observation = tracing::field::Empty,
        usage = tracing::field::Empty,
        input_tokens = tracing::field::Empty,
        output_tokens = tracing::field::Empty,
    ))]
    async fn dispatch(
        &self,
        request: DestinationRequest,
    ) -> Result<Option<CommittedCancellation>, CompleteAgentRunError> {
        let Gateway {
            store,
            adapter,
            observer,
            claim,
        } = *self;
        let prepared = match adapter.prepare(&request).await {
            Ok(prepared) => prepared,
            Err(refusal) => {
                tracing::Span::current().record("observation", "pre_dispatch_refusal");
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
                    tracing::Span::current().record("observation", "work_changed");
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
            cancellation: None,
        };
        let observation = adapter.exchange(prepared.prepared, &mut sink).await;
        record_observation(&observation);
        let cancellation = sink.cancellation;
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
        Ok(cancellation)
    }
}

struct GatewaySink<'a, S, O> {
    store: &'a S,
    observer: &'a O,
    claim: &'a ClaimedAgentRun,
    dispatch: &'a DispatchClaim,
    cancellation: Option<CommittedCancellation>,
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
            Ok(Some(StreamStop::Cancelled(cancellation))) => {
                self.cancellation = Some(cancellation);
                StreamControl::Stop
            }
            Ok(Some(StreamStop::StaleFence)) => StreamControl::Stop,
            Err(error) => {
                tracing::warn!(
                    reason = error.diagnostic(),
                    sql_state = SqlState(&error).diagnostic(),
                    "Model Stream Event append failed"
                );
                StreamControl::Stop
            }
        }
    }
}

fn record_observation(observation: &Observation) {
    tracing::Span::current().record("observation", ObservationKind(observation).diagnostic());
    let Observation::Terminal(response) = observation else {
        return;
    };
    match response.usage {
        ModelUsage::Reported {
            input_tokens,
            output_tokens,
        }
        | ModelUsage::Estimated {
            input_tokens,
            output_tokens,
        } => {
            tracing::Span::current().record("input_tokens", input_tokens.diagnostic());
            tracing::Span::current().record("output_tokens", output_tokens.diagnostic());
        }
        ModelUsage::Unknown => {}
    }
    tracing::Span::current().record("usage", UsageKind(response.usage).diagnostic());
}

struct RequestKind<'a>(&'a DestinationRequest);

impl DiagnosticField for RequestKind<'_> {
    type Value<'b>
        = &'static str
    where
        Self: 'b;

    fn diagnostic(&self) -> Self::Value<'_> {
        match self.0 {
            DestinationRequest::Create(_) => "create",
            DestinationRequest::Retrieve(_) => "retrieve",
            DestinationRequest::Abort(_) => "abort",
        }
    }
}

/// The category of an observation. A rejection reason is native destination text, so it is absent.
struct ObservationKind<'a>(&'a Observation);

impl DiagnosticField for ObservationKind<'_> {
    type Value<'b>
        = &'static str
    where
        Self: 'b;

    fn diagnostic(&self) -> Self::Value<'_> {
        match self.0 {
            Observation::NotSubmitted => "not_submitted",
            Observation::Rejected { .. } => "rejected",
            Observation::Terminal(_) => "terminal",
            Observation::OutcomeUnknown { .. } => "outcome_unknown",
        }
    }
}

struct UsageKind(ModelUsage);

impl DiagnosticField for UsageKind {
    type Value<'a> = &'static str;

    fn diagnostic(&self) -> Self::Value<'_> {
        match self.0 {
            ModelUsage::Reported { .. } => "reported",
            ModelUsage::Estimated { .. } => "estimated",
            ModelUsage::Unknown => "unknown",
        }
    }
}
