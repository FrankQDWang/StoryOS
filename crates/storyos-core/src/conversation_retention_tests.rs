use std::time::{Duration, SystemTime};

use super::earlier_turns_expired;
use crate::{AGENT_PLAN_STORED_RESPONSE_RETENTION, AgentRunStatus};

fn end() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(/*secs*/ 1_800_000_000)
}

#[test]
fn the_retention_of_the_agent_plan_route_is_72_hours() {
    assert_eq!(
        AGENT_PLAN_STORED_RESPONSE_RETENTION,
        Duration::from_secs(/*secs*/ 72 * 60 * 60)
    );
}

#[test]
fn earlier_turns_expire_only_after_more_than_the_retention() {
    let millisecond = Duration::from_millis(/*millis*/ 1);
    let at = |elapsed: Duration| {
        earlier_turns_expired(AgentRunStatus::Completed, Some(end()), end() + elapsed)
    };

    assert_eq!(
        [
            at(Duration::ZERO),
            at(AGENT_PLAN_STORED_RESPONSE_RETENTION - millisecond),
            at(AGENT_PLAN_STORED_RESPONSE_RETENTION),
            at(AGENT_PLAN_STORED_RESPONSE_RETENTION + millisecond),
        ],
        [false, false, false, true]
    );
}

#[test]
fn each_ended_status_counts_and_a_newest_run_that_did_not_end_keeps_the_earlier_turns() {
    let read = end() + AGENT_PLAN_STORED_RESPONSE_RETENTION + Duration::from_secs(/*secs*/ 1);
    let statuses = [
        AgentRunStatus::Queued,
        AgentRunStatus::Claimed,
        AgentRunStatus::Waiting,
        AgentRunStatus::Paused,
        AgentRunStatus::Completed,
        AgentRunStatus::Refused,
        AgentRunStatus::Cancelled,
    ];

    assert_eq!(
        statuses.map(|status| earlier_turns_expired(status, Some(end()), read)),
        [false, false, false, false, true, true, true]
    );
}

#[test]
fn no_run_event_or_an_event_after_the_read_keeps_the_earlier_turns() {
    let read = end() + AGENT_PLAN_STORED_RESPONSE_RETENTION + Duration::from_secs(/*secs*/ 1);
    let later = read + Duration::from_secs(/*secs*/ 1);

    assert_eq!(
        [None, Some(later)].map(|event| earlier_turns_expired(
            AgentRunStatus::Cancelled,
            event,
            read
        )),
        [false, false]
    );
}
