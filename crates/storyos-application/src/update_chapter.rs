use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, ChapterId, EditorClientBinding, ProjectCommandChallengeBinding,
    ProjectScope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateChapterCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub chapter_id: ChapterId,
    pub title: String,
    pub order: u64,
    pub expected_tree_revision: u64,
    pub ids: AuthorCommandAdmissionIds,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateChapterSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub effect: UpdateChapterSettlementEffect,
    pub receipt_created_at: String,
    pub project_activity_position: u64,
    pub project_activity_event_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateChapterSettlementEffect {
    Applied {
        title: String,
        order: u64,
        tree_revision: u64,
    },
    NoEffect {
        reason: storyos_core::UpdateChapterNoEffect,
    },
    Conflicted {
        reason: storyos_core::UpdateChapterConflict,
    },
    Refused {
        reason: storyos_core::UpdateChapterRefusal,
    },
}

#[derive(Debug)]
pub enum UpdateChapterError {
    BindingConflict,
    InvalidChallenge,
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for UpdateChapterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The Update Chapter binding conflicts"),
            Self::InvalidChallenge => {
                formatter.write_str("The Update Chapter challenge is invalid")
            }
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::Unavailable(_) => formatter.write_str("The Update Chapter store is unavailable"),
        }
    }
}

impl std::error::Error for UpdateChapterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict | Self::InvalidChallenge | Self::MissingProject => None,
        }
    }
}

/// Owns one admitted Update Chapter and its atomic Core settlement.
pub trait UpdateChapterStore: Sync {
    fn update_chapter(
        &self,
        command: &UpdateChapterCommand,
    ) -> impl Future<Output = Result<UpdateChapterSettlement, UpdateChapterError>> + Send;
}

pub async fn update_chapter(
    store: &impl UpdateChapterStore,
    command: &UpdateChapterCommand,
) -> Result<UpdateChapterSettlement, UpdateChapterError> {
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
        format!("sha256:storyos.command.updateChapter.jcs.v1:{value}")
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "updateChapter"
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "PATCH"
        || challenge.route_template != "/api/v1/projects/{project_id}/chapters/{chapter_id}"
        || challenge.command_schema != "storyos.command.update-chapter.request.v1"
        || command.title.is_empty()
        || command.title.len() > 1024
        || command.order < 1
        || command.expected_tree_revision < 1
    {
        return Err(UpdateChapterError::BindingConflict);
    }
    store.update_chapter(command).await
}

#[cfg(test)]
#[path = "update_chapter_tests.rs"]
mod tests;
