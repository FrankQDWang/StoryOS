use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, ProjectCommandChallengeBinding, ProjectScope,
};
use std::future::Future;
use storyos_contracts::CloseEditorFlowDraftInput;

#[derive(Clone, Debug)]
pub struct CloseEditorFlowDraftCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub ids: AuthorCommandAdmissionIds,
    pub draft_id: String,
    pub input: CloseEditorFlowDraftInput,
}

#[derive(Clone, Debug)]
pub struct DraftCloseSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub correlation_id: String,
    pub result: storyos_core::CloseEditorFlowDraftResult,
    pub draft_revision_id: String,
    pub payload_digest: String,
    pub observed_closure: String,
    pub event_id: Option<String>,
    pub author_action_sequence: Option<String>,
    pub created_at: String,
}

#[derive(Debug)]
pub enum DraftCloseError {
    BindingConflict,
    InvalidChallenge,
    MissingDraft,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

/// Settles one exact admitted Draft Discard or returns its original acknowledgement.
pub trait CloseEditorFlowDraftStore: Sync {
    fn close_editor_flow_draft(
        &self,
        command: &CloseEditorFlowDraftCommand,
    ) -> impl Future<Output = Result<DraftCloseSettlement, DraftCloseError>> + Send;
}

pub async fn close_editor_flow_draft(
    store: &impl CloseEditorFlowDraftStore,
    command: &CloseEditorFlowDraftCommand,
) -> Result<DraftCloseSettlement, DraftCloseError> {
    let binding = &command.challenge_binding;
    let client = &command.client_binding;
    let digest = storyos_core::hex_sha256(&command.canonical_command_bytes);
    if binding.project_scope != command.project_scope
        || binding.client_session_binding_digest != client.binding_ref
        || binding.client_session_generation != client.session_generation
        || binding.client_contract_revision != client.client_contract_revision
        || binding.security_policy_revision != client.security_policy_revision
        || binding.command_kind != "closeEditorFlowDraft"
        || binding.command_schema != storyos_contracts::CLOSE_EDITOR_FLOW_DRAFT_REQUEST_SCHEMA_ID
        || binding.route_template != storyos_contracts::CLOSE_EDITOR_FLOW_DRAFT_PATH
        || binding.method != "POST"
        || binding.canonical_command_digest
            != format!("sha256:storyos.command.closeEditorFlowDraft.jcs.v1:{digest}")
        || command.input.draft_kind != "refused_edit"
        || command.input.expected_closure != "open"
        || command.input.close_reason != "abandoned"
    {
        return Err(DraftCloseError::BindingConflict);
    }
    store.close_editor_flow_draft(command).await
}
