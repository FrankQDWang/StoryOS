//! The ToolCall rows of one durable Agent Decision (ADR 0034, ADR 0084). The store writes them
//! in the transaction that makes the Decision durable, one for each validated member in item
//! order. The identity is the Model Attempt and the native call identifier, so a second delivery
//! of the same observation or a retry of the same commit creates no second row.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{ToolCallBatch, ValidatedToolCall};
use uuid::Uuid;

use crate::agent_run_work::complete_database_error;

/// The caller route of a ToolCall that a model requested.
const MODEL_CALLER_ROUTE: &str = "model";

/// The Decision and the Attempt that the rows belong to.
pub(crate) struct ToolCallOwner<'a> {
    pub run_id: &'a str,
    pub decision_position: &'a str,
    pub model_attempt_id: &'a str,
    pub decision_id: &'a str,
}

/// Inserts one `pending` row for each call. An existing row of the same Attempt and call
/// identifier stays as it is.
pub(crate) async fn insert_tool_calls(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    owner: &ToolCallOwner<'_>,
    calls: &[ValidatedToolCall],
) -> Result<(), CompleteAgentRunError> {
    for (position, call) in (0_i32..).zip(calls) {
        client
            .execute(
                "INSERT INTO storyos.tool_calls
                   (owner_user_id, project_id, tool_call_id, run_id, decision_position,
                    model_attempt_id, decision_id, call_id, call_position, caller_route,
                    function_name, registration_id, tool_registration_revision,
                    tool_spec_revision, arguments, argument_digest, state)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::numeric, $6::text::uuid, $7::text::uuid, $8, $9, $10,
                         $11, $12, $13::text::uuid, $14, $15::text::jsonb, $16, 'pending')
                 ON CONFLICT ON CONSTRAINT tool_calls_attempt_call_key DO NOTHING",
                &[
                    &claim.project_scope.owner_user_id.as_ref(),
                    &claim.project_scope.project_id.as_ref(),
                    &Uuid::now_v7().to_string(),
                    &owner.run_id,
                    &owner.decision_position,
                    &owner.model_attempt_id,
                    &owner.decision_id,
                    &call.call_id,
                    &position,
                    &MODEL_CALLER_ROUTE,
                    &call.name,
                    &call.registration_id,
                    &call.registration_revision,
                    &call.spec_revision,
                    &call.arguments().to_string(),
                    &call.argument_digest(),
                ],
            )
            .await
            .map_err(complete_database_error)?;
    }
    Ok(())
}

/// Whether the Run has a ToolCall that no execution resolved.
pub(crate) async fn pending_tool_call_exists(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<bool, CompleteAgentRunError> {
    client
        .query_one(
            "SELECT EXISTS (
               SELECT 1 FROM storyos.tool_calls
                WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                  AND run_id = $3::text::uuid AND state = 'pending')",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map(|row| row.get(0))
        .map_err(complete_database_error)
}

/// The evidence record of the batch validation on the Attempt payload. `None` when the response
/// had no Tool item.
pub(crate) fn batch_evidence(batch: &ToolCallBatch) -> Option<serde_json::Value> {
    match batch {
        ToolCallBatch::Valid(calls) if calls.is_empty() => None,
        ToolCallBatch::Valid(calls) => Some(serde_json::json!({
            "kind": "valid",
            "call_ids": calls.iter().map(|call| call.call_id.as_str()).collect::<Vec<_>>(),
        })),
        ToolCallBatch::Invalid(refusal) => Some(serde_json::json!({
            "kind": "invalid",
            "refusal": refusal.label(),
        })),
    }
}

/// The batch of the terminal `response` against the Exposure that the Step Snapshot of
/// `attempt_payload` pinned. A damaged Snapshot is a store fault.
pub(crate) fn validate_batch(
    attempt_payload: &serde_json::Value,
    response: &storyos_application::ModelResponse,
) -> Result<ToolCallBatch, CompleteAgentRunError> {
    let exposure = storyos_core::ExposureSnapshot::decode(
        attempt_payload.pointer("/step_snapshot/tool_exposure"),
    )
    .ok_or_else(|| {
        CompleteAgentRunError::Unavailable(Box::new(std::io::Error::other(
            "The Step Snapshot of the Tool Exposure is damaged",
        )))
    })?;
    Ok(storyos_core::validate_tool_call_batch(
        response.state,
        &response.items,
        &exposure,
    ))
}

#[cfg(test)]
#[path = "agent_run_tool_call_tests.rs"]
pub(crate) mod tests;
