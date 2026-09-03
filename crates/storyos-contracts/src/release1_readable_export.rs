use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, QueryOperation};
use crate::release1_snapshot::SnapshotDescriptor;

pub const EXPORT_HUMAN_READABLE_MANUSCRIPT_REQUEST_SCHEMA_ID: &str =
    "storyos.command.export-human-readable-manuscript.request.v1";
pub const EXPORT_HUMAN_READABLE_MANUSCRIPT_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.export-human-readable-manuscript.response.v1";
pub const EXPORT_HUMAN_READABLE_MANUSCRIPT_DIGEST_PROFILE: &str =
    "storyos.command.exportHumanReadableManuscript.jcs.v1";

pub(super) const EXPORT_HUMAN_READABLE_MANUSCRIPT: QueryOperation = QueryOperation {
    operation_id: "exportHumanReadableManuscript",
    method: "POST",
    path: "/api/v1/projects/{project_id}/manuscript/exports",
    request_schema: EXPORT_HUMAN_READABLE_MANUSCRIPT_REQUEST_SCHEMA_ID,
    response_schema: EXPORT_HUMAN_READABLE_MANUSCRIPT_RESPONSE_SCHEMA_ID,
    responses: &[
        (202, "Human-readable manuscript export admitted"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency conflict"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Export refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.exportHumanReadableManuscript.positive.v1",
        "storyos.golden.exportHumanReadableManuscript.invalid.v1",
        "storyos.golden.exportHumanReadableManuscript.boundary.v1",
    ],
};

pub const EXPORT_HUMAN_READABLE_MANUSCRIPT_PATH: &str = EXPORT_HUMAN_READABLE_MANUSCRIPT.path;
pub const EXPORT_HUMAN_READABLE_MANUSCRIPT_METHOD: &str = EXPORT_HUMAN_READABLE_MANUSCRIPT.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ExportHumanReadableManuscriptInput {
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ExportHumanReadableManuscriptRequest {
    pub command_schema: String,
    pub export_human_readable_manuscript_input: ExportHumanReadableManuscriptInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ExportHumanReadableManuscriptRefusalReason {
    ArchivedProject,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ExportAcknowledgement {
    Accepted,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HumanReadableManuscriptExportRef {
    HumanReadableManuscriptExport { export_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExportHumanReadableManuscriptEffect {
    Admitted {
        export_id: String,
        export_profile: String,
        #[ts(type = "SnapshotDescriptor")]
        source_snapshot: Box<SnapshotDescriptor>,
    },
    Refused {
        reason: ExportHumanReadableManuscriptRefusalReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ExportHumanReadableManuscriptResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub acknowledgement: ExportAcknowledgement,
    pub operation_ref: Option<HumanReadableManuscriptExportRef>,
    pub project: ControlledProject,
    pub effect: ExportHumanReadableManuscriptEffect,
}
