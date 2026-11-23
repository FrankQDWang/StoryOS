//! The Tool Gateway: the only owner of the dispatch order for the ToolCalls of one durable Agent
//! Decision (ADR 0034, ADR 0039, ADR 0084).

use std::future::Future;

use storyos_core::{EmbeddingInput, EmbeddingRefusal};

use crate::model_gateway_ports::{
    ContractFaultObserver, ContractFaultPoint, GatewayClock, ModelDispatchStore,
};
use crate::model_lease_renewal::renew_or_drop;
use crate::tool_gateway_ports::{
    QuestionSearch, SubrunWork, ToolDestinationAdapter, ToolDispatchRecord, ToolDispatchStore,
    ToolWork,
};
use crate::{
    ClaimedAgentRun, CompleteAgentRunError, EmbeddingDispatchStore, EmbeddingProviderAdapter,
    EmbeddingResult, ProjectReadError, dispatch_embedding,
};

/// The embedding destination of the questions of the `hybrid_search_manuscript` calls.
pub(crate) trait QuestionEmbeddings: Sync {
    fn embed(
        &self,
        claim: &ClaimedAgentRun,
        search: &QuestionSearch,
    ) -> impl Future<Output = Result<EmbeddingResult, ProjectReadError>> + Send;
}

/// The question embeddings of a process with no embedding store: each request refuses with
/// `embedding_not_configured` before the claim.
pub(crate) struct NoQuestionEmbeddings;

impl QuestionEmbeddings for NoQuestionEmbeddings {
    async fn embed(
        &self,
        _claim: &ClaimedAgentRun,
        _search: &QuestionSearch,
    ) -> Result<EmbeddingResult, ProjectReadError> {
        Ok(EmbeddingResult::Refused(EmbeddingRefusal::NotConfigured))
    }
}

/// The question embeddings through the embedding sequence of ADR 0039 with `store` and
/// `adapter`.
pub(crate) struct Embeddings<'a, S, E> {
    pub store: &'a S,
    pub adapter: &'a E,
}

impl<S: EmbeddingDispatchStore, E: EmbeddingProviderAdapter> QuestionEmbeddings
    for Embeddings<'_, S, E>
{
    async fn embed(
        &self,
        claim: &ClaimedAgentRun,
        search: &QuestionSearch,
    ) -> Result<EmbeddingResult, ProjectReadError> {
        let input = EmbeddingInput::Question {
            tool_call_id: search.tool_call_id.clone(),
            text: search.question.clone(),
        };
        dispatch_embedding(
            self.store,
            self.adapter,
            Some(search.registration),
            &claim.project_scope,
            &input,
        )
        .await
    }
}

/// Dispatches each pending ToolCall in item order: gets the next ToolCall, prepares, commits the
/// dispatch claim, exchanges with no open transaction, and records the observation after a Run
/// Lease check. Returns when each ToolCall is resolved. A lost lease drops the exchange locally:
/// the fence that the cancel or the takeover committed settles the ToolCall (ADR 0086). A
/// `hybrid_search_manuscript` call sends the embedding request of its question with no open
/// transaction, and its record step ranks with the result (ADR 0095 section 1). Returns the
/// Subrun of a `start_subrun` call, which the Gateway runs before the next ToolCall (ADR 0103
/// section 3A).
pub(crate) async fn dispatch_tool_calls(
    store: &(impl ModelDispatchStore + ToolDispatchStore),
    tools: &impl ToolDestinationAdapter,
    embeddings: &impl QuestionEmbeddings,
    observer: &impl ContractFaultObserver,
    clock: &impl GatewayClock,
    claim: &ClaimedAgentRun,
) -> Result<Option<SubrunWork>, CompleteAgentRunError> {
    observer
        .reached(ContractFaultPoint::ToolCallsCommitted)
        .await;
    while let Some(work) = store.next_tool_call(claim).await? {
        let request = match work {
            ToolWork::Dispatch(request) => request,
            ToolWork::Subrun(work) => return Ok(Some(work)),
            ToolWork::QuestionSearch(search) => {
                // A lost lease drops the request. The Embedding Attempt stays OutcomeUnknown, and
                // the next claim of the Run sends a new request.
                let Some(result) =
                    renew_or_drop(store, clock, claim, embeddings.embed(claim, &search)).await
                else {
                    return Err(CompleteAgentRunError::StaleFence);
                };
                let result =
                    result.map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
                store.record_question_search(claim, &search, result).await?;
                continue;
            }
        };
        observer
            .reached(ContractFaultPoint::ToolContextCommitted)
            .await;
        let prepared = match tools.prepare(&request).await {
            Ok(prepared) => prepared,
            Err(refusal) => {
                store
                    .record_tool_call(
                        claim,
                        ToolDispatchRecord::Refusal {
                            request: &request,
                            refusal,
                        },
                    )
                    .await?;
                continue;
            }
        };
        let Some(dispatch) = store
            .commit_tool_dispatch_claim(claim, &request, &prepared.projection)
            .await?
        else {
            continue;
        };
        observer
            .reached(ContractFaultPoint::ToolDispatchClaimed)
            .await;
        let Some(observation) =
            renew_or_drop(store, clock, claim, tools.exchange(prepared.prepared)).await
        else {
            return Err(CompleteAgentRunError::StaleFence);
        };
        store
            .record_tool_call(
                claim,
                ToolDispatchRecord::Exchange {
                    claim: &dispatch,
                    request: &request,
                    observation,
                },
            )
            .await?;
    }
    Ok(None)
}
