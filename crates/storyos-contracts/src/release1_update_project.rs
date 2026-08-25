use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, QueryOperation};
use crate::release1_author_edit::DomainReceipt;

pub const UPDATE_PROJECT_REQUEST_SCHEMA_ID: &str = "storyos.command.update-project.request.v1";
pub const UPDATE_PROJECT_RESPONSE_SCHEMA_ID: &str = "storyos.command.update-project.response.v1";
pub const UPDATE_PROJECT_DIGEST_PROFILE: &str = "storyos.command.updateProject.jcs.v1";

pub(super) const UPDATE_PROJECT: QueryOperation = QueryOperation {
    operation_id: "updateProject",
    method: "PATCH",
    path: "/api/v1/projects/{project_id}",
    request_schema: UPDATE_PROJECT_REQUEST_SCHEMA_ID,
    response_schema: UPDATE_PROJECT_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Project renamed"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or Project revision conflict"),
        (412, "Session binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Update Project refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.updateProject.positive.v1",
        "storyos.golden.updateProject.invalid.v1",
        "storyos.golden.updateProject.boundary.v1",
    ],
};

pub const UPDATE_PROJECT_PATH: &str = UPDATE_PROJECT.path;
pub const UPDATE_PROJECT_METHOD: &str = UPDATE_PROJECT.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateProjectInput {
    pub title: String,
    pub expected_project_revision: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateProjectRequest {
    pub command_schema: String,
    pub update_project_input: UpdateProjectInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum UpdateProjectNoEffectReason {
    TitleUnchanged,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum UpdateProjectConflictReason {
    StaleProjectRevision,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UpdateProjectEffect {
    AuthoritativeApplied {
        title: String,
        revision: String,
        project_activity_position: String,
    },
    NoEffect {
        reason: UpdateProjectNoEffectReason,
    },
    Conflicted {
        reason: UpdateProjectConflictReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateProjectResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub project: ControlledProject,
    pub effect: UpdateProjectEffect,
}
