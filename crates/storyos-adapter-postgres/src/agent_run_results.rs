//! The results of a Run in the order in which the model made them (ADR 0091 section 6): the
//! Proposal of each `propose_edit` call that settled, the Research Artifact of the
//! `submit_research` call that settled, then the final answer text of a completed Run. No result
//! replaces another.

use storyos_application::{AgentRunDecisionInspect, CreateAgentRunError, ProjectScope};
use storyos_contracts::{AgentRunResultInspect, ProseChangeLocationInspect};
use storyos_core::AgentRunStatus;

use crate::create_agent_run::{agent_run_database_error, agent_run_parse_error};

/// The results of the Run `run_id` with the status `status`. `closing` is the Decision of its
/// active RunStep, which gives the final answer text.
pub(crate) async fn load(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
    status: AgentRunStatus,
    closing: &AgentRunDecisionInspect,
) -> Result<Vec<AgentRunResultInspect>, CreateAgentRunError> {
    // One response accepts one call of each Host Tool, so each row is one result.
    let rows = client
        .query(
            "SELECT call.decision_id::text, artifact.research_artifact_id::text,
                    (attempt.payload -> 'proposed_edit')::text
               FROM storyos.tool_calls AS call
               JOIN storyos.model_attempts AS attempt
                 ON (attempt.owner_user_id, attempt.project_id, attempt.model_attempt_id) =
                    (call.owner_user_id, call.project_id, call.model_attempt_id)
               LEFT JOIN storyos.research_artifacts AS artifact
                 ON (artifact.owner_user_id, artifact.project_id, artifact.model_attempt_id) =
                    (call.owner_user_id, call.project_id, call.model_attempt_id)
                AND call.function_name = 'submit_research'
              WHERE call.owner_user_id = $1::text::uuid AND call.project_id = $2::text::uuid
                AND call.run_id = $3::text::uuid AND call.state = 'settled'
                AND ((call.function_name = 'propose_edit' AND attempt.payload ? 'proposed_edit')
                     OR artifact.research_artifact_id IS NOT NULL)
              ORDER BY call.decision_position, call.call_position",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    let mut results = Vec::with_capacity(rows.len() + 1);
    for row in rows {
        let decision_id: String = row.get(/*idx*/ 0);
        if let Some(research_artifact_id) = row.get::<_, Option<String>>(/*idx*/ 1) {
            results.push(AgentRunResultInspect::ResearchArtifact {
                decision_id,
                research_artifact_id,
            });
            continue;
        }
        let record: serde_json::Value =
            serde_json::from_str(row.get(/*idx*/ 2)).map_err(agent_run_parse_error)?;
        let mut locations: Vec<ProseChangeLocationInspect> =
            serde_json::from_value::<Option<_>>(record["locations"].clone())
                .map_err(agent_run_parse_error)?
                .unwrap_or_default();
        crate::prose_change_location_read::hydrate(client, scope, &decision_id, &mut locations)
            .await?;
        results.push(AgentRunResultInspect::Proposal {
            decision_id,
            proposal_id: record["proposal_id"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            locations,
        });
    }
    if let AgentRunDecisionInspect::Advisory {
        decision_id, text, ..
    } = closing
        && status == AgentRunStatus::Completed
        && !text.is_empty()
    {
        results.push(AgentRunResultInspect::Answer {
            decision_id: decision_id.clone(),
            text: text.clone(),
        });
    }
    Ok(results)
}
