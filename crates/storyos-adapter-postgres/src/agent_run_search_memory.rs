//! The in-process execution of a `search_memory` call (ADR 0094 section 4). It reads the current
//! Memory Entries of the Project of the Run at the time of the call, and it settles the call
//! with the entries that hold the term. A search writes no read row: only a read of an entry
//! counts as a read. It writes no Outbound Disclosure Event. No diagnostic records the term or a
//! value of an entry (ADR 0047).

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, resolve};

/// Searches the current entries for `query` for the pending call `row`, and settles `row`.
pub(crate) async fn execute(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    query: &str,
) -> Result<(), CompleteAgentRunError> {
    let (_, _, entries) = crate::memory_entries::current_memory(client, &claim.project_scope)
        .await
        .map_err(CompleteAgentRunError::Unavailable)?;
    let result = storyos_core::search_memory(&entries, query, &row.call_id);
    resolve(
        client,
        claim,
        row,
        Resolution::settled(&result),
    )
    .await
}

#[cfg(test)]
#[path = "agent_run_search_memory_tests.rs"]
mod tests;
