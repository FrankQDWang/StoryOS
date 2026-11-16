//! The wire forms of the Project Instruction query and its save command (ADR 0094 section 2).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, HttpMethod, QueryOperation};
use crate::release1_author_edit::DomainReceipt;

pub const GET_PROJECT_INSTRUCTION_REQUEST_SCHEMA_ID: &str =
    "storyos.query.project-instruction.request.v1";
pub const GET_PROJECT_INSTRUCTION_RESPONSE_SCHEMA_ID: &str =
    "storyos.query.project-instruction.response.v1";
pub const UPDATE_PROJECT_INSTRUCTION_REQUEST_SCHEMA_ID: &str =
    "storyos.command.update-project-instruction.request.v1";
pub const UPDATE_PROJECT_INSTRUCTION_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.update-project-instruction.response.v1";
pub const UPDATE_PROJECT_INSTRUCTION_DIGEST_PROFILE: &str =
    "storyos.command.updateProjectInstruction.jcs.v1";

pub(super) const GET_PROJECT_INSTRUCTION: QueryOperation = QueryOperation {
    operation_id: "getProjectInstruction",
    method: HttpMethod::Get,
    path: "/api/v1/projects/{project_id}/instruction",
    request_schema: GET_PROJECT_INSTRUCTION_REQUEST_SCHEMA_ID,
    response_schema: GET_PROJECT_INSTRUCTION_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Current Project Instruction Revision"),
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
    fixture: "storyos.golden.getProjectInstruction.positive.v1",
};

pub(super) const UPDATE_PROJECT_INSTRUCTION: QueryOperation = QueryOperation {
    operation_id: "updateProjectInstruction",
    method: HttpMethod::Put,
    path: "/api/v1/projects/{project_id}/instruction",
    request_schema: UPDATE_PROJECT_INSTRUCTION_REQUEST_SCHEMA_ID,
    response_schema: UPDATE_PROJECT_INSTRUCTION_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Project Instruction saved"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency binding conflict"),
        (412, "Session binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Update Project Instruction refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixture: "storyos.golden.updateProjectInstruction.positive.v1",
};

pub const GET_PROJECT_INSTRUCTION_PATH: &str = GET_PROJECT_INSTRUCTION.path;
pub const GET_PROJECT_INSTRUCTION_METHOD: &str = GET_PROJECT_INSTRUCTION.method.as_str();
pub const UPDATE_PROJECT_INSTRUCTION_PATH: &str = UPDATE_PROJECT_INSTRUCTION.path;
pub const UPDATE_PROJECT_INSTRUCTION_METHOD: &str = UPDATE_PROJECT_INSTRUCTION.method.as_str();

/// One immutable Revision of the Project Instruction with its exact text.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProjectInstruction {
    pub instruction_revision_id: String,
    pub revision: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetProjectInstructionResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    #[schemars(required)]
    pub instruction: Option<ProjectInstruction>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateProjectInstructionInput {
    pub text: String,
    pub expected_instruction_revision: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateProjectInstructionRequest {
    pub command_schema: String,
    pub update_project_instruction_input: UpdateProjectInstructionInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum UpdateProjectInstructionNoEffectReason {
    TextUnchanged,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum UpdateProjectInstructionConflictReason {
    StaleInstructionRevision,
}

/// The effect of one save. An applied save gives the new Revision without its text.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UpdateProjectInstructionEffect {
    AuthoritativeApplied {
        instruction_revision_id: String,
        revision: String,
        project_activity_position: String,
    },
    NoEffect {
        reason: UpdateProjectInstructionNoEffectReason,
    },
    Conflicted {
        reason: UpdateProjectInstructionConflictReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct UpdateProjectInstructionResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub project: ControlledProject,
    pub effect: UpdateProjectInstructionEffect,
}
