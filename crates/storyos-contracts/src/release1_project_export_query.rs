use crate::release1::{ProjectScope, QueryOperation};
use crate::release1_snapshot::SnapshotDescriptor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const GET_EXPORT_OPERATION_REQUEST_SCHEMA_ID: &str =
    "storyos.query.export-operation.request.v1";
pub const GET_EXPORT_OPERATION_RESPONSE_SCHEMA_ID: &str =
    "storyos.query.export-operation.response.v1";

pub(super) const GET_EXPORT_OPERATION: QueryOperation = QueryOperation {
    operation_id: "getExportOperation",
    method: "GET",
    path: "/api/v1/projects/{project_id}/exports/{export_id}",
    request_schema: GET_EXPORT_OPERATION_REQUEST_SCHEMA_ID,
    response_schema: GET_EXPORT_OPERATION_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Admitted Project Export operation"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Snapshot expired"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Request refused"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.getExportOperation.positive.v1",
        "storyos.golden.getExportOperation.invalid.v1",
        "storyos.golden.getExportOperation.boundary.v1",
    ],
};

pub const GET_EXPORT_OPERATION_PATH: &str = GET_EXPORT_OPERATION.path;
pub const GET_EXPORT_OPERATION_METHOD: &str = GET_EXPORT_OPERATION.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ExportOperationStatus {
    InProgress,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetExportOperationResponse {
    pub schema_id: String,
    pub query_id: String,
    pub correlation_id: String,
    #[ts(type = "ProjectScope")]
    pub project_scope: ProjectScope,
    pub export_id: String,
    pub archive_profile: String,
    pub archive_path_profile: String,
    pub status: ExportOperationStatus,
    #[schemars(required)]
    pub immutable_root: Option<String>,
    #[ts(type = "SnapshotDescriptor")]
    pub source_snapshot: SnapshotDescriptor,
}
