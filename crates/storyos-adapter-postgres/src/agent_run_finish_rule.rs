//! The records of the finish rule of ADR 0091 section 4: the `tool_choice` of the active RunStep
//! from the ToolCalls of the RunStep before it, and the next RunStep after a text that came
//! before a `finish` call that counted.

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
    if !storyos_core::model_registration(&run.model_registration_revision)
        .is_some_and(storyos_core::ModelRegistration::declares_host_tools)
    {
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
            Some(PriorStepCall {
                operation: storyos_core::tool_spec(row.get(/*idx*/ 0))?.operation,
                state: ToolCallState::parse(row.get(/*idx*/ 1))?,
            })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| damaged("The ToolCall is damaged"))?;
    Ok(storyos_core::step_tool_choice(
        /*declares_host_tools*/ true,
        &prior_step,
    ))
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
