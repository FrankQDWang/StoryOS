use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::QueryOperation;
use crate::{DomainReceipt, ProjectScope, RefusedEditDraftSource};

pub const CLOSE_EDITOR_FLOW_DRAFT_REQUEST_SCHEMA_ID: &str =
    "storyos.command.close-editor-flow-draft.request.v1";
pub const CLOSE_EDITOR_FLOW_DRAFT_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.close-editor-flow-draft.response.v1";
pub const CLOSE_EDITOR_FLOW_DRAFT_DIGEST_PROFILE: &str =
    "storyos.command.closeEditorFlowDraft.jcs.v1";
pub const EDITOR_FLOW_DRAFT_CLOSED_SCHEMA_ID: &str = "storyos.event.editor-flow-draft-closed.v1";
pub const CLOSE_EDITOR_FLOW_DRAFT_PATH: &str =
    "/api/v1/projects/{project_id}/drafts/{draft_id}/closures";
pub(super) const CLOSE_EDITOR_FLOW_DRAFT: QueryOperation = QueryOperation {
    operation_id: "closeEditorFlowDraft",
    method: "POST",
    path: CLOSE_EDITOR_FLOW_DRAFT_PATH,
    request_schema: CLOSE_EDITOR_FLOW_DRAFT_REQUEST_SCHEMA_ID,
    response_schema: CLOSE_EDITOR_FLOW_DRAFT_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Draft Discard settlement"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Binding conflict"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Challenge refused"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.closeEditorFlowDraft.positive.v1",
        "storyos.golden.closeEditorFlowDraft.invalid.v1",
        "storyos.golden.closeEditorFlowDraft.boundary.v1",
    ],
};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CloseEditorFlowDraftInput {
    pub draft_id: String,
    pub draft_kind: String,
    pub source_current_draft_revision_id: String,
    pub source_draft_payload_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_reopen_event_id: Option<String>,
    pub expected_closure: String,
    pub close_reason: String,
    pub editor_session_id: String,
    pub writer_generation: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CloseEditorFlowDraftRequest {
    pub command_schema: String,
    pub close_editor_flow_draft_input: CloseEditorFlowDraftInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct EditorFlowDraftClosed {
    pub schema_id: String,
    pub event_kind: String,
    pub event_id: String,
    pub project_scope: ProjectScope,
    pub draft_id: String,
    pub draft_revision_id: String,
    pub payload_digest: String,
    pub prior_closure: String,
    pub closure: String,
    pub close_reason: String,
    pub source: RefusedEditDraftSource,
    pub author_action_sequence: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CloseEditorFlowDraftEffect {
    DraftClosureChanged {
        event: Box<EditorFlowDraftClosed>,
    },
    Conflicted {
        current_revision_id: String,
        current_digest: String,
        current_closure: String,
    },
    Refused {
        reason: DraftCloseRefusal,
        current_closure: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DraftCloseRefusal {
    SourceDraftNotOpen,
    SourceUnavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CloseEditorFlowDraftResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub effect: CloseEditorFlowDraftEffect,
}
