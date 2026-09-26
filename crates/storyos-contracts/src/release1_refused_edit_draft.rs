use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::QueryOperation;
use crate::{DigestValue, ProjectScope, RefusedEditPayload};

pub const GET_REFUSED_EDIT_DRAFT_REQUEST_SCHEMA_ID: &str =
    "storyos.query.refused-edit-draft.request.v1";
pub const GET_REFUSED_EDIT_DRAFT_RESPONSE_SCHEMA_ID: &str =
    "storyos.query.refused-edit-draft.response.v1";
pub const REFUSED_EDIT_DRAFT_CREATED_SCHEMA_ID: &str =
    "storyos.event.refused-edit-draft-created.v1";

pub(super) const GET_REFUSED_EDIT_DRAFT: QueryOperation = QueryOperation {
    operation_id: "getRefusedEditDraft",
    method: "GET",
    path: "/api/v1/projects/{project_id}/refused-edit-drafts/{draft_id}",
    request_schema: GET_REFUSED_EDIT_DRAFT_REQUEST_SCHEMA_ID,
    response_schema: GET_REFUSED_EDIT_DRAFT_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Retained Refused Edit Draft"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Active release conflict"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Request refused"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.getRefusedEditDraft.positive.v1",
        "storyos.golden.getRefusedEditDraft.invalid.v1",
        "storyos.golden.getRefusedEditDraft.boundary.v1",
    ],
};
pub const GET_REFUSED_EDIT_DRAFT_PATH: &str = GET_REFUSED_EDIT_DRAFT.path;
pub const GET_REFUSED_EDIT_DRAFT_METHOD: &str = GET_REFUSED_EDIT_DRAFT.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RefusedEditDraftSource {
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt_id: String,
    pub idempotency_key: String,
    pub command_digest: DigestValue,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RefusedEditDraftCreated {
    pub event_kind: String,
    pub project_scope: ProjectScope,
    pub creator: RefusedEditDraftCreator,
    pub schema_id: String,
    pub creation_event_id: String,
    pub draft_id: String,
    pub draft_revision_id: String,
    pub created_at: String,
    pub source: RefusedEditDraftSource,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RefusedEditDraftInspect {
    pub draft_id: String,
    pub draft_revision_id: String,
    pub kind: String,
    pub closure: String,
    pub retention_state: String,
    pub payload: RefusedEditPayload,
    pub payload_digest: String,
    pub payload_digest_profile: String,
    pub creation: RefusedEditDraftCreated,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closure_event: Option<crate::EditorFlowDraftClosed>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetRefusedEditDraftResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: ProjectScope,
    pub draft: RefusedEditDraftInspect,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RefusedEditDraftCreator {
    CoreTransition { receipt_id: String },
}
