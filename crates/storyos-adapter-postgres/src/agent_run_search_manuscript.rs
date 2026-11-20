//! The in-process execution of a `search_manuscript` call (ADR 0095 section 1). It searches the
//! authoritative head of each live Chapter of the Project of the Run, in manuscript order, with
//! the exact match of the author search, and it settles the call with the bounded list of the
//! Blocks that have a match. It searches no Proposal candidate, and it writes no Outbound
//! Disclosure Event.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, resolve};

/// Searches the manuscript for `query` for the pending call `row`, and settles `row`. A search
/// with no match also settles.
pub(crate) async fn execute(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    query: &str,
) -> Result<(), CompleteAgentRunError> {
    let chapters = crate::agent_run_read_chapter::readable_chapters(client, claim).await?;
    let result = storyos_core::admit_manuscript_search(&chapters, query, &row.call_id);
    resolve(
        client,
        claim,
        row,
        Resolution::settled(&result),
    )
    .await
}

#[cfg(test)]
#[path = "agent_run_search_manuscript_tests.rs"]
mod tests;
