//! The in-process execution of a `read_tool_result` call (ADR 0093). It reads the stored ToolCall
//! that the identity names and settles the call with the bounded form of the first read, or
//! refuses it with the reason. It writes no Source Snapshot and no Outbound Disclosure Event.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{ReadTarget, StoredCall, ToolCallState};

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, damaged, resolve};
use crate::agent_run_work::complete_database_error;

/// Reads the ToolCall `call_id` for the pending call `row`, and settles or refuses `row`.
pub(crate) async fn execute(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    call_id: &str,
) -> Result<(), CompleteAgentRunError> {
    // A ToolCall of the Run goes first, and then the latest one in item order.
    let found = client
        .query_opt(
            "SELECT call.tool_call_id::text, call.run_id = $3::text::uuid,
                    call.tool_spec_revision, call.state, call.result_text
               FROM storyos.tool_calls AS call
              WHERE call.owner_user_id = $1::text::uuid AND call.project_id = $2::text::uuid
                AND call.call_id = $4
              ORDER BY call.run_id = $3::text::uuid DESC, call.decision_position DESC,
                       call.call_position DESC
              LIMIT 1",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &call_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    let sources = match &found {
        Some(found) if found.get::<_, bool>(/*idx*/ 1) => {
            let tool_call_id: String = found.get(/*idx*/ 0);
            crate::research_artifact::run_sources(client, claim)
                .await?
                .into_iter()
                .filter(|(call, _)| *call == tool_call_id)
                .map(|(_, source)| source)
                .collect()
        }
        Some(_) | None => Vec::new(),
    };
    let target = match &found {
        None => ReadTarget::Unknown,
        Some(found) if !found.get::<_, bool>(/*idx*/ 1) => ReadTarget::OtherRun,
        Some(found) => ReadTarget::SameRun(StoredCall {
            call_id,
            spec_revision: found.get(/*idx*/ 2),
            state: ToolCallState::parse(found.get(/*idx*/ 3))
                .ok_or_else(|| damaged("The ToolCall state is damaged"))?,
            result_text: found.get(/*idx*/ 4),
            sources: &sources,
        }),
    };
    let read = storyos_core::read_tool_result(target, storyos_core::CONTEXT_ITEM_TOKEN_LIMIT);
    let (state, refusal, result) = match read {
        Ok(result) => (ToolCallState::Settled, None, result),
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
            refusal,
            attempt: None,
            outcome: None,
            result: Some(&result),
        },
    )
    .await
}

#[cfg(test)]
#[path = "agent_run_read_tool_result_tests.rs"]
mod tests;
