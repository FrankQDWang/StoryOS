use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding, ProjectScope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TakeOverProjectWriterCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub ids: AuthorCommandAdmissionIds,
    pub editor_session_id: EditorSessionId,
    pub observed_writer_generation: u64,
    pub editor_contract_revision: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TakeOverProjectWriterSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub receipt_created_at: String,
    pub expected_heads: Vec<String>,
    pub prior_heads: Vec<String>,
    pub resulting_heads: Vec<String>,
    pub effect: TakeOverProjectWriterEffect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TakeOverProjectWriterEffect {
    TakeoverApplied {
        prior_editor_session_id: String,
        prior_writer_generation: u64,
        resulting_editor_session_id: String,
        resulting_writer_generation: u64,
        resulting_snapshot_id: String,
        resulting_snapshot_activity_position: u64,
    },
}

#[derive(Debug)]
pub enum TakeOverProjectWriterError {
    BindingConflict,
    InvalidChallenge,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for TakeOverProjectWriterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The writer takeover binding conflicts"),
            Self::InvalidChallenge => {
                formatter.write_str("The writer takeover challenge is invalid")
            }
            Self::Unavailable(_) => formatter.write_str("The writer takeover store is unavailable"),
        }
    }
}

impl std::error::Error for TakeOverProjectWriterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict | Self::InvalidChallenge => None,
        }
    }
}

/// Owns one admitted writer takeover and its atomic settlement.
pub trait TakeOverProjectWriterStore: Sync {
    fn take_over_project_writer(
        &self,
        command: &TakeOverProjectWriterCommand,
    ) -> impl Future<Output = Result<TakeOverProjectWriterSettlement, TakeOverProjectWriterError>> + Send;
}

pub async fn take_over_project_writer(
    store: &impl TakeOverProjectWriterStore,
    command: &TakeOverProjectWriterCommand,
) -> Result<TakeOverProjectWriterSettlement, TakeOverProjectWriterError> {
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
        format!("sha256:storyos.command.takeOverProjectWriter.jcs.v1:{value}")
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "takeOverProjectWriter"
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "POST"
        || challenge.route_template
            != "/api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers"
        || challenge.command_schema != "storyos.command.take-over-project-writer.request.v1"
        || command.editor_contract_revision != "storyos.editor-contract.release-1.v2"
    {
        return Err(TakeOverProjectWriterError::BindingConflict);
    }
    store.take_over_project_writer(command).await
}

#[cfg(test)]
#[path = "takeover_tests.rs"]
mod tests;
