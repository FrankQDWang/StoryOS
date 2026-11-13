//! The in-process execution of a `submit_research` call (ADR 0091 section 2). It runs the Core
//! validation of ADR 0087 section 3 against the sources that the Run captured before the response
//! of the call, and it records the Research Artifact in the Tool Gateway transaction. One
//! response and one Run accept one call that records an Artifact.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{
    CitableSources, ClaimCandidate, RESEARCH_RECORDED_RESULT, SubmitResearchRefusal, ToolCallState,
};

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, resolve};
use crate::agent_run_work::{complete_database_error, load_run_phase};

/// Validates the pending `submit_research` call `row`, records its Research Artifact, and
/// settles it. A refused call gets the reason as its result and writes no Artifact.
pub(crate) async fn execute(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    summary: &str,
    claims: &[ClaimCandidate],
) -> Result<(), CompleteAgentRunError> {
    let facts = client
        .query_one(
            "SELECT call.model_attempt_id::text, call.decision_id::text,
                    EXISTS (SELECT 1 FROM storyos.tool_calls AS earlier
                             WHERE (earlier.owner_user_id, earlier.project_id,
                                    earlier.model_attempt_id) =
                                   (call.owner_user_id, call.project_id, call.model_attempt_id)
                               AND earlier.tool_spec_revision = call.tool_spec_revision
                               AND earlier.call_position < call.call_position),
                    EXISTS (SELECT 1 FROM storyos.research_artifacts AS artifact
                             WHERE (artifact.owner_user_id, artifact.project_id, artifact.run_id)
                                   = (call.owner_user_id, call.project_id, call.run_id)),
                    ARRAY(SELECT sibling.tool_call_id::text FROM storyos.tool_calls AS sibling
                           WHERE (sibling.owner_user_id, sibling.project_id,
                                  sibling.model_attempt_id) =
                                 (call.owner_user_id, call.project_id, call.model_attempt_id))
               FROM storyos.tool_calls AS call
              WHERE call.owner_user_id = $1::text::uuid AND call.project_id = $2::text::uuid
                AND call.tool_call_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &row.tool_call_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    let (attempt_id, decision_id): (String, String) = (facts.get(/*idx*/ 0), facts.get(/*idx*/ 1));
    let same_response: Vec<String> = facts.get(/*idx*/ 4);
    let refusal = if facts.get(/*idx*/ 2) {
        Err(SubmitResearchRefusal::SecondInResponse)
    } else if facts.get(/*idx*/ 3) {
        Err(SubmitResearchRefusal::ArtifactRecorded)
    } else {
        // The model has not read the results of the calls of its own response.
        let sources = crate::research_artifact::run_sources(client, claim)
            .await?
            .into_iter()
            .filter(|(tool_call_id, _)| !same_response.contains(tool_call_id))
            .map(|(_, source)| source)
            .collect();
        let citable = CitableSources {
            owner_user_id: claim.project_scope.owner_user_id.as_ref().to_owned(),
            project_id: claim.project_scope.project_id.as_ref().to_owned(),
            run_id: claim.run_id.clone(),
            sources,
        };
        storyos_core::validate_research_synthesis(summary, claims, &citable)
            .map_err(SubmitResearchRefusal::Synthesis)
    };
    let (state, label, result) = match refusal {
        Ok(synthesis) => {
            let run = load_run_phase(client, claim).await?;
            crate::research_artifact::record(
                client,
                claim,
                &run,
                &attempt_id,
                &decision_id,
                &synthesis,
            )
            .await?;
            (
                ToolCallState::Settled,
                None,
                storyos_core::host_tool_result(RESEARCH_RECORDED_RESULT),
            )
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
            refusal: label,
            attempt: None,
            outcome: None,
            result: Some(&result),
        },
    )
    .await
}

#[cfg(test)]
#[path = "agent_run_submit_research_tests.rs"]
mod tests;
