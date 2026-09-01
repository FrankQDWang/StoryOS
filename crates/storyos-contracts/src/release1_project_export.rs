use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, QueryOperation};
use crate::release1_author_edit::DomainReceipt;
use crate::release1_readable_export::ExportAcknowledgement;

pub const EXPORT_PROJECT_ARCHIVE_REQUEST_SCHEMA_ID: &str =
    "storyos.command.export-project-archive.request.v1";
pub const EXPORT_PROJECT_ARCHIVE_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.export-project-archive.response.v1";
pub const EXPORT_PROJECT_ARCHIVE_DIGEST_PROFILE: &str =
    "storyos.command.exportProjectArchive.jcs.v1";

pub(super) const EXPORT_PROJECT_ARCHIVE: QueryOperation = QueryOperation {
    operation_id: "exportProjectArchive",
    method: "POST",
    path: "/api/v1/projects/{project_id}/exports",
    request_schema: EXPORT_PROJECT_ARCHIVE_REQUEST_SCHEMA_ID,
    response_schema: EXPORT_PROJECT_ARCHIVE_RESPONSE_SCHEMA_ID,
    responses: &[
        (202, "Project Export Archive operation accepted"),
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
        "storyos.golden.exportProjectArchive.positive.v1",
        "storyos.golden.exportProjectArchive.invalid.v1",
        "storyos.golden.exportProjectArchive.boundary.v1",
    ],
};

pub const EXPORT_PROJECT_ARCHIVE_PATH: &str = EXPORT_PROJECT_ARCHIVE.path;
pub const EXPORT_PROJECT_ARCHIVE_METHOD: &str = EXPORT_PROJECT_ARCHIVE.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ExportProjectArchiveInput {
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
    pub archive_profile: String,
    pub archive_path_profile: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ExportProjectArchiveRequest {
    pub command_schema: String,
    pub export_project_archive_input: ExportProjectArchiveInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ExportProjectArchiveRefusalReason {
    ArchivedProject,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectExportRef {
    ProjectExport { export_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExportProjectArchiveEffect {
    Admitted {
        export_id: String,
        archive_profile: String,
        archive_path_profile: String,
        project_activity_position: String,
    },
    Refused {
        reason: ExportProjectArchiveRefusalReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ExportProjectArchiveResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub acknowledgement: ExportAcknowledgement,
    pub operation_ref: Option<ProjectExportRef>,
    pub receipt: DomainReceipt,
    pub project: ControlledProject,
    pub effect: ExportProjectArchiveEffect,
}
