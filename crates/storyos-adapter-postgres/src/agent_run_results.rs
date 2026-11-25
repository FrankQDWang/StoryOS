//! The results of a Run in the order in which the model made them (ADR 0091 section 6): the
//! Proposal of each `propose_edit` call that settled, the Research Artifact of the
//! `submit_research` call that settled, the Memory Entry change of each `write_memory` call that
//! settled, then the final answer text of a completed Run. No result replaces another.

use storyos_application::{AgentRunDecisionInspect, CreateAgentRunError, ProjectScope};
use storyos_contracts::{
    AgentRunResultInspect, MemoryWriteActionInspect, ProseChangeLocationInspect,
};
use storyos_core::{AgentRunStatus, MemoryEntryState, MemoryWriteAction, RunCallPosition};

use crate::create_agent_run::{agent_run_database_error, agent_run_parse_error};

/// The results of the Run `run_id` with the status `status`. `closing` is the Decision of its
/// active RunStep, which gives the final answer text.
pub(crate) async fn load(
    client: &crate::statement_cache::Client,
    scope: &ProjectScope,
    run_id: &str,
    status: AgentRunStatus,
    closing: &AgentRunDecisionInspect,
) -> Result<Vec<AgentRunResultInspect>, CreateAgentRunError> {
    // One response accepts one call of each Host Tool, so each row is one result.
    let rows = client
        .query(
            "SELECT call.decision_position::bigint, call.call_position, call.decision_id::text,
                    artifact.research_artifact_id::text, (attempt.payload -> 'proposed_edit')::text
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
                     OR artifact.research_artifact_id IS NOT NULL)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    let mut results = memory_writes(client, scope, run_id).await?;
    for row in rows {
        let position = RunCallPosition {
            decision_position: row.get(/*idx*/ 0),
            call_position: row.get(/*idx*/ 1),
        };
        let decision_id: String = row.get(/*idx*/ 2);
        if let Some(research_artifact_id) = row.get::<_, Option<String>>(/*idx*/ 3) {
            let artifact = AgentRunResultInspect::ResearchArtifact {
                decision_id,
                research_artifact_id,
            };
            results.push((position, artifact));
            continue;
        }
        let record: serde_json::Value =
            serde_json::from_str(row.get(/*idx*/ 4)).map_err(agent_run_parse_error)?;
        let mut locations: Vec<ProseChangeLocationInspect> =
            serde_json::from_value::<Option<_>>(record["locations"].clone())
                .map_err(agent_run_parse_error)?
                .unwrap_or_default();
        crate::prose_change_location_read::hydrate(client, scope, &decision_id, &mut locations)
            .await?;
        let proposal = AgentRunResultInspect::Proposal {
            decision_id,
            proposal_id: record["proposal_id"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            locations,
        };
        results.push((position, proposal));
    }
    let answer = match closing {
        AgentRunDecisionInspect::Advisory {
            decision_id, text, ..
        } if status == AgentRunStatus::Completed && !text.is_empty() => {
            Some(AgentRunResultInspect::Answer {
                decision_id: decision_id.clone(),
                text: text.clone(),
            })
        }
        _ => None,
    };
    Ok(storyos_core::run_results(results, answer))
}

/// The change of each settled `write_memory` call of the Run, from the version row that the call
/// wrote. A refused call wrote no version. A version of background work can keep the source of the
/// call from its entry, so only a version with the cause kind `author_request` is a write of the
/// call: `write_memory` is the one writer of that cause (ADR 0091 section 6, #1692).
async fn memory_writes(
    client: &crate::statement_cache::Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Vec<(RunCallPosition, AgentRunResultInspect)>, CreateAgentRunError> {
    let rows = client
        .query(
            "SELECT call.decision_position::bigint, call.call_position, call.decision_id::text,
                    version.state, version.version = 1, version.title
               FROM storyos.tool_calls AS call
               JOIN storyos.memory_entry_versions AS version
                 ON (version.owner_user_id, version.project_id) =
                    (call.owner_user_id, call.project_id)
                AND version.cause ->> 'kind' = 'author_request'
                AND version.source_references @>
                    jsonb_build_array(jsonb_build_object('tool_call_id', call.tool_call_id::text))
              WHERE call.owner_user_id = $1::text::uuid AND call.project_id = $2::text::uuid
                AND call.run_id = $3::text::uuid AND call.state = 'settled'
                AND call.function_name = 'write_memory'",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    rows.iter()
        .map(|row| {
            let state = MemoryEntryState::parse(row.get(/*idx*/ 3)).ok_or_else(|| {
                agent_run_parse_error(std::io::Error::other("The Memory Entry state is unknown"))
            })?;
            let action = match MemoryWriteAction::of_version(state, row.get(/*idx*/ 4)) {
                MemoryWriteAction::Create => MemoryWriteActionInspect::Create,
                MemoryWriteAction::Change => MemoryWriteActionInspect::Change,
                MemoryWriteAction::Delete => MemoryWriteActionInspect::Delete,
            };
            let position = RunCallPosition {
                decision_position: row.get(/*idx*/ 0),
                call_position: row.get(/*idx*/ 1),
            };
            let write = AgentRunResultInspect::MemoryWrite {
                decision_id: row.get(/*idx*/ 2),
                action,
                title: row.get(/*idx*/ 5),
            };
            Ok((position, write))
        })
        .collect()
}

#[cfg(test)]
#[path = "agent_run_results_tests.rs"]
mod tests;
