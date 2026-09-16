use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, QueryOperation};
use crate::release1_author_edit::DomainReceipt;

pub const GET_PROJECT_ASSISTANCE_REQUEST_SCHEMA_ID: &str =
    "storyos.query.project-assistance.request.v1";
pub const GET_PROJECT_ASSISTANCE_RESPONSE_SCHEMA_ID: &str =
    "storyos.query.project-assistance.response.v1";
pub const UPDATE_PROJECT_ASSISTANCE_REQUEST_SCHEMA_ID: &str =
    "storyos.command.update-project-assistance.request.v1";
pub const UPDATE_PROJECT_ASSISTANCE_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.update-project-assistance.response.v1";
pub const UPDATE_PROJECT_ASSISTANCE_DIGEST_PROFILE: &str =
    "storyos.command.updateProjectAssistance.jcs.v1";

pub(super) const GET_PROJECT_ASSISTANCE: QueryOperation = QueryOperation {
    operation_id: "getProjectAssistance",
    method: "GET",
    path: "/api/v1/projects/{project_id}/assistance",
    request_schema: GET_PROJECT_ASSISTANCE_REQUEST_SCHEMA_ID,
    response_schema: GET_PROJECT_ASSISTANCE_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Project assistance binding"),
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
        "storyos.golden.getProjectAssistance.positive.v1",
        "storyos.golden.getProjectAssistance.invalid.v1",
        "storyos.golden.getProjectAssistance.boundary.v1",
    ],
};

pub(super) const UPDATE_PROJECT_ASSISTANCE: QueryOperation = QueryOperation {
    operation_id: "updateProjectAssistance",
    method: "PUT",
    path: "/api/v1/projects/{project_id}/assistance",
    request_schema: UPDATE_PROJECT_ASSISTANCE_REQUEST_SCHEMA_ID,
    response_schema: UPDATE_PROJECT_ASSISTANCE_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Project assistance prepared"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or assistance revision conflict"),
        (412, "Session binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Update Project Assistance refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.updateProjectAssistance.positive.v1",
        "storyos.golden.updateProjectAssistance.invalid.v1",
        "storyos.golden.updateProjectAssistance.boundary.v1",
    ],
};

pub const GET_PROJECT_ASSISTANCE_PATH: &str = GET_PROJECT_ASSISTANCE.path;
pub const GET_PROJECT_ASSISTANCE_METHOD: &str = GET_PROJECT_ASSISTANCE.method;
pub const UPDATE_PROJECT_ASSISTANCE_PATH: &str = UPDATE_PROJECT_ASSISTANCE.path;
pub const UPDATE_PROJECT_ASSISTANCE_METHOD: &str = UPDATE_PROJECT_ASSISTANCE.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProjectAssistanceAvailability {
    Available,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProjectAssistanceBinding {
    pub availability: ProjectAssistanceAvailability,
    pub revision: String,
    pub model_registration_revision: String,
    pub processing_destination_identity: String,
    pub processing_destination_identity_evidence_revision: String,
    pub project_model_use_binding_revision: String,
    pub external_compatibility_decision: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetProjectAssistanceResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub assistance: ProjectAssistanceBinding,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateProjectAssistanceInput {
    pub availability: ProjectAssistanceAvailability,
    pub expected_assistance_revision: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateProjectAssistanceRequest {
    pub command_schema: String,
    pub update_project_assistance_input: UpdateProjectAssistanceInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum UpdateProjectAssistanceNoEffectReason {
    AvailabilityUnchanged,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum UpdateProjectAssistanceConflictReason {
    StaleAssistanceRevision,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UpdateProjectAssistanceEffect {
    Initialized {
        availability: ProjectAssistanceAvailability,
        revision: String,
        project_activity_position: String,
    },
    AuthoritativeApplied {
        availability: ProjectAssistanceAvailability,
        revision: String,
        project_activity_position: String,
    },
    NoEffect {
        reason: UpdateProjectAssistanceNoEffectReason,
    },
    Conflicted {
        reason: UpdateProjectAssistanceConflictReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateProjectAssistanceResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub project: ControlledProject,
    pub assistance: ProjectAssistanceBinding,
    pub effect: UpdateProjectAssistanceEffect,
}
