use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, DraftCloseError, EditorClientBinding,
    ProjectCommandChallengeBinding, ProjectScope,
};
use storyos_contracts::ExpandRefusedEditDraftInput;

#[derive(Clone, Debug)]
pub struct ExpandRefusedEditDraftCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub ids: AuthorCommandAdmissionIds,
    pub draft_id: String,
    pub input: ExpandRefusedEditDraftInput,
}

#[derive(Clone, Debug)]
pub struct DraftExpansionSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub correlation_id: String,
    pub draft_revision_id: String,
    pub payload_digest: String,
    pub observed_closure: String,
    pub event_id: Option<String>,
    pub author_action_sequence: Option<String>,
    pub created_at: String,
    pub result: storyos_core::ExpandRefusedEditDraftResult,
    pub proposal_id: Option<String>,
    pub proposal_revision_id: Option<String>,
    pub current_target_revision_id: Option<String>,
}

/// Settles one whole retained Draft expansion or returns its exact durable result.
pub trait ExpandRefusedEditDraftStore: Sync {
    fn expand_refused_edit_draft(
        &self,
        command: &ExpandRefusedEditDraftCommand,
    ) -> impl Future<Output = Result<DraftExpansionSettlement, DraftCloseError>> + Send;
}

pub async fn expand_refused_edit_draft(
    store: &impl ExpandRefusedEditDraftStore,
    command: &ExpandRefusedEditDraftCommand,
) -> Result<DraftExpansionSettlement, DraftCloseError> {
    let binding = &command.challenge_binding;
    let client = &command.client_binding;
    let input = &command.input;
    let digest = storyos_core::hex_sha256(&command.canonical_command_bytes);
    if binding.project_scope != command.project_scope
        || binding.client_session_binding_digest != client.binding_ref
        || binding.client_session_generation != client.session_generation
        || binding.client_contract_revision != client.client_contract_revision
        || binding.security_policy_revision != client.security_policy_revision
        || binding.command_kind != "expandRefusedEditDraftToProposal"
        || binding.command_schema != storyos_contracts::EXPAND_REFUSED_EDIT_DRAFT_REQUEST_SCHEMA_ID
        || binding.route_template != storyos_contracts::EXPAND_REFUSED_EDIT_DRAFT_PATH
        || binding.method != "POST"
        || binding.canonical_command_digest
            != format!("sha256:storyos.command.expandRefusedEditDraftToProposal.jcs.v1:{digest}")
        || input.draft_id != command.draft_id
        || input.expected_source_draft_closure != "open"
        || input.proposal_kind != "inline_edit"
        || input.target_refs.len() != 1
        || input.expected_target_revisions.len() != 1
        || input.anchors.len() != 1
        || input.anchors[0].manuscript_block_id != input.target_refs[0]
        || input.anchors[0].base_authoritative_revision_id != input.expected_target_revisions[0]
    {
        return Err(DraftCloseError::BindingConflict);
    }
    store.expand_refused_edit_draft(command).await
}
