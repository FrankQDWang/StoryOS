use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, QueryOperation};
use crate::release1_author_edit::DomainReceipt;

pub const ARCHIVE_PROJECT_REQUEST_SCHEMA_ID: &str = "storyos.command.archive-project.request.v1";
pub const ARCHIVE_PROJECT_RESPONSE_SCHEMA_ID: &str = "storyos.command.archive-project.response.v1";
pub const ARCHIVE_PROJECT_DIGEST_PROFILE: &str = "storyos.command.archiveProject.jcs.v1";

pub(super) const ARCHIVE_PROJECT: QueryOperation = QueryOperation {
    operation_id: "archiveProject",
    method: "PUT",
    path: "/api/v1/projects/{project_id}/archival",
    request_schema: ARCHIVE_PROJECT_REQUEST_SCHEMA_ID,
    response_schema: ARCHIVE_PROJECT_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Project archived"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or Project revision conflict"),
        (412, "Session binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Archive Project refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.archiveProject.positive.v1",
        "storyos.golden.archiveProject.invalid.v1",
        "storyos.golden.archiveProject.boundary.v1",
    ],
};

pub const ARCHIVE_PROJECT_PATH: &str = ARCHIVE_PROJECT.path;
pub const ARCHIVE_PROJECT_METHOD: &str = ARCHIVE_PROJECT.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ArchiveProjectInput {
    pub expected_project_revision: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ArchiveProjectRequest {
    pub command_schema: String,
    pub archive_project_input: ArchiveProjectInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveProjectNoEffectReason {
    AlreadyArchived,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveProjectConflictReason {
    StaleProjectRevision,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ArchiveProjectEffect {
    AuthoritativeApplied {
        revision: String,
        project_activity_position: String,
    },
    NoEffect {
        reason: ArchiveProjectNoEffectReason,
    },
    Conflicted {
        reason: ArchiveProjectConflictReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ArchiveProjectResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub project: ControlledProject,
    pub effect: ArchiveProjectEffect,
}
