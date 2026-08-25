use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, CreateProjectChallengeBinding, EditorClientBinding, ProjectScope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateProjectCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: CreateProjectChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub title: String,
    pub ids: AuthorCommandAdmissionIds,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateProjectSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub title: String,
    pub receipt_created_at: String,
    pub project_activity_position: u64,
    pub project_activity_event_id: String,
}

#[derive(Debug)]
pub enum CreateProjectError {
    BindingConflict,
    InvalidChallenge,
    ExistingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for CreateProjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The Create Project binding conflicts"),
            Self::InvalidChallenge => {
                formatter.write_str("The Create Project challenge is invalid")
            }
            Self::ExistingProject => formatter.write_str("The prospective Project already exists"),
            Self::Unavailable(_) => formatter.write_str("The Create Project store is unavailable"),
        }
    }
}

impl std::error::Error for CreateProjectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict | Self::InvalidChallenge | Self::ExistingProject => None,
        }
    }
}

/// Owns one admitted Create Project and its atomic Core settlement.
pub trait CreateProjectStore: Sync {
    fn create_project(
        &self,
        command: &CreateProjectCommand,
    ) -> impl Future<Output = Result<CreateProjectSettlement, CreateProjectError>> + Send;
}

pub async fn create_project(
    store: &impl CreateProjectStore,
    command: &CreateProjectCommand,
) -> Result<CreateProjectSettlement, CreateProjectError> {
    let challenge = &command.challenge_binding;
    if challenge.owner_user_id != command.project_scope.owner_user_id
        || challenge.prospective_project_id != command.project_scope.project_id
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "createProject"
        || challenge.method != "POST"
        || challenge.route_template != "/api/v1/projects"
        || challenge.command_schema != "storyos.command.create-project.request.v1"
        || command.title.is_empty()
        || command.title.len() > 1024
    {
        return Err(CreateProjectError::BindingConflict);
    }
    store.create_project(command).await
}

#[cfg(test)]
#[path = "create_project_tests.rs"]
mod tests;
