use super::{
    AgentRunStatus, SteerAgentRunConflict, SteerAgentRunNoEffect, classify_steer_agent_run,
};
use crate::TransitionOutcome;

#[test]
fn steering_of_a_run_that_is_not_terminal_is_retained_without_effect() {
    for lifecycle in [
        AgentRunStatus::Queued,
        AgentRunStatus::Claimed,
        AgentRunStatus::Waiting,
        AgentRunStatus::Paused,
    ] {
        assert_eq!(
            classify_steer_agent_run(lifecycle),
            TransitionOutcome::NoEffect(SteerAgentRunNoEffect::SteeringRetained)
        );
    }
}

#[test]
fn steering_of_a_terminal_run_conflicts() {
    for lifecycle in [
        AgentRunStatus::Completed,
        AgentRunStatus::Refused,
        AgentRunStatus::Cancelled,
    ] {
        assert_eq!(
            classify_steer_agent_run(lifecycle),
            TransitionOutcome::Conflicted(SteerAgentRunConflict::TerminalRun)
        );
    }
}
