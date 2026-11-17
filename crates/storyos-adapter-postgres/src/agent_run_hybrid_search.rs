//! The in-process execution of a `hybrid_search_manuscript` call (ADR 0095 section 1). It ranks
//! the current passages of each live Chapter of the Project of the Run with the exact terms of
//! the call, and it settles the call with the bounded result. It sends no embedding request, and
//! it writes no Embedding Attempt, no Destination Attempt, and no Outbound Disclosure Event. The
//! embedding request of the question is S7-C3b2.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{PassageTerms, ToolCallState};

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, damaged, resolve};
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

/// Ranks the passages for `terms` for the pending call `row`, and settles `row`. The call is
/// refused with `hybrid_search_unavailable` when, at the call, the deployment has no embedding
/// configuration or the database has no vector store.
pub(crate) async fn execute(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    terms: &[String],
) -> Result<(), CompleteAgentRunError> {
    let facts = client
        .query_one(
            &format!(
                "SELECT {DEPLOYMENT_REGISTRATION} IS NOT NULL,
                        to_regclass('storyos.passage_vectors') IS NOT NULL"
            ),
            &[],
        )
        .await
        .map_err(complete_database_error)?;
    let (state, refusal, result) =
        match storyos_core::admit_hybrid_search(facts.get(/*idx*/ 0), facts.get(/*idx*/ 1)) {
            Ok(()) => {
                let terms = PassageTerms::new(terms)
                    .map_err(|_| damaged("The Host ToolCall is damaged"))?;
                let passages =
                    crate::passage_ranking::read_facts(client, &claim.project_scope, None)
                        .await
                        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?
                        .ok_or_else(|| damaged("The Project of the Run is not visible"))?;
                let result =
                    storyos_core::hybrid_search_result(&passages.chapters, &terms, &row.call_id);
                (ToolCallState::Settled, None, result)
            }
            Err(refusal) => (
                ToolCallState::Refused,
                Some(refusal.label()),
                refusal.result(),
            ),
        };
    resolve(
        client,
        claim,
        row,
        Resolution {
            state,
            refusal,
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
