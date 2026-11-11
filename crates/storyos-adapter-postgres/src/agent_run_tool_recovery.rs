//! The fence of an interrupted ToolCall and the late result after it (S5-08, ADR 0086). The
//! cancel transaction writes one ToolCall Cancellation fence on each dispatched ToolCall and
//! settles it `unknown`; the next claim after a Worker restart writes a restart fence the same
//! way. The Worker stops only after it reads a committed fence, and it can only drop the
//! exchange. A result that arrives after the fence settles the disclosure and the effect evidence
//! of its exact Destination Attempt only.

use serde_json::{Value, json};
use storyos_application::{
    ClaimedAgentRun, CompleteAgentRunError, ProjectScope, ToolCallRequest, ToolDispatchClaim,
};
use storyos_core::{ToolAttemptSettlement, ToolCallState, ToolObservation};
use uuid::Uuid;

use crate::agent_run_tool_authority::{
    Resolution, ToolCallRow, append_event, damaged, locked, refuse,
};
use crate::agent_run_tool_dispatch::settled_attempt;
use crate::agent_run_work::complete_database_error;

/// What fenced a dispatched ToolCall, with the Run fence token that the fence committed with.
#[derive(Clone, Copy)]
pub(crate) enum ToolCallFence {
    /// The ToolCall Cancellation fence of `cancelAgentRun`.
    Cancellation { fence_generation: u64 },
    /// The next claim of the Run after the Worker that dispatched the ToolCall lost its lease.
    Restart { fence_token: i64 },
}

impl ToolCallFence {
    fn record(self) -> Value {
        let (cause, token) = match self {
            Self::Cancellation { fence_generation } => {
                ("run_cancelled", fence_generation.to_string())
            }
            Self::Restart { fence_token } => ("worker_restart", fence_token.to_string()),
        };
        // The MCP transport has no remote abort: the Host only drops its local exchange.
        json!({
            "fence_id": Uuid::now_v7().to_string(),
            "cause": cause,
            "run_fence_token": token,
            "local_stop": "drop_exchange",
            "remote_abort": "unsupported",
        })
    }
}

/// Fences each unresolved ToolCall of a Run that the author cancels, in the cancel transaction
/// and so before any local stop. A `pending` ToolCall refuses before its claim; a `dispatched`
/// ToolCall settles `unknown` with its fence, and nothing is sent.
pub(crate) async fn fence_on_cancel(
    client: &tokio_postgres::Client,
    project_scope: &ProjectScope,
    run_id: &str,
    fence_generation: u64,
) -> Result<(), CompleteAgentRunError> {
    let run = ClaimedAgentRun {
        project_scope: project_scope.clone(),
        run_id: run_id.to_owned(),
        // The cancel transaction holds the Run lock; no fence check follows.
        fence_token: 0,
    };
    let unresolved = client
        .query(
            "SELECT tool_call_id::text, state FROM storyos.tool_calls
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND state IN ('pending', 'dispatched')
              ORDER BY decision_position, call_position",
            &[
                &project_scope.owner_user_id.as_ref(),
                &project_scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    for found in unresolved {
        let tool_call_id: String = found.get(/*idx*/ 0);
        let state = ToolCallState::parse(found.get(/*idx*/ 1))
            .ok_or_else(|| damaged("The ToolCall state is damaged"))?;
        let Some(row) = locked(client, &run, &tool_call_id, state).await? else {
            continue;
        };
        if state == ToolCallState::Pending {
            refuse(client, &run, &row, ToolCallState::Refused, "run_cancelled").await?;
        } else {
            let fence = ToolCallFence::Cancellation { fence_generation };
            settle_interrupted(client, &run, &row, fence).await?;
        }
    }
    Ok(())
}

/// Settles a `dispatched` ToolCall `unknown` with its fence: the outcome of the exchange is not
/// known, and no Gateway sends it again.
pub(crate) async fn settle_interrupted(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    fence: ToolCallFence,
) -> Result<(), CompleteAgentRunError> {
    let settlement =
        storyos_core::settle_tool_call(row.plan()?.tool.spec, &ToolObservation::OutcomeUnknown);
    let mut attempt = settled_attempt(
        row,
        settlement.attempt.as_str(),
        settlement.outcome.record(),
    );
    attempt["fence"] = fence.record();
    attempt["reservation"] = storyos_core::ToolCallClass::Unknown.reservation();
    crate::agent_run_tool_authority::resolve(
        client,
        claim,
        row,
        Resolution {
            state: settlement.state,
            refusal: None,
            attempt: Some(&attempt),
            outcome: Some(settlement.outcome.record()),
            result: None,
        },
    )
    .await
}

/// Records the observation of a Worker whose lease a fence ended, as evidence of its exact
/// Destination Attempt. Only a ToolCall that a fence settled takes it, one time; the state stays
/// `unknown`, the result text never becomes the result of the ToolCall, no Source Snapshot is
/// captured, and the reservation stays. A ToolCall that no fence settled yet keeps nothing: the
/// next claim settles it.
pub(crate) async fn record_late(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    request: &ToolCallRequest,
    dispatch: &ToolDispatchClaim,
    observation: &ToolObservation,
) -> Result<(), CompleteAgentRunError> {
    let Some(row) = locked(client, claim, &request.tool_call_id, ToolCallState::Unknown)
        .await?
        .filter(|row| {
            row.destination_attempt_id.as_deref() == Some(dispatch.destination_attempt_id.as_str())
        })
    else {
        return Ok(());
    };
    let Some(mut attempt) = row.attempt.clone().filter(|attempt| {
        attempt.get("fence").is_some() && attempt.get("late_observation").is_none()
    }) else {
        return Ok(());
    };
    let late = storyos_core::late_tool_evidence(request.tool.spec, observation);
    let class = storyos_core::tool_call_class(
        ToolCallState::Unknown,
        Some(ToolAttemptSettlement::OutcomeUnknown),
        Some(late.attempt),
    );
    let outcome = late.outcome.record();
    attempt["late_observation"] = json!({
        "settlement": late.attempt.as_str(),
        "outcome": outcome,
        "disclosure_confirmation": {
            "event_id": attempt["outbound_disclosure_event"]["event_id"].clone(),
            "state": outcome["outbound_disclosure"].clone(),
        },
        "result_text": late.result.as_ref().map(|result| result.text.as_str()),
        "result_digest": late.result.as_ref().map(|result| result.digest.as_str()),
        "result_bytes": late.result.as_ref().map(|result| result.bytes),
    });
    client
        .execute(
            "UPDATE storyos.tool_calls SET attempt = $4::text::jsonb, effect_outcome = $5::text::jsonb
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND tool_call_id = $3::text::uuid AND state = 'unknown'",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &row.tool_call_id,
                &attempt.to_string(),
                &outcome.to_string(),
            ],
        )
        .await
        .map_err(complete_database_error)?;
    append_event(
        client,
        claim,
        "tool_call_late_result",
        &json!({
            "tool_call_id": row.tool_call_id,
            "call_id": row.call_id,
            "destination_attempt_id": row.destination_attempt_id,
            "class": class.map(storyos_core::ToolCallClass::as_str),
            "outcome": outcome,
            "result_bytes": late.result.as_ref().map(|result| result.bytes),
        }),
    )
    .await
}

#[cfg(test)]
#[path = "agent_run_tool_recovery_tests.rs"]
mod tests;
