//! The shared steps of the AgentRun control commands: pauseAgentRun, cancelAgentRun, and
//! steerAgentRun.

use storyos_application::{
    AgentRunControlRefusal, ProjectCommandError, ProjectScope, RefusableCommandError,
};
use storyos_core::AgentRunStatus;
use tokio_postgres::Client;

use crate::command_sequence::unavailable;

/// The most settlements of one control command that loses a serialization race with the Worker.
pub(crate) const CONTROL_SETTLEMENTS: usize = 3;

/// The AgentRun row that a control command locked.
pub(crate) struct LockedAgentRun {
    pub lifecycle: AgentRunStatus,
    pub conversation_id: String,
    pub author_message: String,
    pub active_decision_position: String,
}

/// The status that a pause or a cancel fences the Run at.
pub(crate) enum FencedStatus {
    Paused,
    Cancelled,
}

/// Locks the AgentRun row of the Project and parses its status. A missing AgentRun is a refusal
/// before Admission.
pub(crate) async fn lock_agent_run(
    client: &Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<LockedAgentRun, RefusableCommandError<AgentRunControlRefusal>> {
    let run = client
        .query_opt(
            "SELECT status, conversation_id::text, author_message,
                    active_decision_position::text
               FROM storyos.agent_runs
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
              FOR UPDATE",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(unavailable)?
        .ok_or(RefusableCommandError::RefusedBeforeAdmission(
            AgentRunControlRefusal::MissingRun,
        ))?;
    let status = run.get::<_, String>(/*idx*/ 0);
    let lifecycle = AgentRunStatus::parse(&status)
        .ok_or_else(|| unavailable(format!("unsupported AgentRun status {status}")))?;
    Ok(LockedAgentRun {
        lifecycle,
        conversation_id: run.get(/*idx*/ 1),
        author_message: run.get(/*idx*/ 2),
        active_decision_position: run.get(/*idx*/ 3),
    })
}

/// Sets the status of a locked AgentRun, advances its fence token, and clears its lease and its
/// wakeup. Gives the new fence generation.
pub(crate) async fn fence_agent_run(
    client: &Client,
    scope: &ProjectScope,
    run_id: &str,
    status: FencedStatus,
) -> Result<u64, ProjectCommandError> {
    let status = match status {
        FencedStatus::Paused => "paused",
        FencedStatus::Cancelled => "cancelled",
    };
    let fence_generation = client
        .query_one(
            "UPDATE storyos.agent_runs
                SET status = $4::text,
                    fence_token = fence_token + 1,
                    lease_expires_at = NULL,
                    wakeup_pending = false
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid
          RETURNING fence_token",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
                &status,
            ],
        )
        .await
        .map_err(unavailable)?
        .get::<_, i64>(/*idx*/ 0);
    u64::try_from(fence_generation).map_err(unavailable)
}

#[cfg(test)]
#[path = "agent_run_control_race_tests.rs"]
mod race_tests;
