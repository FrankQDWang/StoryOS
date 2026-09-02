use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, ProjectCommandChallengeBinding, ProjectScope,
    VolumeId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateVolumeCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub volume_id: VolumeId,
    pub title: String,
    pub order: u64,
    pub expected_tree_revision: u64,
    pub ids: AuthorCommandAdmissionIds,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateVolumeSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub effect: UpdateVolumeSettlementEffect,
    pub receipt_created_at: String,
    pub project_activity_position: u64,
    pub project_activity_event_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateVolumeSettlementEffect {
    Applied {
        title: String,
        order: u64,
        tree_revision: u64,
    },
    NoEffect {
        reason: storyos_core::UpdateVolumeNoEffect,
    },
    Conflicted {
        reason: storyos_core::UpdateVolumeConflict,
    },
    Refused {
        reason: storyos_core::UpdateVolumeRefusal,
    },
}

#[derive(Debug)]
pub enum UpdateVolumeError {
    BindingConflict,
    InvalidChallenge,
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for UpdateVolumeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The Update Volume binding conflicts"),
            Self::InvalidChallenge => formatter.write_str("The Update Volume challenge is invalid"),
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::Unavailable(_) => formatter.write_str("The Update Volume store is unavailable"),
        }
    }
}

impl std::error::Error for UpdateVolumeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict | Self::InvalidChallenge | Self::MissingProject => None,
        }
    }
}

/// Owns one admitted Update Volume and its atomic Core settlement.
pub trait UpdateVolumeStore: Sync {
    fn update_volume(
        &self,
        command: &UpdateVolumeCommand,
    ) -> impl Future<Output = Result<UpdateVolumeSettlement, UpdateVolumeError>> + Send;
}

pub async fn update_volume(
    store: &impl UpdateVolumeStore,
    command: &UpdateVolumeCommand,
) -> Result<UpdateVolumeSettlement, UpdateVolumeError> {
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
        format!("sha256:storyos.command.updateVolume.jcs.v1:{value}")
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "updateVolume"
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "PATCH"
        || challenge.route_template != "/api/v1/projects/{project_id}/volumes/{volume_id}"
        || challenge.command_schema != "storyos.command.update-volume.request.v1"
        || command.title.is_empty()
        || command.title.len() > 1024
        || command.order < 1
        || command.expected_tree_revision < 1
    {
        return Err(UpdateVolumeError::BindingConflict);
    }
    store.update_volume(command).await
}

#[cfg(test)]
#[path = "update_volume_tests.rs"]
mod tests;
