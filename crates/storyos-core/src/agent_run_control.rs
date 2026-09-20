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

/// Result of classifying one pauseAgentRun against the current Run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PauseAgentRunResult {
    Applied,
    AlreadyPaused,
    Terminal,
}

/// Result of classifying one cancelAgentRun against the current Run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancelAgentRunResult {
    Applied,
    AlreadyCancelled,
    Terminal,
}

/// Classifies pause without treating it as cancellation.
pub fn classify_pause_agent_run(lifecycle: AgentRunLifecycle) -> PauseAgentRunResult {
    match lifecycle {
        AgentRunLifecycle::Queued | AgentRunLifecycle::Claimed | AgentRunLifecycle::Waiting => {
            PauseAgentRunResult::Applied
        }
        AgentRunLifecycle::Paused => PauseAgentRunResult::AlreadyPaused,
        AgentRunLifecycle::Completed
        | AgentRunLifecycle::Refused
        | AgentRunLifecycle::Cancelled => PauseAgentRunResult::Terminal,
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
