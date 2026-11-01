//! Pure Core classification for AgentRun control.

use std::convert::Infallible;

use super::TransitionOutcome;
use crate::transition_outcome::reason_codes;

/// The durable status of one AgentRun.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRunStatus {
    Queued,
    Claimed,
    Waiting,
    Paused,
    Completed,
    Refused,
    Cancelled,
}

impl AgentRunStatus {
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
    lifecycle: AgentRunStatus,
) -> TransitionOutcome<(), PauseAgentRunNoEffect, PauseAgentRunConflict, Infallible> {
    match lifecycle {
        AgentRunStatus::Queued | AgentRunStatus::Claimed | AgentRunStatus::Waiting => {
            TransitionOutcome::Applied(())
        }
        AgentRunStatus::Paused => TransitionOutcome::NoEffect(PauseAgentRunNoEffect::AlreadyPaused),
        AgentRunStatus::Completed | AgentRunStatus::Refused | AgentRunStatus::Cancelled => {
            TransitionOutcome::Conflicted(PauseAgentRunConflict::TerminalRun)
        }
    }
}

/// Classifies cancel as the irreversible fence, including a paused Run.
pub fn classify_cancel_agent_run(
    lifecycle: AgentRunStatus,
) -> TransitionOutcome<(), CancelAgentRunNoEffect, CancelAgentRunConflict, Infallible> {
    match lifecycle {
        AgentRunStatus::Queued
        | AgentRunStatus::Claimed
        | AgentRunStatus::Waiting
        | AgentRunStatus::Paused => TransitionOutcome::Applied(()),
        AgentRunStatus::Cancelled => {
            TransitionOutcome::NoEffect(CancelAgentRunNoEffect::AlreadyCancelled)
        }
        AgentRunStatus::Completed | AgentRunStatus::Refused => {
            TransitionOutcome::Conflicted(CancelAgentRunConflict::TerminalRun)
        }
    }
}

/// Classifies steering. A Run that is not terminal retains the input and changes no authority.
pub fn classify_steer_agent_run(
    lifecycle: AgentRunStatus,
) -> TransitionOutcome<Infallible, SteerAgentRunNoEffect, SteerAgentRunConflict, Infallible> {
    match lifecycle {
        AgentRunStatus::Queued
        | AgentRunStatus::Claimed
        | AgentRunStatus::Waiting
        | AgentRunStatus::Paused => {
            TransitionOutcome::NoEffect(SteerAgentRunNoEffect::SteeringRetained)
        }
        AgentRunStatus::Completed | AgentRunStatus::Refused | AgentRunStatus::Cancelled => {
            TransitionOutcome::Conflicted(SteerAgentRunConflict::TerminalRun)
        }
    }
}

#[cfg(test)]
#[path = "agent_run_control_tests.rs"]
mod tests;
