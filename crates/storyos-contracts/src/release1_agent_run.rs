use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, QueryOperation};
use crate::release1_readable_export::ExportAcknowledgement;

pub const CREATE_AGENT_RUN_REQUEST_SCHEMA_ID: &str = "storyos.command.create-agent-run.request.v2";
pub const CREATE_AGENT_RUN_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.create-agent-run.response.v2";
pub const CREATE_AGENT_RUN_DIGEST_PROFILE: &str = "storyos.command.createAgentRun.jcs.v1";
pub const GET_AGENT_RUN_REQUEST_SCHEMA_ID: &str = "storyos.query.agent-run.request.v2";
pub const GET_AGENT_RUN_RESPONSE_SCHEMA_ID: &str = "storyos.query.agent-run.response.v2";

pub(super) const CREATE_AGENT_RUN: QueryOperation = QueryOperation {
    operation_id: "createAgentRun",
    method: "POST",
    path: "/api/v1/projects/{project_id}/agent-runs",
    request_schema: CREATE_AGENT_RUN_REQUEST_SCHEMA_ID,
    response_schema: CREATE_AGENT_RUN_RESPONSE_SCHEMA_ID,
    responses: &[
        (202, "Bounded assistance request accepted"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or conversation conflict"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Assistance request refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.createAgentRun.positive.v1",
        "storyos.golden.createAgentRun.invalid.v1",
        "storyos.golden.createAgentRun.boundary.v1",
    ],
};

pub(super) const GET_AGENT_RUN: QueryOperation = QueryOperation {
    operation_id: "getAgentRun",
    method: "GET",
    path: "/api/v1/projects/{project_id}/agent-runs/{run_id}",
    request_schema: GET_AGENT_RUN_REQUEST_SCHEMA_ID,
    response_schema: GET_AGENT_RUN_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Durable AgentRun status"),
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
        "storyos.golden.getAgentRun.positive.v1",
        "storyos.golden.getAgentRun.invalid.v1",
        "storyos.golden.getAgentRun.boundary.v1",
    ],
};

pub const CREATE_AGENT_RUN_PATH: &str = CREATE_AGENT_RUN.path;
pub const CREATE_AGENT_RUN_METHOD: &str = CREATE_AGENT_RUN.method;
pub const GET_AGENT_RUN_PATH: &str = GET_AGENT_RUN.path;
pub const GET_AGENT_RUN_METHOD: &str = GET_AGENT_RUN.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConversationSelection {
    New,
    Existing { conversation_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AuthorMessage {
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssistanceWorkingTarget {
    CurrentChapter { chapter_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InstructionBinding {
    Absent,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssistanceCause {
    AuthorRequest,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateAgentRunInput {
    pub conversation: ConversationSelection,
    pub author_message: AuthorMessage,
    pub working_target: AssistanceWorkingTarget,
    pub instruction: InstructionBinding,
    pub cause: AssistanceCause,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateAgentRunRequest {
    pub command_schema: String,
    pub create_agent_run_input: CreateAgentRunInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentRunRef {
    AgentRun { run_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CreateAgentRunEffect {
    Admitted {
        project_agent_id: String,
        conversation_id: String,
        memory_settings_revision: String,
        run_id: String,
        project_activity_position: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateAgentRunResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub acknowledgement: ExportAcknowledgement,
    pub operation_ref: Option<AgentRunRef>,
    pub project: ControlledProject,
    pub project_agent_id: String,
    pub conversation_id: String,
    pub memory_settings_revision: String,
    pub effect: CreateAgentRunEffect,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunStatus {
    Queued,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetAgentRunResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub project_agent_id: String,
    pub conversation_id: String,
    pub memory_settings_revision: String,
    pub run_id: String,
    pub status: AgentRunStatus,
}
