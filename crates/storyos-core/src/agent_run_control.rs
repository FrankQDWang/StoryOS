//! Pure Core classification for AgentRun control.

use std::convert::Infallible;

use super::TransitionOutcome;
use crate::transition_outcome::reason_codes;

/// Current lifecycle of one AgentRun for pause and cancel classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRunLifecycle {
    Queued,
    Claimed,
    Waiting,
    Paused,
    Completed,
    Refused,
    Cancelled,
}

impl AgentRunLifecycle {
    /// Parses one durable AgentRun status token.
    pub fn parse(status: &str) -> Option<Self> {
        Some(match status {
            "queued" => Self::Queued,
            "claimed" => Self::Claimed,
            "waiting" => Self::Waiting,
            "paused" => Self::Paused,
            "completed" => Self::Completed,
            "refused" => Self::Refused,
            "cancelled" => Self::Cancelled,
            _ => return None,
        })
    }
}

/// The `no_effect` reason of a pauseAgentRun.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PauseAgentRunNoEffect {
    AlreadyPaused,
}

/// The `conflicted` reason of a pauseAgentRun.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PauseAgentRunConflict {
    TerminalRun,
}

reason_codes!(PauseAgentRunNoEffect { AlreadyPaused => "already_paused" });
reason_codes!(PauseAgentRunConflict { TerminalRun => "terminal_run" });

/// The `no_effect` reason of a cancelAgentRun.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancelAgentRunNoEffect {
    AlreadyCancelled,
}

/// The `conflicted` reason of a cancelAgentRun.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancelAgentRunConflict {
    TerminalRun,
}

reason_codes!(CancelAgentRunNoEffect { AlreadyCancelled => "already_cancelled" });
reason_codes!(CancelAgentRunConflict { TerminalRun => "terminal_run" });

/// The `no_effect` reason of a steerAgentRun.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SteerAgentRunNoEffect {
    SteeringRetained,
}

/// The `conflicted` reason of a steerAgentRun.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SteerAgentRunConflict {
    TerminalRun,
}

reason_codes!(SteerAgentRunNoEffect { SteeringRetained => "steering_retained" });
reason_codes!(SteerAgentRunConflict { TerminalRun => "terminal_run" });

/// Classifies pause without treating it as cancellation.
pub fn classify_pause_agent_run(
    lifecycle: AgentRunLifecycle,
) -> TransitionOutcome<(), PauseAgentRunNoEffect, PauseAgentRunConflict, Infallible> {
    match lifecycle {
        AgentRunLifecycle::Queued | AgentRunLifecycle::Claimed | AgentRunLifecycle::Waiting => {
            TransitionOutcome::Applied(())
        }
        AgentRunLifecycle::Paused => {
            TransitionOutcome::NoEffect(PauseAgentRunNoEffect::AlreadyPaused)
        }
        AgentRunLifecycle::Completed
        | AgentRunLifecycle::Refused
        | AgentRunLifecycle::Cancelled => {
            TransitionOutcome::Conflicted(PauseAgentRunConflict::TerminalRun)
        }
    }
}

/// Classifies cancel as the irreversible fence, including a paused Run.
pub fn classify_cancel_agent_run(
    lifecycle: AgentRunLifecycle,
) -> TransitionOutcome<(), CancelAgentRunNoEffect, CancelAgentRunConflict, Infallible> {
    match lifecycle {
        AgentRunLifecycle::Queued
        | AgentRunLifecycle::Claimed
        | AgentRunLifecycle::Waiting
        | AgentRunLifecycle::Paused => TransitionOutcome::Applied(()),
        AgentRunLifecycle::Cancelled => {
            TransitionOutcome::NoEffect(CancelAgentRunNoEffect::AlreadyCancelled)
        }
        AgentRunLifecycle::Completed | AgentRunLifecycle::Refused => {
            TransitionOutcome::Conflicted(CancelAgentRunConflict::TerminalRun)
        }
    }
}

/// Classifies steering. A Run that is not terminal retains the input and changes no authority.
pub fn classify_steer_agent_run(
    lifecycle: AgentRunLifecycle,
) -> TransitionOutcome<Infallible, SteerAgentRunNoEffect, SteerAgentRunConflict, Infallible> {
    match lifecycle {
        AgentRunLifecycle::Queued
        | AgentRunLifecycle::Claimed
        | AgentRunLifecycle::Waiting
        | AgentRunLifecycle::Paused => {
            TransitionOutcome::NoEffect(SteerAgentRunNoEffect::SteeringRetained)
        }
        AgentRunLifecycle::Completed
        | AgentRunLifecycle::Refused
        | AgentRunLifecycle::Cancelled => {
            TransitionOutcome::Conflicted(SteerAgentRunConflict::TerminalRun)
        }
    }
}

#[cfg(test)]
#[path = "agent_run_control_tests.rs"]
mod tests;
