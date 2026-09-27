use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::QueryOperation;
use crate::{DomainReceipt, EditorFlowDraftClosed, ProjectScope, ProposalAnchorInspect};

pub const EXPAND_REFUSED_EDIT_DRAFT_REQUEST_SCHEMA_ID: &str =
    "storyos.command.expand-refused-edit-draft-to-proposal.request.v1";
pub const EXPAND_REFUSED_EDIT_DRAFT_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.expand-refused-edit-draft-to-proposal.response.v1";
pub const EXPAND_REFUSED_EDIT_DRAFT_DIGEST_PROFILE: &str =
    "storyos.command.expandRefusedEditDraftToProposal.jcs.v1";
pub const EXPAND_REFUSED_EDIT_DRAFT_PATH: &str =
    "/api/v1/projects/{project_id}/drafts/{draft_id}/proposal-expansions";
pub(super) const EXPAND_REFUSED_EDIT_DRAFT: QueryOperation = QueryOperation {
    operation_id: "expandRefusedEditDraftToProposal",
    method: "POST",
    path: EXPAND_REFUSED_EDIT_DRAFT_PATH,
    request_schema: EXPAND_REFUSED_EDIT_DRAFT_REQUEST_SCHEMA_ID,
    response_schema: EXPAND_REFUSED_EDIT_DRAFT_RESPONSE_SCHEMA_ID,
    responses: super::release1_close_editor_flow_draft::CLOSE_EDITOR_FLOW_DRAFT.responses,
    fixtures: &[
        "storyos.golden.expandRefusedEditDraftToProposal.positive.v1",
        "storyos.golden.expandRefusedEditDraftToProposal.invalid.v1",
        "storyos.golden.expandRefusedEditDraftToProposal.boundary.v1",
    ],
};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WholeDraftPayload {
    WholeDraftPayload,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ExpandRefusedEditDraftInput {
    pub draft_id: String,
    pub source_current_draft_revision_id: String,
    pub source_draft_payload_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_reopen_event_id: Option<String>,
    pub expected_source_draft_closure: String,
    pub selected_payload_range: WholeDraftPayload,
    pub proposal_kind: String,
    pub chapter_id: String,
    pub target_refs: Vec<String>,
    pub expected_target_revisions: Vec<String>,
    pub anchors: Vec<ProposalAnchorInspect>,
    pub editor_session_id: String,
    pub writer_generation: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ExpandRefusedEditDraftRequest {
    pub command_schema: String,
    pub expand_refused_edit_draft_to_proposal_input: ExpandRefusedEditDraftInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExpandRefusedEditDraftEffect {
    ProposalCreatedFromDraft {
        proposal_id: String,
        proposal_revision_id: String,
        event: Box<EditorFlowDraftClosed>,
    },
    Conflicted {
        current_revision_id: String,
        current_digest: String,
        current_closure: String,
    },
    Refused {
        reason: String,
        current_closure: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ExpandRefusedEditDraftResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub effect: ExpandRefusedEditDraftEffect,
}
