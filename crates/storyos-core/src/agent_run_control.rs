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

/// Result of classifying one cancelAgentRun against the current Run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancelAgentRunResult {
    Applied,
    AlreadyCancelled,
    Terminal,
}

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
pub fn classify_cancel_agent_run(lifecycle: AgentRunLifecycle) -> CancelAgentRunResult {
    match lifecycle {
        AgentRunLifecycle::Queued
        | AgentRunLifecycle::Claimed
        | AgentRunLifecycle::Waiting
        | AgentRunLifecycle::Paused => CancelAgentRunResult::Applied,
        AgentRunLifecycle::Cancelled => CancelAgentRunResult::AlreadyCancelled,
        AgentRunLifecycle::Completed | AgentRunLifecycle::Refused => CancelAgentRunResult::Terminal,
    }
}
