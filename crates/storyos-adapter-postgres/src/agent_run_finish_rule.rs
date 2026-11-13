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
    if !declares_host_tools(run) {
        return Ok(ToolChoice::Auto);
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
        /*declares_host_tools*/ true,
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

fn declares_host_tools(run: &RunPhaseRow) -> bool {
    storyos_core::model_registration(&run.model_registration_revision)
        .is_some_and(storyos_core::ModelRegistration::declares_host_tools)
}

/// Leaves out the function calls of a response that a recovery path settles in the last RunStep
/// of `run`, as the finish rule does for a created response (ADR 0091 section 5).
pub(crate) fn drop_last_step_calls(
    run: &RunPhaseRow,
    items: &mut Vec<storyos_core::NativeStreamItem>,
) -> Result<(), CompleteAgentRunError> {
    if !run_step_limit(run)?.executes_calls(declares_host_tools(run)) {
        items.retain(|item| item.role != storyos_core::StreamItemRole::Tool);
    }
    Ok(())
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
mod tests;
