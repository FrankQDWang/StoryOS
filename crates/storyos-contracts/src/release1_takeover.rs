use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{EditorWriterProjection, ProjectScope, QueryOperation};
use crate::release1_author_edit::DomainReceipt;

pub const TAKE_OVER_PROJECT_WRITER_REQUEST_SCHEMA_ID: &str =
    "storyos.command.take-over-project-writer.request.v1";
pub const TAKE_OVER_PROJECT_WRITER_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.take-over-project-writer.response.v1";

pub(super) const TAKE_OVER_PROJECT_WRITER: QueryOperation = QueryOperation {
    operation_id: "takeOverProjectWriter",
    method: "POST",
    path: "/api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers",
    request_schema: TAKE_OVER_PROJECT_WRITER_REQUEST_SCHEMA_ID,
    response_schema: TAKE_OVER_PROJECT_WRITER_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Committed writer takeover"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or authoritative Head conflict"),
        (412, "Session or writer binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Author command refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.takeOverProjectWriter.positive.v1",
        "storyos.golden.takeOverProjectWriter.invalid.v1",
        "storyos.golden.takeOverProjectWriter.boundary.v1",
    ],
};

pub const TAKE_OVER_PROJECT_WRITER_PATH: &str = TAKE_OVER_PROJECT_WRITER.path;
pub const TAKE_OVER_PROJECT_WRITER_METHOD: &str = TAKE_OVER_PROJECT_WRITER.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct TakeOverProjectWriterRequest {
    pub command_schema: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
    pub editor_session_id: String,
    pub observed_writer_generation: String,
    pub editor_contract_revision: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TakeoverCompareFailedReason {
    WriterGenerationAdvancedAfterAdmission,
    RequesterBecameCurrentAfterAdmission,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TakeOverProjectWriterResult {
    TakeoverApplied {
        prior_editor_session_id: String,
        prior_writer_generation: String,
        resulting_editor_session_id: String,
        resulting_writer_generation: String,
        resulting_snapshot_id: String,
        resulting_snapshot_activity_position: String,
        resulting_heads: Vec<String>,
    },
    TakeoverCompareFailed {
        observed_writer_generation: String,
        current_writer_generation: String,
        current_writer_projection: EditorWriterProjection,
        current_snapshot_id: String,
        current_snapshot_activity_position: String,
        current_heads: Vec<String>,
        reason: TakeoverCompareFailedReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct TakeOverProjectWriterResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub result: TakeOverProjectWriterResult,
}
