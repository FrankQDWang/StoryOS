//! The in-process execution of a `read_memory_entry` call (ADR 0094 section 4). It reads the
//! current version of the entry with the number of the call in the Project of the Run, and it
//! settles the call with that version and writes one read row in the Tool Gateway transaction
//! that settles it, or it refuses the call. It writes no Outbound Disclosure Event. No diagnostic
//! records a value of an entry (ADR 0047).

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::ToolCallState;

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, resolve};

/// Reads the entry with the number `entry` for the pending call `row`, and settles or refuses
/// `row`.
pub(crate) async fn execute(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    entry: i64,
) -> Result<(), CompleteAgentRunError> {
    let scope = &claim.project_scope;
    let read = crate::memory_entries::read_current_entry(client, scope, entry)
        .await
        .map_err(CompleteAgentRunError::Unavailable)?;
    let (state, refusal, result) = match read {
        Some(read) => {
            client
                .execute(
                    "INSERT INTO storyos.memory_entry_reads
                       (owner_user_id, project_id, tool_call_id, run_id, memory_entry_id, version)
                     VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                             $5::text::uuid, $6::text::numeric)",
                    &[
                        &scope.owner_user_id.as_ref(),
                        &scope.project_id.as_ref(),
                        &row.tool_call_id,
                        &claim.run_id,
                        &read.memory_entry_id,
                        &read.version.to_string(),
                    ],
                )
                .await
                .map_err(crate::agent_run_work::complete_database_error)?;
            (ToolCallState::Settled, None, read.result())
        }
        None => (
            ToolCallState::Refused,
            Some(storyos_core::MEMORY_READ_NOT_FOUND),
            storyos_core::memory_read_not_found(),
        ),
    };
    resolve(
        client,
        claim,
        row,
        Resolution::host(state, refusal, Some(&result)),
    )
    .await
}

#[cfg(test)]
#[path = "agent_run_read_memory_entry_tests.rs"]
pub(crate) mod tests;
