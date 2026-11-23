//! The Model Gateway: the only owner of the dispatch order for model destination requests.

use crate::model_gateway_ports::{
    CommittedCancellation, ContractFaultObserver, ContractFaultPoint, CreatePurpose,
    DestinationRequest, DispatchRecord, GatewayClock, ModelDispatchStore, ModelProviderAdapter,
    ModelUsage, NextDispatchWork, Observation, RequestAttempt,
};
use crate::model_lease_renewal::{renew_or_drop, renew_while};
use crate::model_stream_commit::GatewaySink;
use crate::tool_gateway::{
    Embeddings, NoQuestionEmbeddings, QuestionEmbeddings, dispatch_tool_calls,
};
use crate::tool_gateway_ports::{
    NoToolDestination, SubrunWork, ToolDestinationAdapter, ToolDispatchStore,
};
use crate::{
    ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError, DiagnosticField, DiagnosticId,
    EmbeddingDispatchStore, EmbeddingProviderAdapter,
};

/// Gets the next work, prepares, claims, exchanges with no open transaction, and records,
/// until the claimed AgentRun needs no destination I/O. A process with no Tool adapter refuses
/// each ToolCall before its claim.
pub async fn complete_agent_run(
    store: &(impl ModelDispatchStore + ToolDispatchStore),
    adapter: &impl ModelProviderAdapter,
    observer: &impl ContractFaultObserver,
    clock: &impl GatewayClock,
    claim: &ClaimedAgentRun,
) -> Result<CompleteAgentRun, CompleteAgentRunError> {
    complete_agent_run_with_tools(store, adapter, &NoToolDestination, observer, clock, claim).await
}

/// The adapters of the Worker that `adapter` serves.
fn adapters_of<A: ModelProviderAdapter>(_adapter: &A) -> &'static [storyos_core::ModelAdapter] {
    A::ADAPTERS
}

/// `complete_agent_run` with the Tool destination `tools` for the ToolDispatch phase. The
/// embedding request of each question refuses with `embedding_not_configured`.
pub async fn complete_agent_run_with_tools(
    store: &(impl ModelDispatchStore + ToolDispatchStore),
    adapter: &impl ModelProviderAdapter,
    tools: &impl ToolDestinationAdapter,
    observer: &impl ContractFaultObserver,
    clock: &impl GatewayClock,
    claim: &ClaimedAgentRun,
) -> Result<CompleteAgentRun, CompleteAgentRunError> {
    run(
        store,
        adapter,
        tools,
        &NoQuestionEmbeddings,
        observer,
        clock,
        claim,
    )
    .await
}

/// `complete_agent_run_with_tools` with the embedding destination `embeddings` for the questions
/// of the `hybrid_search_manuscript` calls (ADR 0095 section 1).
pub async fn complete_agent_run_with_destinations(
    store: &(impl ModelDispatchStore + ToolDispatchStore + EmbeddingDispatchStore),
    adapter: &impl ModelProviderAdapter,
    tools: &impl ToolDestinationAdapter,
    embeddings: &impl EmbeddingProviderAdapter,
    observer: &impl ContractFaultObserver,
    clock: &impl GatewayClock,
    claim: &ClaimedAgentRun,
) -> Result<CompleteAgentRun, CompleteAgentRunError> {
    let embeddings = Embeddings {
        store,
        adapter: embeddings,
    };
    run(store, adapter, tools, &embeddings, observer, clock, claim).await
}

async fn run(
    store: &(impl ModelDispatchStore + ToolDispatchStore),
    adapter: &impl ModelProviderAdapter,
    tools: &impl ToolDestinationAdapter,
    embeddings: &impl QuestionEmbeddings,
    observer: &impl ContractFaultObserver,
    clock: &impl GatewayClock,
    claim: &ClaimedAgentRun,
) -> Result<CompleteAgentRun, CompleteAgentRunError> {
    let gateway = Gateway {
        store,
        adapter,
        observer,
        clock,
        claim,
    };
    loop {
        let request = match store
            .next_dispatch_work(claim, adapters_of(adapter))
            .await?
        {
            NextDispatchWork::Settled(result) => return Ok(result),
            NextDispatchWork::Request(request) => *request,
            NextDispatchWork::Abort(cancellation) => cancellation.into_abort(),
            NextDispatchWork::ToolCalls => {
                let subrun =
                    dispatch_tool_calls(store, tools, embeddings, observer, clock, claim).await?;
                if let Some(work) = subrun {
                    let destinations = Destinations {
                        adapter,
                        tools,
                        embeddings,
                        observer,
                        clock,
                    };
                    run_subrun(store, &destinations, claim, &work).await?;
                }
                continue;
            }
        };
        if let Some(cancellation) = gateway.dispatch(request).await? {
            gateway.dispatch(cancellation.into_abort()).await?;
        }
    }
}

/// The destinations and the observer of one `run`, which a Subrun uses again.
struct Destinations<'a, A, T, E, O, C> {
    adapter: &'a A,
    tools: &'a T,
    embeddings: &'a E,
    observer: &'a O,
    clock: &'a C,
}

/// Takes the Subrun of `work` and runs it to its end with the loop of a Run, with the renewal of
/// the lease of the parent `claim`. The parent sends no model request meanwhile. A lost lease of
/// the parent stops the Subrun, and the next claim of the parent continues it. The next ToolCall
/// work of the parent settles the call with the result of the Subrun (ADR 0103 section 3A).
async fn run_subrun(
    store: &(impl ModelDispatchStore + ToolDispatchStore),
    destinations: &Destinations<
        '_,
        impl ModelProviderAdapter,
        impl ToolDestinationAdapter,
        impl QuestionEmbeddings,
        impl ContractFaultObserver,
        impl GatewayClock,
    >,
    claim: &ClaimedAgentRun,
    work: &SubrunWork,
) -> Result<(), CompleteAgentRunError> {
    let Some(subrun) = store.take_subrun(claim, work).await? else {
        return Ok(());
    };
    let Destinations {
        adapter,
        tools,
        embeddings,
        observer,
        clock,
    } = *destinations;
    let subrun_work = Box::pin(run(
        store, adapter, tools, embeddings, observer, clock, &subrun,
    ));
    match renew_or_drop(store, clock, claim, subrun_work).await {
        Some(result) => result.map(|_settled| ()),
        None => Err(CompleteAgentRunError::StaleFence),
    }
}

struct Gateway<'a, S, A, O, C> {
    store: &'a S,
    adapter: &'a A,
    observer: &'a O,
    clock: &'a C,
    claim: &'a ClaimedAgentRun,
}

impl<S: ModelDispatchStore, A: ModelProviderAdapter, O: ContractFaultObserver, C: GatewayClock>
    Gateway<'_, S, A, O, C>
{
    /// Sends one request in order. Returns the cancellation that stopped its stream, if any.
    #[tracing::instrument(skip_all, fields(
        project_id = self.claim.project_scope.project_id.diagnostic(),
        run_id = DiagnosticId(&self.claim.run_id).diagnostic(),
        request_kind = RequestKind(&request).diagnostic(),
        adapter = request.route().adapter.kind().diagnostic(),
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
            clock,
            claim,
        } = *self;
        let prepared = match adapter.prepare(&request).await {
            Ok(prepared) => prepared,
            Err(refusal) => {
                tracing::Span::current().record("observation", "pre_dispatch_refusal");
                store
                    .record(
                        claim,
                        DispatchRecord::Refusal {
                            refusal,
                            request: &request,
                        },
                    )
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
        let mut sink = GatewaySink::new(store, observer, clock, claim, &dispatch);
        let exchange = adapter.exchange(prepared.prepared, &mut sink);
        let observation = renew_while(store, clock, claim, exchange).await;
        record_observation(&observation);
        let cancellation = sink.finish().await;
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
        if let DestinationRequest::Create(create) = &request
            && create.purpose == CreatePurpose::CompactionSummary
        {
            observer.reached(ContractFaultPoint::CompactionStaged).await;
        }
        Ok(cancellation)
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
