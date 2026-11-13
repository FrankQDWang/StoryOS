//! The ToolCall rows of one durable Agent Decision (ADR 0034, ADR 0084). The store writes them
//! in the transaction that makes the Decision durable, one for each member in item order. A
//! member that failed its validation is a `refused` row with its reason as its result (ADR 0091
//! section 3). The identity is the Model Attempt and the native call identifier, so a second
//! delivery of the same observation or a retry of the same commit creates no second row.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{
    CurrentToolAuthority, ExposureSnapshot, RequestedToolCall, StreamItemRole,
    ToolAttemptSettlement, ToolCallBatch, ToolCallClass, ToolCallState, ToolRegistrationLifecycle,
    tool_call_class,
};
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

/// Inserts one `pending` row for each validated call, and one `refused` row with its result and
/// its `tool_call_settled` Activity Run Event for each refused call. An existing row of the same
/// Attempt and call identifier stays as it is.
pub(crate) async fn insert_tool_calls(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    owner: &ToolCallOwner<'_>,
    calls: &[RequestedToolCall],
) -> Result<(), CompleteAgentRunError> {
    // The global row of the Host Tool Registration appears at its first ToolCall.
    if calls.iter().any(|call| {
        matches!(call, RequestedToolCall::Validated(call)
            if call.registration_revision == storyos_core::HOST_TOOL_REGISTRATION.revision)
    }) {
        crate::tool_registration::insert_host_registration_row(client)
            .await
            .map_err(complete_database_error)?;
    }
    for (position, call) in (0_i32..).zip(calls) {
        let (name, pin, arguments, argument_digest, refusal) = match call {
            RequestedToolCall::Validated(call) => (
                &call.name,
                Some((
                    &call.registration_id,
                    &call.registration_revision,
                    &call.spec_revision,
                )),
                call.arguments(),
                call.argument_digest(),
                None,
            ),
            RequestedToolCall::Refused(call) => (
                &call.name,
                call.tool.as_ref().map(|tool| {
                    (
                        &tool.registration_id,
                        &tool.registration_revision,
                        &tool.spec_revision,
                    )
                }),
                call.arguments(),
                call.argument_digest(),
                Some(call.refusal),
            ),
        };
        let tool_call_id = Uuid::now_v7().to_string();
        let text = refusal.map(storyos_core::CallValidationRefusal::result_text);
        let digest = text
            .as_deref()
            .map(|text| format!("sha256:{}", storyos_core::hex_sha256(text.as_bytes())));
        let bytes = text
            .as_deref()
            .map(|text| i32::try_from(text.len()))
            .transpose()
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
        let state = match refusal {
            Some(_) => ToolCallState::Refused,
            None => ToolCallState::Pending,
        };
        let inserted = client
            .execute(
                "INSERT INTO storyos.tool_calls
                   (owner_user_id, project_id, tool_call_id, run_id, decision_position,
                    model_attempt_id, decision_id, call_id, call_position, caller_route,
                    function_name, registration_id, tool_registration_revision,
                    tool_spec_revision, arguments, argument_digest, state, refusal, result_text,
                    result_digest, result_bytes)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::numeric, $6::text::uuid, $7::text::uuid, $8, $9, $10,
                         $11, $12, $13::text::uuid, $14, $15::text::jsonb, $16, $17, $18, $19,
                         $20, $21)
                 ON CONFLICT ON CONSTRAINT tool_calls_attempt_call_key DO NOTHING",
                &[
                    &claim.project_scope.owner_user_id.as_ref(),
                    &claim.project_scope.project_id.as_ref(),
                    &tool_call_id,
                    &owner.run_id,
                    &owner.decision_position,
                    &owner.model_attempt_id,
                    &owner.decision_id,
                    &call.call_id(),
                    &position,
                    &MODEL_CALLER_ROUTE,
                    name,
                    &pin.map(|(id, _, _)| id),
                    &pin.map(|(_, revision, _)| revision),
                    &pin.map(|(_, _, spec)| spec),
                    &arguments.to_string(),
                    &argument_digest,
                    &state.as_str(),
                    &refusal.map(storyos_core::CallValidationRefusal::label),
                    &text,
                    &digest,
                    &bytes,
                ],
            )
            .await
            .map_err(complete_database_error)?;
        if inserted == 1
            && let Some(refusal) = refusal
        {
            crate::agent_run_tool_authority::append_event(
                client,
                claim,
                "tool_call_settled",
                &serde_json::json!({
                    "tool_call_id": tool_call_id,
                    "call_id": call.call_id(),
                    "tool_registration_revision": pin.map(|(_, revision, _)| revision),
                    "tool_spec_revision": pin.map(|(_, _, spec)| spec),
                    "destination_attempt_id": null,
                    "state": state.as_str(),
                    "refusal": refusal.label(),
                    "outcome": null,
                    "result_bytes": bytes,
                }),
            )
            .await?;
        }
    }
    Ok(())
}

/// Whether the Run has a ToolCall that no observation or refusal resolved.
pub(crate) async fn pending_tool_call_exists(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<bool, CompleteAgentRunError> {
    client
        .query_one(
            "SELECT EXISTS (
               SELECT 1 FROM storyos.tool_calls
                WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                  AND run_id = $3::text::uuid AND state IN ('pending', 'dispatched'))",
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

/// Whether a ToolCall of the Run has an effect that no evidence settled: a ToolCall with no
/// observation, or one whose outcome Core classifies as unknown (ADR 0033 section 5, ADR 0086).
/// A ToolCall of an earlier Decision can keep an unknown effect when a continuation starts.
pub(crate) async fn unresolved_tool_effect(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
) -> Result<bool, CompleteAgentRunError> {
    let rows = client
        .query(
            "SELECT state, attempt -> 'destination_attempt' ->> 'settlement',
                    attempt -> 'late_observation' ->> 'settlement'
               FROM storyos.tool_calls
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    let settlement = |value: Option<&str>| value.and_then(ToolAttemptSettlement::parse);
    Ok(rows.iter().any(|row| {
        let class = ToolCallState::parse(row.get(/*idx*/ 0)).and_then(|state| {
            tool_call_class(
                state,
                settlement(row.get(/*idx*/ 1)),
                settlement(row.get(/*idx*/ 2)),
            )
        });
        matches!(class, None | Some(ToolCallClass::Unknown))
    }))
}

/// Fills the calls of a function request Decision from its research ToolCall rows in item order:
/// the validated arguments and the outcome, and no identifier (ADR 0040). A Host ToolCall and a
/// call that failed its validation, which has no validated arguments, are not in the public list
/// (ADR 0091).
pub(crate) async fn hydrate_calls(
    client: &tokio_postgres::Client,
    scope: &storyos_application::ProjectScope,
    decision: &mut storyos_application::AgentRunDecisionInspect,
) -> Result<(), storyos_application::CreateAgentRunError> {
    let storyos_application::AgentRunDecisionInspect::ToolRequest { decision_id, calls } = decision
    else {
        return Ok(());
    };
    let rows = client
        .query(
            "SELECT tool_spec_revision, arguments::text, state FROM storyos.tool_calls
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND decision_id = $3::text::uuid AND registration_id <> $4
                AND jsonb_typeof(arguments) = 'object'
              ORDER BY call_position",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &decision_id.as_str(),
                &storyos_core::HOST_TOOL_REGISTRATION.id,
            ],
        )
        .await
        .map_err(crate::create_agent_run::agent_run_database_error)?;
    for row in rows {
        calls.push(call_inspect(
            row.get(/*idx*/ 0),
            row.get(/*idx*/ 1),
            row.get(/*idx*/ 2),
        )?);
    }
    Ok(())
}

/// What one ToolCall asked for, from its stored pins and arguments, and how it settled. An
/// `unknown` ToolCall may have run (ADR 0086).
pub(crate) fn call_inspect(
    spec_revision: &str,
    arguments: &str,
    state: &str,
) -> Result<storyos_contracts::ToolCallInspect, storyos_application::CreateAgentRunError> {
    use storyos_contracts::{ToolCallInspect, ToolCallOutcomeInspect};
    let damaged = || {
        storyos_application::CreateAgentRunError::Unavailable(Box::new(std::io::Error::other(
            "The ToolCall is damaged",
        )))
    };
    let arguments: serde_json::Value =
        serde_json::from_str(arguments).map_err(crate::create_agent_run::agent_run_parse_error)?;
    let intake = storyos_core::tool_spec(spec_revision)
        .and_then(|spec| spec.intake(&arguments).ok())
        .ok_or_else(damaged)?;
    let outcome = match ToolCallState::parse(state).ok_or_else(damaged)? {
        ToolCallState::Pending | ToolCallState::Dispatched => ToolCallOutcomeInspect::Pending,
        ToolCallState::Settled => ToolCallOutcomeInspect::Settled,
        ToolCallState::Unknown => ToolCallOutcomeInspect::Unknown,
        ToolCallState::Failed | ToolCallState::Refused | ToolCallState::AwaitingApproval => {
            ToolCallOutcomeInspect::Failed
        }
    };
    Ok(match intake {
        storyos_core::ToolIntake::WebSearch { query, .. } => {
            ToolCallInspect::Search { query, outcome }
        }
        storyos_core::ToolIntake::WebFetch { urls } => ToolCallInspect::PageRead { urls, outcome },
        // The public list holds only research calls.
        storyos_core::ToolIntake::Finish | storyos_core::ToolIntake::SubmitResearch { .. } => {
            return Err(damaged());
        }
    })
}

/// The evidence record of the batch validation on the Attempt payload. `None` when the response
/// had no Tool item.
pub(crate) fn batch_evidence(batch: &ToolCallBatch) -> Option<serde_json::Value> {
    match batch {
        ToolCallBatch::Valid(calls) if calls.is_empty() => None,
        ToolCallBatch::Valid(calls) => Some(serde_json::json!({
            "kind": "valid",
            "call_ids": calls.iter().map(RequestedToolCall::call_id).collect::<Vec<_>>(),
        })),
        ToolCallBatch::Invalid(refusal) => Some(serde_json::json!({
            "kind": "invalid",
            "refusal": refusal.label(),
        })),
    }
}

/// The batch of the terminal `response` against the Exposure that the Step Snapshot of
/// `attempt_payload` pinned and against the current authority, which the transaction of the
/// caller reads before it selects the Decision. A damaged Snapshot is a store fault.
pub(crate) async fn validate_batch(
    client: &tokio_postgres::Client,
    attempt_payload: &serde_json::Value,
    response: &storyos_application::ModelResponse,
) -> Result<ToolCallBatch, CompleteAgentRunError> {
    let exposure =
        ExposureSnapshot::decode(attempt_payload.pointer("/step_snapshot/tool_exposure"))
            .ok_or_else(|| damaged("The Step Snapshot of the Tool Exposure is damaged"))?;
    // A response without a Tool item makes no batch member, so it needs no current authority.
    let current = if response
        .items
        .iter()
        .any(|item| item.role == StreamItemRole::Tool)
    {
        current_tool_authority(client, &exposure).await?
    } else {
        Vec::new()
    };
    Ok(storyos_core::validate_tool_call_batch(
        response.state,
        &response.items,
        &exposure,
        &current,
    ))
}

/// The current authority of each Registration revision that `exposure` pinned: the lifecycle of
/// the revision. A revision without a lifecycle row is the active constant, because the row
/// appears at the first binding of the deployment.
async fn current_tool_authority(
    client: &tokio_postgres::Client,
    exposure: &ExposureSnapshot,
) -> Result<Vec<CurrentToolAuthority>, CompleteAgentRunError> {
    let mut current: Vec<CurrentToolAuthority> = Vec::new();
    for tool in &exposure.tools {
        if current.iter().any(|authority| {
            authority.registration_id == tool.registration_id
                && authority.registration_revision == tool.registration_revision
        }) {
            continue;
        }
        let row = client
            .query_opt(
                "SELECT lifecycle FROM storyos.tool_registration_revisions
                  WHERE tool_registration_revision = $1::text::uuid",
                &[&tool.registration_revision],
            )
            .await
            .map_err(complete_database_error)?;
        let lifecycle = match row.as_ref().map(|row| row.get::<_, &str>(/*idx*/ 0)) {
            None => ToolRegistrationLifecycle::Active,
            Some(lifecycle) => ToolRegistrationLifecycle::parse(lifecycle)
                .ok_or_else(|| damaged("Tool Registration is damaged"))?,
        };
        current.push(CurrentToolAuthority {
            registration_id: tool.registration_id.clone(),
            registration_revision: tool.registration_revision.clone(),
            lifecycle,
        });
    }
    Ok(current)
}

fn damaged(message: &'static str) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(std::io::Error::other(message)))
}

#[cfg(test)]
#[path = "agent_run_tool_call_tests.rs"]
pub(crate) mod tests;
