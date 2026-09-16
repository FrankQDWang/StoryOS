use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, Project, ProjectCommandChallengeBinding,
    ProjectScope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConversationSelection {
    New,
    Existing { conversation_id: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateAgentRunCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub conversation: ConversationSelection,
    pub author_message: String,
    pub chapter_id: String,
    pub ids: AuthorCommandAdmissionIds,
    pub run_id: String,
    pub conversation_id: String,
    pub project_agent_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunRecord {
    pub project_agent_id: String,
    pub conversation_id: String,
    pub memory_settings_revision: String,
    pub run_id: String,
    pub status: AgentRunStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRunStatus {
    Queued,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateAgentRunAdmission {
    pub ids: AuthorCommandAdmissionIds,
    pub project_agent_id: String,
    pub conversation_id: String,
    pub memory_settings_revision: String,
    pub run_id: String,
    pub project_activity_position: u64,
    pub response_project: Project,
}

#[derive(Debug)]
pub enum CreateAgentRunError {
    AssistanceUnavailable,
    ArchivedProject,
    BindingConflict,
    ConversationBusy,
    HistoricalAcknowledgementUnavailable,
    InaccessibleConversation,
    InvalidChapterJoin,
    InvalidChallenge,
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for CreateAgentRunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AssistanceUnavailable => formatter.write_str("Project assistance is unavailable"),
            Self::ArchivedProject => formatter.write_str("The Project is archived"),
            Self::BindingConflict => formatter.write_str("The createAgentRun binding conflicts"),
            Self::ConversationBusy => {
                formatter.write_str("The Project Conversation already has a non-terminal Run")
            }
            Self::HistoricalAcknowledgementUnavailable => formatter
                .write_str("The original createAgentRun acknowledgement cannot be recovered"),
            Self::InaccessibleConversation => {
                formatter.write_str("The Project Conversation is not in exact Scope")
            }
            Self::InvalidChapterJoin => {
                formatter.write_str("The Working Target Chapter is invalid")
            }
            Self::InvalidChallenge => {
                formatter.write_str("The createAgentRun challenge is invalid")
            }
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::Unavailable(_) => formatter.write_str("The createAgentRun store is unavailable"),
        }
    }
}

impl std::error::Error for CreateAgentRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::AssistanceUnavailable
            | Self::ArchivedProject
            | Self::BindingConflict
            | Self::ConversationBusy
            | Self::HistoricalAcknowledgementUnavailable
            | Self::InaccessibleConversation
            | Self::InvalidChapterJoin
            | Self::InvalidChallenge
            | Self::MissingProject => None,
        }
    }
}

/// Owns one admitted createAgentRun and its atomic Conversation/settings/Run settlement.
pub trait CreateAgentRunStore: Sync {
    fn create_agent_run(
        &self,
        command: &CreateAgentRunCommand,
    ) -> impl Future<Output = Result<CreateAgentRunAdmission, CreateAgentRunError>> + Send;

    fn read_agent_run(
        &self,
        scope: &ProjectScope,
        run_id: &str,
    ) -> impl Future<Output = Result<Option<AgentRunRecord>, CreateAgentRunError>> + Send;
}

pub async fn request_create_agent_run(
    store: &impl CreateAgentRunStore,
    command: &CreateAgentRunCommand,
) -> Result<CreateAgentRunAdmission, CreateAgentRunError> {
    let challenge = &command.challenge_binding;
    let command_digest = {
        use sha2::{Digest as _, Sha256};
        let value = Sha256::digest(&command.canonical_command_bytes)
            .iter()
            .fold(String::with_capacity(64), |mut value, byte| {
                use std::fmt::Write as _;
                write!(value, "{byte:02x}").expect("writing to String cannot fail");
                value
            });
        format!("sha256:storyos.command.createAgentRun.jcs.v1:{value}")
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "createAgentRun"
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "POST"
        || challenge.route_template != "/api/v1/projects/{project_id}/agent-runs"
        || challenge.command_schema != "storyos.command.create-agent-run.request.v2"
        || command.author_message.is_empty()
        || command.author_message.len() > 8000
    {
        return Err(CreateAgentRunError::BindingConflict);
    }
    store.create_agent_run(command).await
}

pub async fn open_agent_run(
    store: &impl CreateAgentRunStore,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Option<AgentRunRecord>, CreateAgentRunError> {
    store.read_agent_run(scope, run_id).await
}

#[cfg(test)]
#[path = "create_agent_run_tests.rs"]
mod tests;
