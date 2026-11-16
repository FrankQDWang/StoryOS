//! The Host fact that the Provider possibly no longer keeps the earlier turns of a Project
//! Conversation (ADR 0094 section 10, ADR 0053).

use std::time::SystemTime;

use crate::{AGENT_PLAN_STORED_RESPONSE_RETENTION, AgentRunStatus};

/// Gives true when the newest Run of a Conversation is in a terminal state and the newest Run
/// event of the Conversation, over all its Runs, is older than the Provider retention at
/// `read_time`. A Run that ended before its first Tool call has no Run event, so the newest event
/// of an earlier Run decides. A Conversation with no Run event (`None`) gives false.
pub fn earlier_turns_expired(
    newest_run_status: AgentRunStatus,
    newest_event_at: Option<SystemTime>,
    read_time: SystemTime,
) -> bool {
    let ended = matches!(
        newest_run_status,
        AgentRunStatus::Completed | AgentRunStatus::Refused | AgentRunStatus::Cancelled
    );
    ended
        && newest_event_at.is_some_and(|event| {
            read_time
                .duration_since(event)
                .is_ok_and(|elapsed| elapsed > AGENT_PLAN_STORED_RESPONSE_RETENTION)
        })
}

#[cfg(test)]
#[path = "conversation_retention_tests.rs"]
mod tests;
