use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding, ProjectScope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetCurrentChapterCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub ids: AuthorCommandAdmissionIds,
    pub editor_session_id: EditorSessionId,
    pub chapter_id: String,
    pub expected_current_chapter_id: String,
    pub expected_target_revision_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetCurrentChapterSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub effect: SetCurrentChapterSettlementEffect,
    pub receipt_created_at: String,
    pub project_activity_position: u64,
    pub project_activity_event_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SetCurrentChapterSettlementEffect {
    Applied {
        current_chapter_id: String,
        base_snapshot_id: String,
    },
    NoEffect {
        reason: storyos_core::SetCurrentChapterNoEffect,
    },
    Conflicted {
        reason: storyos_core::SetCurrentChapterConflict,
    },
    Refused {
        reason: storyos_core::SetCurrentChapterRefusal,
    },
}

#[derive(Debug)]
pub enum SetCurrentChapterError {
    BindingConflict,
    InvalidChallenge,
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for SetCurrentChapterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => {
                formatter.write_str("The Set Current Chapter binding conflicts")
            }
            Self::InvalidChallenge => {
                formatter.write_str("The Set Current Chapter challenge is invalid")
            }
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::Unavailable(_) => {
                formatter.write_str("The Set Current Chapter store is unavailable")
            }
        }
    }
}

impl std::error::Error for SetCurrentChapterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict | Self::InvalidChallenge | Self::MissingProject => None,
        }
    }
}

/// Owns one admitted Set Current Chapter and its atomic Core settlement.
pub trait SetCurrentChapterStore: Sync {
    fn set_current_chapter(
        &self,
        command: &SetCurrentChapterCommand,
    ) -> impl Future<Output = Result<SetCurrentChapterSettlement, SetCurrentChapterError>> + Send;
}

pub async fn set_current_chapter(
    store: &impl SetCurrentChapterStore,
    command: &SetCurrentChapterCommand,
) -> Result<SetCurrentChapterSettlement, SetCurrentChapterError> {
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
        format!("sha256:storyos.command.setCurrentChapter.jcs.v1:{value}")
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "setCurrentChapter"
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "PUT"
        || challenge.route_template != "/api/v1/projects/{project_id}/current-chapter"
        || challenge.command_schema != "storyos.command.set-current-chapter.request.v1"
        || command.chapter_id.is_empty()
        || command.expected_current_chapter_id.is_empty()
        || command.expected_target_revision_id.is_empty()
    {
        return Err(SetCurrentChapterError::BindingConflict);
    }
    store.set_current_chapter(command).await
}

#[cfg(test)]
#[path = "set_current_chapter_tests.rs"]
mod tests;
