use crate::release1::{ProjectScope, QueryOperation};
use crate::release1_snapshot::SnapshotDescriptor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_REQUEST_SCHEMA_ID: &str =
    "storyos.query.human-readable-manuscript-export.request.v1";
pub const GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_RESPONSE_SCHEMA_ID: &str =
    "storyos.query.human-readable-manuscript-export.response.v1";

pub(super) const GET_HUMAN_READABLE_MANUSCRIPT_EXPORT: QueryOperation = QueryOperation {
    operation_id: "getHumanReadableManuscriptExport",
    method: "GET",
    path: "/api/v1/projects/{project_id}/manuscript/exports/{export_id}",
    request_schema: GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_REQUEST_SCHEMA_ID,
    response_schema: GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Human-readable manuscript export status"),
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
        "storyos.golden.getHumanReadableManuscriptExport.positive.v1",
        "storyos.golden.getHumanReadableManuscriptExport.invalid.v1",
        "storyos.golden.getHumanReadableManuscriptExport.boundary.v1",
    ],
};

pub const GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_PATH: &str =
    GET_HUMAN_READABLE_MANUSCRIPT_EXPORT.path;
pub const GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_METHOD: &str =
    GET_HUMAN_READABLE_MANUSCRIPT_EXPORT.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum GetHumanReadableManuscriptExportResponse {
    InProgress {
        schema_id: String,
        query_id: String,
        correlation_id: String,
        #[ts(type = "ProjectScope")]
        project_scope: ProjectScope,
        export_id: String,
        export_profile: String,
        #[ts(type = "SnapshotDescriptor")]
        source_snapshot: SnapshotDescriptor,
    },
    Ready {
        schema_id: String,
        query_id: String,
        correlation_id: String,
        #[ts(type = "ProjectScope")]
        project_scope: ProjectScope,
        export_id: String,
        export_profile: String,
        content_sha256: String,
        manuscript_utf8: String,
        #[ts(type = "SnapshotDescriptor")]
        source_snapshot: SnapshotDescriptor,
    },
    Failed {
        schema_id: String,
        query_id: String,
        correlation_id: String,
        #[ts(type = "ProjectScope")]
        project_scope: ProjectScope,
        export_id: String,
        export_profile: String,
        #[ts(type = "SnapshotDescriptor")]
        source_snapshot: SnapshotDescriptor,
    },
    OutcomeUnknown {
        schema_id: String,
        query_id: String,
        correlation_id: String,
        #[ts(type = "ProjectScope")]
        project_scope: ProjectScope,
        export_id: String,
        export_profile: String,
        #[ts(type = "SnapshotDescriptor")]
        source_snapshot: SnapshotDescriptor,
    },
}
