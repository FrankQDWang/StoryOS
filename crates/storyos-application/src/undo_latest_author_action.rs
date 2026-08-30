use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding, ProjectScope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UndoLatestAuthorActionCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub ids: AuthorCommandAdmissionIds,
    pub editor_session_id: EditorSessionId,
    pub expected_author_undo_frontier_sequence: u64,
    pub expected_authoritative_revision_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UndoLatestAuthorActionSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub effect: UndoLatestAuthorActionSettlementEffect,
    pub receipt_created_at: String,
    pub project_activity_position: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UndoLatestAuthorActionSettlementEffect {
    Compensated {
        source_sequence: u64,
        author_action_sequence: u64,
        authoritative_commit_id: String,
        revision_id: String,
        body: String,
        blocks: Vec<crate::ManuscriptBlock>,
        author_undo_frontier_sequence: Option<u64>,
    },
    Conflicted {
        reason: storyos_core::UndoLatestAuthorActionConflict,
    },
    Unavailable {
        reason: storyos_core::UndoLatestAuthorActionUnavailable,
    },
}

#[derive(Debug)]
pub enum UndoLatestAuthorActionError {
    BindingConflict,
    InvalidChallenge,
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for UndoLatestAuthorActionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => {
                formatter.write_str("The Undo Latest Author Action binding conflicts")
            }
            Self::InvalidChallenge => {
                formatter.write_str("The Undo Latest Author Action challenge is invalid")
            }
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::Unavailable(_) => {
                formatter.write_str("The Undo Latest Author Action store is unavailable")
            }
        }
    }
}

impl std::error::Error for UndoLatestAuthorActionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict | Self::InvalidChallenge | Self::MissingProject => None,
        }
    }
}

/// Owns one admitted Undo Latest Author Action and its atomic Core settlement.
pub trait UndoLatestAuthorActionStore: Sync {
    fn undo_latest_author_action(
        &self,
        command: &UndoLatestAuthorActionCommand,
    ) -> impl Future<Output = Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError>> + Send;
}

pub async fn undo_latest_author_action(
    store: &impl UndoLatestAuthorActionStore,
    command: &UndoLatestAuthorActionCommand,
) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
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
        format!("sha256:storyos.command.undoLatestAuthorAction.jcs.v1:{value}")
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "undoLatestAuthorAction"
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "POST"
        || challenge.route_template != "/api/v1/projects/{project_id}/author-actions/undo"
        || challenge.command_schema != "storyos.command.undo-latest-author-action.request.v1"
        || command.expected_authoritative_revision_id.is_empty()
        || command.expected_author_undo_frontier_sequence == 0
    {
        return Err(UndoLatestAuthorActionError::BindingConflict);
    }
    store.undo_latest_author_action(command).await
}

#[cfg(test)]
#[path = "undo_latest_author_action_tests.rs"]
mod tests;
