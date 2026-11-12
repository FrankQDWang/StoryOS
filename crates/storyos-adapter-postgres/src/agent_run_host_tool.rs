//! The in-process execution of a Host ToolCall (ADR 0091). It runs in the Tool Gateway
//! transaction that checked the Run Lease fence, and it skips each destination step: no Context
//! Assembly, no Destination Attempt, no Outbound Disclosure Event, and no source capture.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{ToolCallState, ToolSpec};

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, damaged, resolve};

/// Executes the pending Host ToolCall `row` of the Host Tool `spec`, and settles it with its
/// result and its one `tool_call_settled` Activity Run Event.
pub(crate) async fn execute(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    spec: &ToolSpec,
) -> Result<(), CompleteAgentRunError> {
    let result = spec
        .intake(&row.arguments)
        .ok()
        .as_ref()
        .and_then(storyos_core::execute_host_tool)
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
