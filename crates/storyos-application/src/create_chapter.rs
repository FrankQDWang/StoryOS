use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, ProjectCommandChallengeBinding, ProjectScope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateChapterCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub volume_id: String,
    pub title: String,
    pub expected_tree_revision: u64,
    pub ids: AuthorCommandAdmissionIds,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateChapterSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub effect: CreateChapterSettlementEffect,
    pub receipt_created_at: String,
    pub project_activity_position: u64,
    pub project_activity_event_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateChapterSettlementEffect {
    Applied {
        tree_revision: u64,
        chapter_id: String,
        current: storyos_core::CreateChapterCurrent,
    },
    Conflicted {
        reason: storyos_core::CreateChapterConflict,
    },
    Refused {
        reason: storyos_core::CreateChapterRefusal,
    },
}

#[derive(Debug)]
pub enum CreateChapterError {
    BindingConflict,
    InvalidChallenge,
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for CreateChapterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The Create Chapter binding conflicts"),
            Self::InvalidChallenge => {
                formatter.write_str("The Create Chapter challenge is invalid")
            }
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::Unavailable(_) => formatter.write_str("The Create Chapter store is unavailable"),
        }
    }
}

impl std::error::Error for CreateChapterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict | Self::InvalidChallenge | Self::MissingProject => None,
        }
    }
}

/// Owns one admitted Create Chapter and its atomic Core settlement.
pub trait CreateChapterStore: Sync {
    fn create_chapter(
        &self,
        command: &CreateChapterCommand,
    ) -> impl Future<Output = Result<CreateChapterSettlement, CreateChapterError>> + Send;
}

pub async fn create_chapter(
    store: &impl CreateChapterStore,
    command: &CreateChapterCommand,
) -> Result<CreateChapterSettlement, CreateChapterError> {
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
        format!("sha256:storyos.command.createChapter.jcs.v1:{value}")
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "createChapter"
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "POST"
        || challenge.route_template != "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters"
        || challenge.command_schema != "storyos.command.create-chapter.request.v1"
        || command.title.is_empty()
        || command.title.len() > 1024
        || command.volume_id.is_empty()
        || command.expected_tree_revision < 1
    {
        return Err(CreateChapterError::BindingConflict);
    }
    store.create_chapter(command).await
}

#[cfg(test)]
#[path = "create_chapter_tests.rs"]
mod tests;
