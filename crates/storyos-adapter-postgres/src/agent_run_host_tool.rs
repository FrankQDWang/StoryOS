//! The in-process execution of a Host ToolCall (ADR 0091). It runs in the Tool Gateway
//! transaction that checked the Run Lease fence, and it skips each destination step: no Context
//! Assembly, no Destination Attempt, no Outbound Disclosure Event, and no source capture.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{ToolIntake, ToolSpec};

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, damaged, resolve};

/// Executes the pending Host ToolCall `row` of the Host Tool `spec`, and settles it with its
/// result and its one `tool_call_settled` Activity Run Event. The Tool Gateway executes a Host
/// ToolCall after each other ToolCall of its response, and `finish` last, so the refusals of the
/// response are known.
pub(crate) async fn execute(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    spec: &ToolSpec,
) -> Result<(), CompleteAgentRunError> {
    let intake = spec
        .intake(&row.arguments)
        .map_err(|_| damaged("The Host ToolCall is damaged"))?;
    match &intake {
        ToolIntake::SubmitResearch { summary, claims } => {
            return crate::agent_run_submit_research::execute(client, claim, row, summary, claims)
                .await;
        }
        ToolIntake::ProposeEdit(edit) => {
            return crate::agent_run_propose_edit::execute(client, claim, row, edit).await;
        }
        ToolIntake::ReadToolResult { call_id } => {
            return crate::agent_run_read_tool_result::execute(client, claim, row, call_id).await;
        }
        ToolIntake::LoadSkill { skill, .. } => {
            return crate::agent_run_load_skill::execute(client, claim, row, skill).await;
        }
        ToolIntake::ReadSkillResource { skill, path } => {
            return crate::agent_run_read_skill_resource::execute(client, claim, row, skill, path)
                .await;
        }
        ToolIntake::ReadChapter {
            chapter,
            first_block,
            last_block,
        } => {
            let range = storyos_core::BlockRange {
                first: *first_block,
                last: *last_block,
            };
            return crate::agent_run_read_chapter::execute(client, claim, row, *chapter, range)
                .await;
        }
        ToolIntake::SearchManuscript { query } => {
            return crate::agent_run_search_manuscript::execute(client, claim, row, query).await;
        }
        ToolIntake::HybridSearchManuscript { .. } => {
            return crate::agent_run_hybrid_search::execute(client, claim, row).await;
        }
        ToolIntake::WriteMemory(write) => {
            return crate::agent_run_write_memory::execute(client, claim, row, write).await;
        }
        ToolIntake::ReadMemoryEntry { entry } => {
            return crate::agent_run_read_memory_entry::execute(client, claim, row, *entry).await;
        }
        ToolIntake::SearchMemory { query } => {
            return crate::agent_run_search_memory::execute(client, claim, row, query).await;
        }
        ToolIntake::RecordMemoryObservations(observations) => {
            return crate::agent_run_record_observations::execute(client, claim, row, observations)
                .await;
        }
        ToolIntake::ApplyMemoryOperations(operations) => {
            return crate::memory_consolidation::execute(client, claim, row, operations).await;
        }
        ToolIntake::RecordCharacterIntent { intent, questions } => {
            return crate::intent_pass::execute(client, claim, row, intent, questions).await;
        }
        ToolIntake::WebSearch { .. } | ToolIntake::WebFetch { .. } | ToolIntake::Finish => {}
    }
    let refused_beside: bool = client
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM storyos.tool_calls AS call
                              JOIN storyos.tool_calls AS sibling
                                ON (sibling.owner_user_id, sibling.project_id, sibling.run_id,
                                    sibling.decision_id) =
                                   (call.owner_user_id, call.project_id, call.run_id,
                                    call.decision_id)
                             WHERE call.owner_user_id = $1::text::uuid
                               AND call.project_id = $2::text::uuid
                               AND call.tool_call_id = $3::text::uuid
                               AND sibling.state = 'refused')",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &row.tool_call_id,
            ],
        )
        .await
        .map_err(crate::agent_run_work::complete_unavailable)?
        .get(0);
    let result = storyos_core::execute_host_tool(&intake, refused_beside)
        .ok_or_else(|| damaged("The Host ToolCall is damaged"))?;
    resolve(client, claim, row, Resolution::settled(&result)).await
}

#[cfg(test)]
#[path = "agent_run_host_tool_tests.rs"]
mod tests;
