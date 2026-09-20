use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, QueryOperation};
use crate::release1_agent_run::AgentRunStatus;
use crate::release1_author_edit::DomainReceipt;

pub const PAUSE_AGENT_RUN_REQUEST_SCHEMA_ID: &str = "storyos.command.pause-agent-run.request.v1";
pub const PAUSE_AGENT_RUN_RESPONSE_SCHEMA_ID: &str = "storyos.command.pause-agent-run.response.v1";
pub const PAUSE_AGENT_RUN_DIGEST_PROFILE: &str = "storyos.command.pauseAgentRun.jcs.v1";
pub const CANCEL_AGENT_RUN_REQUEST_SCHEMA_ID: &str = "storyos.command.cancel-agent-run.request.v1";
pub const CANCEL_AGENT_RUN_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.cancel-agent-run.response.v1";
pub const CANCEL_AGENT_RUN_DIGEST_PROFILE: &str = "storyos.command.cancelAgentRun.jcs.v1";

const CONTROL_STATUSES: &[(u16, &str)] = &[
    (200, "AgentRun control settled"),
    (400, "Invalid request"),
    (401, "Authentication required"),
    (403, "Request origin refused"),
    (404, "Resource unavailable"),
    (405, "Method not allowed"),
    (409, "Idempotency or Run-state conflict"),
    (412, "Session binding refused"),
    (413, "Request too large"),
    (415, "Unsupported content type"),
    (422, "AgentRun control refused"),
    (428, "Precondition required"),
    (429, "Rate limited"),
    (503, "Service unavailable"),
];

pub(super) const PAUSE_AGENT_RUN: QueryOperation = QueryOperation {
    operation_id: "pauseAgentRun",
    method: "POST",
    path: "/api/v1/projects/{project_id}/agent-runs/{run_id}/pause",
    request_schema: PAUSE_AGENT_RUN_REQUEST_SCHEMA_ID,
    response_schema: PAUSE_AGENT_RUN_RESPONSE_SCHEMA_ID,
    responses: CONTROL_STATUSES,
    fixtures: &[
        "storyos.golden.pauseAgentRun.positive.v1",
        "storyos.golden.pauseAgentRun.invalid.v1",
        "storyos.golden.pauseAgentRun.boundary.v1",
    ],
};

pub(super) const CANCEL_AGENT_RUN: QueryOperation = QueryOperation {
    operation_id: "cancelAgentRun",
    method: "POST",
    path: "/api/v1/projects/{project_id}/agent-runs/{run_id}/cancel",
    request_schema: CANCEL_AGENT_RUN_REQUEST_SCHEMA_ID,
    response_schema: CANCEL_AGENT_RUN_RESPONSE_SCHEMA_ID,
    responses: CONTROL_STATUSES,
    fixtures: &[
        "storyos.golden.cancelAgentRun.positive.v1",
        "storyos.golden.cancelAgentRun.invalid.v1",
        "storyos.golden.cancelAgentRun.boundary.v1",
    ],
};

pub const PAUSE_AGENT_RUN_PATH: &str = PAUSE_AGENT_RUN.path;
pub const PAUSE_AGENT_RUN_METHOD: &str = PAUSE_AGENT_RUN.method;
pub const CANCEL_AGENT_RUN_PATH: &str = CANCEL_AGENT_RUN.path;
pub const CANCEL_AGENT_RUN_METHOD: &str = CANCEL_AGENT_RUN.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PauseAgentRunInput {
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PauseAgentRunRequest {
    pub command_schema: String,
    pub pause_agent_run_input: PauseAgentRunInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PauseAgentRunNoEffectReason {
    AlreadyPaused,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PauseAgentRunConflictReason {
    TerminalRun,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PauseAgentRunEffect {
    Applied {
        run_id: String,
        status: AgentRunStatus,
        fence_generation: String,
        project_activity_position: String,
    },
    NoEffect {
        reason: PauseAgentRunNoEffectReason,
    },
    Conflicted {
        reason: PauseAgentRunConflictReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PauseAgentRunResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub project: ControlledProject,
    pub effect: PauseAgentRunEffect,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CancelAgentRunInput {
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CancelAgentRunRequest {
    pub command_schema: String,
    pub cancel_agent_run_input: CancelAgentRunInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CancelAgentRunNoEffectReason {
    AlreadyCancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CancelAgentRunConflictReason {
    TerminalRun,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CancelAgentRunEffect {
    Applied {
        run_id: String,
        status: AgentRunStatus,
        fence_generation: String,
        project_activity_position: String,
    },
    NoEffect {
        reason: CancelAgentRunNoEffectReason,
    },
    Conflicted {
        reason: CancelAgentRunConflictReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CancelAgentRunResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub project: ControlledProject,
    pub effect: CancelAgentRunEffect,
}
