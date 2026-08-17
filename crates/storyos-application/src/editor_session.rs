use std::future::Future;

use crate::{ProjectCommandChallengeBinding, ProjectScope};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorSessionId(String);

impl EditorSessionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl AsRef<str> for EditorSessionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorClientBinding {
    pub binding_ref: String,
    pub session_generation: u64,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenEditorSession {
    pub project_scope: ProjectScope,
    pub editor_session_id: EditorSessionId,
    pub snapshot_id: String,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorSessionLookup {
    pub project_scope: ProjectScope,
    pub editor_session_id: EditorSessionId,
    pub client_binding: EditorClientBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditorWriterState {
    CurrentWriter { writer_generation: u64 },
    ReadOnly { observed_writer_generation: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorSessionSnapshot {
    pub snapshot_id: String,
    pub chapter_id: String,
    pub authoritative_revision_id: String,
    pub project_activity_position: u64,
    pub body: String,
    pub payload_digest_hex: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorSession {
    pub editor_session_id: EditorSessionId,
    pub client_binding: EditorClientBinding,
    pub opened_at: String,
    pub writer: EditorWriterState,
    pub base_snapshot: EditorSessionSnapshot,
}

#[derive(Debug)]
pub enum EditorSessionError {
    BindingConflict,
    InvalidChallenge,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for EditorSessionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The Editor Session binding conflicts"),
            Self::InvalidChallenge => {
                formatter.write_str("The Editor Session challenge is invalid")
            }
            Self::Unavailable(_) => formatter.write_str("The Editor Session store is unavailable"),
        }
    }
}

impl std::error::Error for EditorSessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict | Self::InvalidChallenge => None,
        }
    }
}

/// Owns exact-Scope Editor Session creation and lookup.
pub trait EditorSessionStore: Sync {
    fn create_editor_session(
        &self,
        request: &OpenEditorSession,
    ) -> impl Future<Output = Result<EditorSession, EditorSessionError>> + Send;

    fn read_editor_session(
        &self,
        request: &EditorSessionLookup,
    ) -> impl Future<Output = Result<Option<EditorSession>, EditorSessionError>> + Send;
}

pub async fn create_editor_session(
    store: &impl EditorSessionStore,
    request: &OpenEditorSession,
) -> Result<EditorSession, EditorSessionError> {
    let challenge = &request.challenge_binding;
    if challenge.project_scope != request.project_scope
        || challenge.client_session_binding_digest != request.client_binding.binding_ref
        || challenge.client_session_generation != request.client_binding.session_generation
        || challenge.client_contract_revision != request.client_binding.client_contract_revision
        || challenge.security_policy_revision != request.client_binding.security_policy_revision
    {
        return Err(EditorSessionError::BindingConflict);
    }
    store.create_editor_session(request).await
}

pub async fn get_editor_session(
    store: &impl EditorSessionStore,
    request: &EditorSessionLookup,
) -> Result<Option<EditorSession>, EditorSessionError> {
    store.read_editor_session(request).await
}

#[cfg(test)]
#[path = "editor_session_tests.rs"]
mod tests;
