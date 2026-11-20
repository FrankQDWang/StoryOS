//! The in-process execution of a `write_memory` call (ADR 0094 section 5, writer 1). It loads the
//! current Memory of the Project of the Run, decides the write with the Core rules, and writes the
//! version and the next Memory revision in the Tool Gateway transaction that settles the call. A
//! refused call writes no row. No diagnostic records a value of an entry (ADR 0047).

use serde_json::json;
use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{MemoryWrite, ToolCallState};

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, damaged, resolve};
use crate::memory_entries::{
    VersionSource, current_memory, lock_memory_head, write_memory_version,
};

/// Decides and writes the Memory change of the pending call `row`, and settles or refuses `row`.
pub(crate) async fn execute(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    write: &MemoryWrite,
) -> Result<(), CompleteAgentRunError> {
    let scope = &claim.project_scope;
    lock_memory_head(client, scope)
        .await
        .map_err(CompleteAgentRunError::Unavailable)?;
    let (revision, next, entries) = current_memory(client, scope)
        .await
        .map_err(CompleteAgentRunError::Unavailable)?;
    let (state, refusal, result) = match storyos_core::decide_memory_write(write, &entries, next) {
        Ok(version) => {
            // A background Run has no Conversation (S7-D3a), and it cannot write an entry.
            let conversation_id: String = client
                .query_one(
                    "SELECT conversation_id::text FROM storyos.agent_runs
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND run_id = $3::text::uuid",
                    &[
                        &scope.owner_user_id.as_ref(),
                        &scope.project_id.as_ref(),
                        &claim.run_id,
                    ],
                )
                .await
                .map_err(crate::agent_run_work::complete_database_error)?
                .get::<_, Option<String>>(0)
                .ok_or_else(|| damaged("A Memory write needs the Conversation of its Run"))?;
            let source = VersionSource {
                origin: "author_request",
                cause: json!({ "kind": "author_request" }),
                source_references: json!([{
                    "kind": "conversation",
                    "conversation_id": conversation_id,
                    "run_id": claim.run_id,
                    "tool_call_id": row.tool_call_id,
                }]),
            };
            let written = (revision, &version, row.tool_call_id.as_str());
            write_memory_version(client, scope, written, &source)
                .await
                .map_err(CompleteAgentRunError::Unavailable)?;
            (ToolCallState::Settled, None, version.result())
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
        Resolution::host(state, refusal, Some(&result)),
    )
    .await
}

#[cfg(test)]
#[path = "agent_run_write_memory_tests.rs"]
pub(crate) mod tests;
