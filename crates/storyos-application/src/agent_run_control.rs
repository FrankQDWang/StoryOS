use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, Project, ProjectCommandChallengeBinding,
    ProjectScope,
};

/// One admitted pause or cancel command for an existing AgentRun.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunControlCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub run_id: String,
    pub ids: AuthorCommandAdmissionIds,
    pub intent: AgentRunControlIntent,
}

/// Distinguishes pause from cancel at the Application boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRunControlIntent {
    Pause,
    Cancel,
}

/// Settlement of one pause or cancel command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunControlSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub receipt_created_at: String,
    pub project_activity_position: u64,
    pub response_project: Project,
    pub effect: AgentRunControlEffect,
}

/// Observable effect of one pause or cancel command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentRunControlEffect {
    Applied {
        run_id: String,
        status: AgentRunControlStatus,
        fence_generation: u64,
    },
    NoEffect {
        reason: AgentRunControlNoEffect,
    },
    Conflicted {
        reason: AgentRunControlConflict,
    },
}

/// Terminal or paused status written by a control command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRunControlStatus {
    Paused,
    Cancelled,
}

/// Zero-effect reason that is not a terminal conflict.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRunControlNoEffect {
    AlreadyPaused,
    AlreadyCancelled,
}

/// Conflict that leaves the current terminal Run unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRunControlConflict {
    TerminalRun,
}

/// Failure while admitting pause or cancel.
#[derive(Debug)]
pub enum AgentRunControlError {
    BindingConflict,
    HistoricalAcknowledgementUnavailable,
    InvalidChallenge,
    MissingProject,
    MissingRun,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for AgentRunControlError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The AgentRun control binding conflicts"),
            Self::HistoricalAcknowledgementUnavailable => formatter
                .write_str("The original AgentRun control acknowledgement cannot be recovered"),
            Self::InvalidChallenge => {
                formatter.write_str("The AgentRun control challenge is invalid")
            }
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::MissingRun => formatter.write_str("The AgentRun is not in exact Scope"),
            Self::Unavailable(_) => {
                formatter.write_str("The AgentRun control store is unavailable")
            }
        }
    }
}

impl std::error::Error for AgentRunControlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict
            | Self::HistoricalAcknowledgementUnavailable
            | Self::InvalidChallenge
            | Self::MissingProject
            | Self::MissingRun => None,
        }
    }
}

/// Owns one admitted pause or cancel and its durable fence.
pub trait AgentRunControlStore: Sync {
    fn control_agent_run(
        &self,
        command: &AgentRunControlCommand,
    ) -> impl Future<Output = Result<AgentRunControlSettlement, AgentRunControlError>> + Send;
}

pub async fn control_agent_run(
    store: &impl AgentRunControlStore,
    command: &AgentRunControlCommand,
) -> Result<AgentRunControlSettlement, AgentRunControlError> {
    let challenge = &command.challenge_binding;
    let (command_kind, method, route, schema, digest_profile) = match command.intent {
        AgentRunControlIntent::Pause => (
            "pauseAgentRun",
            "POST",
            "/api/v1/projects/{project_id}/agent-runs/{run_id}/pause",
            "storyos.command.pause-agent-run.request.v1",
            "storyos.command.pauseAgentRun.jcs.v1",
        ),
        AgentRunControlIntent::Cancel => (
            "cancelAgentRun",
            "POST",
            "/api/v1/projects/{project_id}/agent-runs/{run_id}/cancel",
            "storyos.command.cancel-agent-run.request.v1",
            "storyos.command.cancelAgentRun.jcs.v1",
        ),
    };
    let command_digest = {
        use sha2::{Digest as _, Sha256};
        let value = Sha256::digest(&command.canonical_command_bytes)
            .iter()
            .fold(String::with_capacity(64), |mut value, byte| {
                use std::fmt::Write as _;
                write!(value, "{byte:02x}").expect("writing to String cannot fail");
                value
            });
        format!("sha256:{digest_profile}:{value}")
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != command_kind
        || challenge.canonical_command_digest != command_digest
        || challenge.method != method
        || challenge.route_template != route
        || challenge.command_schema != schema
    {
        return Err(AgentRunControlError::BindingConflict);
    }
    store.control_agent_run(command).await
}
