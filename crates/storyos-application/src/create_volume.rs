use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, ProjectCommandChallengeBinding, ProjectScope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateVolumeCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub title: String,
    pub expected_tree_revision: u64,
    pub ids: AuthorCommandAdmissionIds,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateVolumeSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub effect: CreateVolumeSettlementEffect,
    pub receipt_created_at: String,
    pub project_activity_position: u64,
    pub project_activity_event_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateVolumePublicOrder {
    CanonicalSiblingOrder(u64),
    HistoricalCreateVolumeAck,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateVolumeSettlementEffect {
    Applied {
        tree_revision: u64,
        volume_id: String,
        order: CreateVolumePublicOrder,
    },
    Conflicted {
        reason: storyos_core::CreateVolumeConflict,
    },
    Refused {
        reason: storyos_core::CreateVolumeRefusal,
    },
}

#[derive(Debug)]
pub enum CreateVolumeError {
    BindingConflict,
    InvalidChallenge,
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for CreateVolumeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The Create Volume binding conflicts"),
            Self::InvalidChallenge => formatter.write_str("The Create Volume challenge is invalid"),
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::Unavailable(_) => formatter.write_str("The Create Volume store is unavailable"),
        }
    }
}

impl std::error::Error for CreateVolumeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict | Self::InvalidChallenge | Self::MissingProject => None,
        }
    }
}

/// Owns one admitted Create Volume and its atomic Core settlement.
pub trait CreateVolumeStore: Sync {
    fn create_volume(
        &self,
        command: &CreateVolumeCommand,
    ) -> impl Future<Output = Result<CreateVolumeSettlement, CreateVolumeError>> + Send;
}

pub async fn create_volume(
    store: &impl CreateVolumeStore,
    command: &CreateVolumeCommand,
) -> Result<CreateVolumeSettlement, CreateVolumeError> {
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
        format!("sha256:storyos.command.createVolume.jcs.v1:{value}")
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "createVolume"
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "POST"
        || challenge.route_template != "/api/v1/projects/{project_id}/volumes"
        || challenge.command_schema != "storyos.command.create-volume.request.v1"
        || command.title.is_empty()
        || command.title.len() > 1024
        || command.expected_tree_revision < 1
    {
        return Err(CreateVolumeError::BindingConflict);
    }
    store.create_volume(command).await
}

#[cfg(test)]
#[path = "create_volume_tests.rs"]
mod tests;
