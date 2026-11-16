//! The Host fact that the Provider possibly no longer keeps the earlier turns of a Project
//! Conversation (ADR 0094 section 10, ADR 0053).

use std::time::SystemTime;

use crate::{AGENT_PLAN_STORED_RESPONSE_RETENTION, AgentRunStatus};

/// Gives true when the newest Run of a Conversation ended more than the Provider retention before
/// `read_time`. `last_recorded_at` is the recorded time of the last Run event of the newest Run,
/// or the time of its Receipt when it has no Run event. For an ended Run, it is the end time. A
/// newest Run that did not end gives false.
pub fn earlier_turns_expired(
    newest_run_status: AgentRunStatus,
    last_recorded_at: SystemTime,
    read_time: SystemTime,
) -> bool {
    let ended = matches!(
        newest_run_status,
        AgentRunStatus::Completed | AgentRunStatus::Refused | AgentRunStatus::Cancelled
    );
    ended
        && read_time
            .duration_since(last_recorded_at)
            .is_ok_and(|elapsed| elapsed > AGENT_PLAN_STORED_RESPONSE_RETENTION)
}

#[cfg(test)]
#[path = "conversation_retention_tests.rs"]
mod tests;
