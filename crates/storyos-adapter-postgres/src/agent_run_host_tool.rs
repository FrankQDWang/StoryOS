//! The in-process execution of a Host ToolCall (ADR 0091). It runs in the Tool Gateway
//! transaction that checked the Run Lease fence, and it skips each destination step: no Context
//! Assembly, no Destination Attempt, no Outbound Disclosure Event, and no source capture.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{ToolCallState, ToolSpec};

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, damaged, resolve};

/// Executes the pending Host ToolCall `row` of the Host Tool `spec`, and settles it with its
/// result and its one `tool_call_settled` Activity Run Event. The Tool Gateway executes a Host
/// ToolCall after each other ToolCall of its response, so the refusals of the response are known.
pub(crate) async fn execute(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    spec: &ToolSpec,
) -> Result<(), CompleteAgentRunError> {
    if let Ok(storyos_core::ToolIntake::ProposeEdit(edit)) = spec.intake(&row.arguments) {
        return crate::agent_run_propose_edit::execute(client, claim, row, &edit).await;
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
        .map_err(crate::agent_run_work::complete_database_error)?
        .get(0);
    let result = spec
        .intake(&row.arguments)
        .ok()
        .and_then(|intake| storyos_core::execute_host_tool(&intake, refused_beside))
        .ok_or_else(|| damaged("The Host ToolCall is damaged"))?;
    resolve(
        client,
        claim,
        row,
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

#[cfg(test)]
#[path = "agent_run_host_tool_tests.rs"]
mod tests;
