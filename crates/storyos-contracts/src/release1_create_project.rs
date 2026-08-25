use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, DigestValue, ProjectScope, QueryOperation};
use crate::release1_author_edit::DomainReceipt;

pub const CREATE_PROJECT_CHALLENGE_REQUEST_SCHEMA_ID: &str =
    "storyos.challenge.create-project-anti-forgery-challenge.request.v1";
pub const CREATE_PROJECT_CHALLENGE_RESPONSE_SCHEMA_ID: &str =
    "storyos.challenge.create-project-anti-forgery-challenge.response.v1";
pub const CREATE_PROJECT_REQUEST_SCHEMA_ID: &str = "storyos.command.create-project.request.v1";
pub const CREATE_PROJECT_RESPONSE_SCHEMA_ID: &str = "storyos.command.create-project.response.v1";
pub const CREATE_PROJECT_DIGEST_PROFILE: &str = "storyos.command.createProject.jcs.v1";

pub(super) const CREATE_PROJECT_CHALLENGE: QueryOperation = QueryOperation {
    operation_id: "createProjectChallenge",
    method: "POST",
    path: "/api/v1/anti-forgery-challenges",
    request_schema: CREATE_PROJECT_CHALLENGE_REQUEST_SCHEMA_ID,
    response_schema: CREATE_PROJECT_CHALLENGE_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Prospective Project command challenge"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency binding conflict"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Command target refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
    ],
    fixtures: &[
        "storyos.golden.createProjectChallenge.positive.v1",
        "storyos.golden.createProjectChallenge.invalid.v1",
        "storyos.golden.createProjectChallenge.boundary.v1",
    ],
};

pub const CREATE_PROJECT_CHALLENGE_PATH: &str = CREATE_PROJECT_CHALLENGE.path;
pub const CREATE_PROJECT_CHALLENGE_METHOD: &str = CREATE_PROJECT_CHALLENGE.method;

pub(super) const CREATE_PROJECT: QueryOperation = QueryOperation {
    operation_id: "createProject",
    method: "POST",
    path: "/api/v1/projects",
    request_schema: CREATE_PROJECT_REQUEST_SCHEMA_ID,
    response_schema: CREATE_PROJECT_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Empty Project created"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or existing Project conflict"),
        (412, "Session binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Create Project refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.createProject.positive.v1",
        "storyos.golden.createProject.invalid.v1",
        "storyos.golden.createProject.boundary.v1",
    ],
};

pub const CREATE_PROJECT_PATH: &str = CREATE_PROJECT.path;
pub const CREATE_PROJECT_METHOD: &str = CREATE_PROJECT.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateProjectInput {
    pub title: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateProjectChallengeRequest {
    pub command_schema: String,
    pub create_project_input: CreateProjectInput,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateProjectChallengeResponse {
    pub prospective_project_id: String,
    pub canonical_command_digest: DigestValue,
    pub nonce: String,
    pub expires_at: String,
    pub limit_profile_revision: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateProjectRequest {
    pub command_schema: String,
    pub prospective_project_id: String,
    pub create_project_input: CreateProjectInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateProjectResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub project: ControlledProject,
}
