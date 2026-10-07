use super::{
    AgentRunLifecycle, SteerAgentRunConflict, SteerAgentRunNoEffect, classify_steer_agent_run,
};
use crate::TransitionOutcome;

#[test]
fn steering_of_a_run_that_is_not_terminal_is_retained_without_effect() {
    for lifecycle in [
        AgentRunLifecycle::Queued,
        AgentRunLifecycle::Claimed,
        AgentRunLifecycle::Waiting,
        AgentRunLifecycle::Paused,
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
        AgentRunLifecycle::Completed,
        AgentRunLifecycle::Refused,
        AgentRunLifecycle::Cancelled,
    ] {
        assert_eq!(
            classify_steer_agent_run(lifecycle),
            TransitionOutcome::Conflicted(SteerAgentRunConflict::TerminalRun)
        );
    }
}
