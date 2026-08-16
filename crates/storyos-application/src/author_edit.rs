use std::future::Future;

use storyos_core::{
    AuthorEditConflict, AuthorEditNoEffect, AuthorEditRefusal, AuthorEditUnit,
    MAX_AUTHOR_EDIT_UNITS, MAX_NORMALIZED_PRIMITIVES,
};

use crate::{EditorClientBinding, EditorSessionId, ProjectCommandChallengeBinding, ProjectScope};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorCommandAdmissionIds {
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt_id: String,
    pub revision_id: String,
    pub payload_id: String,
    pub authoritative_commit_id: String,
    pub project_activity_event_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplyAuthorEditCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub ids: AuthorCommandAdmissionIds,
    pub editor_session_id: EditorSessionId,
    pub writer_generation: u64,
    pub chapter_id: String,
    pub expected_authoritative_revision_id: String,
    pub expected_proposal_head_revision_ids: Vec<String>,
    pub target_refs: Vec<String>,
    pub observed_ownership_partition: String,
    pub editor_contract_revision: String,
    pub undo_group_id: String,
    pub completed_intent_record_id: String,
    pub local_intent_sequence: u64,
    pub author_edit_units: Vec<AuthorEditUnit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorEditSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub effect: AuthorEditSettlementEffect,
    pub project_activity_position: u64,
    pub receipt_created_at: String,
    pub completed_intent_record_id: String,
    pub local_intent_sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorEditSettlementEffect {
    AuthoritativeApplied {
        body: String,
        author_action_sequence: u64,
    },
    NoEffect {
        reason: AuthorEditNoEffect,
    },
    Conflicted {
        reason: AuthorEditConflict,
        current_authoritative_revision_id: String,
    },
    Refused {
        reason: AuthorEditRefusal,
    },
}

#[derive(Debug)]
pub enum AuthorEditError {
    BindingConflict,
    InvalidChallenge,
    StaleWriter,
    AdmissionExpired,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for AuthorEditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The Author Edit binding conflicts"),
            Self::InvalidChallenge => formatter.write_str("The Author Edit challenge is invalid"),
            Self::StaleWriter => {
                formatter.write_str("The Editor Session is not the current writer")
            }
            Self::AdmissionExpired => formatter.write_str("The Author Command Admission expired"),
            Self::Unavailable(_) => formatter.write_str("The Author Edit store is unavailable"),
        }
    }
}

impl std::error::Error for AuthorEditError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict
            | Self::InvalidChallenge
            | Self::StaleWriter
            | Self::AdmissionExpired => None,
        }
    }
}

/// Owns one admitted Author Edit and its atomic Core settlement.
pub trait AuthorEditStore: Sync {
    fn apply_author_edit(
        &self,
        command: &ApplyAuthorEditCommand,
    ) -> impl Future<Output = Result<AuthorEditSettlement, AuthorEditError>> + Send;
}

pub async fn apply_author_edit(
    store: &impl AuthorEditStore,
    command: &ApplyAuthorEditCommand,
) -> Result<AuthorEditSettlement, AuthorEditError> {
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
        format!("sha256:storyos.command.applyAuthorEdit.jcs.v1:{value}")
    };
    let primitive_count = command
        .author_edit_units
        .iter()
        .try_fold(0_usize, |count, unit| {
            count.checked_add(unit.normalized_primitives.len())
        });
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "applyAuthorEdit"
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "POST"
        || challenge.route_template != "/api/v1/projects/{project_id}/manuscript/author-edits"
        || challenge.command_schema != "storyos.command.apply-author-edit.request.v1"
        || command.editor_contract_revision != "storyos.editor-contract.release-1.v2"
        || command.author_edit_units.is_empty()
        || command.author_edit_units.len() > MAX_AUTHOR_EDIT_UNITS
        || primitive_count.is_none_or(|count| count > MAX_NORMALIZED_PRIMITIVES)
    {
        return Err(AuthorEditError::BindingConflict);
    }
    store.apply_author_edit(command).await
}

#[cfg(test)]
#[path = "author_edit_tests.rs"]
mod tests;
