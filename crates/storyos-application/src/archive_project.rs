use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, ProjectCommandChallengeBinding, ProjectScope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveProjectCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub expected_revision: u64,
    pub ids: AuthorCommandAdmissionIds,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveProjectSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub effect: ArchiveProjectSettlementEffect,
    pub receipt_created_at: String,
    pub project_activity_position: u64,
    pub project_activity_event_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveProjectSettlementEffect {
    Applied {
        revision: u64,
    },
    NoEffect {
        reason: storyos_core::ArchiveProjectNoEffect,
    },
    Conflicted {
        reason: storyos_core::ArchiveProjectConflict,
    },
    Refused {
        reason: storyos_core::ArchiveProjectRefusal,
    },
}

#[derive(Debug)]
pub enum ArchiveProjectError {
    BindingConflict,
    InvalidChallenge,
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for ArchiveProjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The Archive Project binding conflicts"),
            Self::InvalidChallenge => {
                formatter.write_str("The Archive Project challenge is invalid")
            }
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::Unavailable(_) => formatter.write_str("The Archive Project store is unavailable"),
        }
    }
}

impl std::error::Error for ArchiveProjectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict | Self::InvalidChallenge | Self::MissingProject => None,
        }
    }
}

/// Owns one admitted Archive Project and its atomic Core settlement.
pub trait ArchiveProjectStore: Sync {
    fn archive_project(
        &self,
        command: &ArchiveProjectCommand,
    ) -> impl Future<Output = Result<ArchiveProjectSettlement, ArchiveProjectError>> + Send;
}

pub async fn archive_project(
    store: &impl ArchiveProjectStore,
    command: &ArchiveProjectCommand,
) -> Result<ArchiveProjectSettlement, ArchiveProjectError> {
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
        format!("sha256:storyos.command.archiveProject.jcs.v1:{value}")
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "archiveProject"
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "PUT"
        || challenge.route_template != "/api/v1/projects/{project_id}/archival"
        || challenge.command_schema != "storyos.command.archive-project.request.v1"
        || command.expected_revision < 1
    {
        return Err(ArchiveProjectError::BindingConflict);
    }
    store.archive_project(command).await
}

#[cfg(test)]
#[path = "archive_project_tests.rs"]
mod tests;
