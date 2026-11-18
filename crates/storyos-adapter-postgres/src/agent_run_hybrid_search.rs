//! The execution of a `hybrid_search_manuscript` call (ADR 0095 section 1). The Tool Gateway
//! transaction gives the call to the Tool Gateway as work that needs the vector of its question.
//! The Tool Gateway sends the embedding request with no open transaction, and the record step
//! ranks the current passages of each live Chapter of the Project of the Run with the exact terms
//! and the vector, and settles the call. The call writes no Destination Attempt.

use storyos_application::{
    ClaimedAgentRun, CompleteAgentRunError, EmbeddingResult, QuestionSearch, QuestionVector,
};
use storyos_core::{
    EmbeddingRegistration, HybridSearchRefusal, PassageTerms, ToolCallState, ToolIntake,
    ToolOperation, ToolSpec,
};

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, damaged, locked, resolve};
use crate::agent_run_work::complete_database_error;

/// The revision of the deployment Embedding Registration in the transaction setting, or `NULL`.
pub(crate) const DEPLOYMENT_REGISTRATION: &str =
    "NULLIF(current_setting('storyos.embedding_registration', true), '')";

/// Whether the search by meaning is possible for the Run of `claim` (ADR 0095 section 4): the
/// deployment has an embedding configuration, the vector store is available, and the latest
/// embedding use binding of the Project has the Embedding Registration of the deployment.
pub(crate) async fn possible(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<bool, CompleteAgentRunError> {
    Ok(client
        .query_one(
            &format!(
                "SELECT to_regclass('storyos.passage_vectors') IS NOT NULL
                    AND EXISTS (SELECT 1 FROM (
                          SELECT binding.embedding_registration_revision
                            FROM storyos.project_external_use_binding_revisions AS binding
                           WHERE binding.owner_user_id = $1::text::uuid
                             AND binding.project_id = $2::text::uuid
                             AND binding.embedding_registration_revision IS NOT NULL
                           ORDER BY binding.project_model_use_binding_revision DESC
                           LIMIT 1) AS latest
                         WHERE latest.embedding_registration_revision::text =
                               {DEPLOYMENT_REGISTRATION})"
            ),
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(complete_database_error)?
        .get(/*idx*/ 0))
}

/// The embedding request of the question of the pending call `row` of the Host Tool `spec`, when
/// `row` is a `hybrid_search_manuscript` call, the deployment has the embedding configuration
/// `registration`, and the database has the vector store. Otherwise `None`, and `execute` refuses
/// the call.
pub(crate) async fn question_search(
    client: &tokio_postgres::Client,
    row: &ToolCallRow,
    spec: &ToolSpec,
    registration: Option<&'static EmbeddingRegistration>,
) -> Result<Option<QuestionSearch>, CompleteAgentRunError> {
    let (ToolOperation::HybridSearchManuscript, Some(registration)) =
        (spec.operation, registration)
    else {
        return Ok(None);
    };
    if !vector_store(client).await? {
        return Ok(None);
    }
    let Ok(ToolIntake::HybridSearchManuscript { question, .. }) = spec.intake(&row.arguments)
    else {
        return Err(damaged("The Host ToolCall is damaged"));
    };
    Ok(Some(QuestionSearch {
        tool_call_id: row.tool_call_id.clone(),
        question,
        registration,
    }))
}

/// Refuses the pending call `row` with `hybrid_search_unavailable`: at the call, the deployment
/// has no embedding configuration or the database has no vector store. An admitted call is the
/// work of `question_search`.
pub(crate) async fn execute(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
) -> Result<(), CompleteAgentRunError> {
    let configured: bool = client
        .query_one(
            &format!("SELECT {DEPLOYMENT_REGISTRATION} IS NOT NULL"),
            &[],
        )
        .await
        .map_err(complete_database_error)?
        .get(/*idx*/ 0);
    let Err(refusal) = storyos_core::admit_hybrid_search(configured, vector_store(client).await?)
    else {
        return Err(damaged("The hybrid search call has no question vector"));
    };
    settle_refused(client, claim, row, refusal).await
}

/// Ranks the passages of the call of `search` with its exact terms and the result of the
/// embedding request, and settles the call. A vector gives the fused result. A settled request
/// with no vector gives the exact-term list after the line that says that the search by meaning
/// was not available. A refusal of the admission refuses the call with
/// `hybrid_search_unavailable`. A call that is not pending changes nothing.
pub(crate) async fn record(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    search: &QuestionSearch,
    result: EmbeddingResult,
) -> Result<(), CompleteAgentRunError> {
    let Some(row) = locked(client, claim, &search.tool_call_id, ToolCallState::Pending).await?
    else {
        return Ok(());
    };
    let values = match result {
        EmbeddingResult::Vector(values) => Some(values),
        EmbeddingResult::Settled(_) => None,
        EmbeddingResult::Refused(refusal) => {
            let refusal = HybridSearchRefusal::of_embedding(refusal);
            return settle_refused(client, claim, &row, refusal).await;
        }
    };
    let Some(Ok(ToolIntake::HybridSearchManuscript { terms, .. })) =
        storyos_core::host_tool(&row.registration_revision, &row.spec_revision)
            .map(|spec| spec.intake(&row.arguments))
    else {
        return Err(damaged("The Host ToolCall is damaged"));
    };
    let terms = PassageTerms::new(&terms).map_err(|_| damaged("The Host ToolCall is damaged"))?;
    let question = values.as_deref().map(|values| QuestionVector {
        registration_revision: search.registration.revision,
        values,
    });
    let passages = crate::passage_ranking::read_facts(client, &claim.project_scope, question)
        .await
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?
        .ok_or_else(|| damaged("The Project of the Run is not visible"))?;
    let nearest = values.map(|_| passages.nearest.as_slice());
    let result =
        storyos_core::hybrid_search_result(&passages.chapters, nearest, &terms, &row.call_id);
    resolve(
        client,
        claim,
        &row,
        Resolution {
            state: ToolCallState::Settled,
            refusal: None,
            attempt: None,
            outcome: None,
            result: Some(&result),
        },
    )
    .await
}

async fn vector_store(client: &tokio_postgres::Client) -> Result<bool, CompleteAgentRunError> {
    Ok(client
        .query_one(
            "SELECT to_regclass('storyos.passage_vectors') IS NOT NULL",
            &[],
        )
        .await
        .map_err(complete_database_error)?
        .get(/*idx*/ 0))
}

async fn settle_refused(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    refusal: HybridSearchRefusal,
) -> Result<(), CompleteAgentRunError> {
    let result = refusal.result();
    resolve(
        client,
        claim,
        row,
        Resolution {
            state: ToolCallState::Refused,
            refusal: Some(refusal.label()),
            attempt: None,
            outcome: None,
            result: Some(&result),
        },
    )
    .await
}

#[cfg(test)]
#[path = "agent_run_hybrid_search_tests.rs"]
mod tests;
