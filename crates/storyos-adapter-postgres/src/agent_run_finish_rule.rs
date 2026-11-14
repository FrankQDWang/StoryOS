//! The records of the finish rule of ADR 0091 section 4: the `tool_choice` of the active RunStep
//! from its position and the ToolCalls of the RunStep before it, and the next RunStep after a
//! text that came before a `finish` call that counted.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{PriorStepCall, ToolCallState, ToolChoice};

use crate::agent_run_tool_authority::damaged;
use crate::agent_run_work::{RunPhaseRow, WorkPhase, complete_database_error};

/// The `tool_choice` of the request of the active RunStep of `run`.
pub(crate) async fn tool_choice(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
) -> Result<ToolChoice, CompleteAgentRunError> {
    // A RunStep that reads a steering input is an ordinary RunStep: no `finish` call before it
    // counts.
    if crate::agent_run_steering::reads_input(client, claim, &run.decision_position).await? {
        return Ok(storyos_core::step_tool_choice(run_step_limit(run)?, &[]));
    }
    let prior_step = client
        .query(
            "SELECT tool_spec_revision, state FROM storyos.tool_calls
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
                AND decision_position = $4::text::numeric - 1",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &run.decision_position,
            ],
        )
        .await
        .map_err(complete_database_error)?
        .iter()
        .map(|row| {
            // A refused call with a name that the Run does not expose pins no ToolSpec.
            let operation = match row.get::<_, Option<&str>>(/*idx*/ 0) {
                Some(spec) => Some(storyos_core::tool_spec(spec)?.operation),
                None => None,
            };
            Some(PriorStepCall {
                operation,
                state: ToolCallState::parse(row.get(/*idx*/ 1))?,
            })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| damaged("The ToolCall is damaged"))?;
    Ok(storyos_core::step_tool_choice(
        run_step_limit(run)?,
        &prior_step,
    ))
}

/// The RunStep limit of the active RunStep of `run`.
pub(crate) fn run_step_limit(
    run: &RunPhaseRow,
) -> Result<storyos_core::RunStepLimit, CompleteAgentRunError> {
    run.decision_position
        .parse::<u64>()
        .map(storyos_core::RunStepLimit::at)
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))
}

/// The finish rule for a response that a recovery path settles in the active RunStep of `run`,
/// with the `tool_choice` of that RunStep, as for a created response (ADR 0091, ADR 0088). The
/// Host executes no call of the last RunStep. A response of that RunStep that advances the
/// continuation keeps its calls as evidence, because the request that names it answers each of
/// them (ADR 0091 section 5). The calls of another response leave the items.
pub(crate) async fn recovered_outcome(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    response: &mut storyos_application::ModelResponse,
    outcome: storyos_core::AgentDecisionOutcome,
) -> Result<storyos_core::FinishRuleOutcome, CompleteAgentRunError> {
    let limit = run_step_limit(run)?;
    let rule = storyos_core::apply_finish_rule(
        tool_choice(client, claim, run).await?,
        limit,
        outcome,
        response.state,
        &response.items,
    );
    let advances = matches!(
        rule,
        storyos_core::FinishRuleOutcome::NoResult
            | storyos_core::FinishRuleOutcome::Settle(
                storyos_core::AgentDecisionOutcome::Decision {
                    advances_continuation: true,
                    ..
                }
            )
    );
    if !limit.executes_calls() && !advances {
        response
            .items
            .retain(|item| item.role != storyos_core::StreamItemRole::Tool);
    }
    Ok(rule)
}

/// Binds the settled response of a text with no function call as the prior response of the next
/// RunStep, and opens that RunStep. The model reads its own text when it continues.
pub(crate) async fn open_after_text(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    run: &RunPhaseRow,
    attempt_id: &str,
    payload: &serde_json::Value,
) -> Result<WorkPhase, CompleteAgentRunError> {
    crate::agent_run_continuation::bind_produced_continuation(
        client, claim, attempt_id, payload, payload,
    )
    .await?;
    crate::agent_run_tool_result::open_next_step(client, claim, run).await
}

#[cfg(test)]
#[path = "agent_run_finish_rule_tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "agent_run_finish_rule_recovery_tests.rs"]
mod recovery_tests;
