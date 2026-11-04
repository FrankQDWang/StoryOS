use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, HttpMethod, QueryOperation};
use crate::release1_author_edit::DomainReceipt;

pub const SET_PROJECT_TOOL_ENABLEMENT_REQUEST_SCHEMA_ID: &str =
    "storyos.command.set-project-tool-enablement.request.v1";
pub const SET_PROJECT_TOOL_ENABLEMENT_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.set-project-tool-enablement.response.v1";
pub const SET_PROJECT_TOOL_ENABLEMENT_DIGEST_PROFILE: &str =
    "storyos.command.setProjectToolEnablement.jcs.v1";

pub(super) const SET_PROJECT_TOOL_ENABLEMENT: QueryOperation = QueryOperation {
    operation_id: "setProjectToolEnablement",
    method: HttpMethod::Put,
    path: "/api/v1/projects/{project_id}/tool-enablement",
    request_schema: SET_PROJECT_TOOL_ENABLEMENT_REQUEST_SCHEMA_ID,
    response_schema: SET_PROJECT_TOOL_ENABLEMENT_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Project Tool Enablement settled"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency conflict"),
        (412, "Session binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Set Project Tool Enablement refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixture: "storyos.golden.setProjectToolEnablement.positive.v1",
};

pub const SET_PROJECT_TOOL_ENABLEMENT_PATH: &str = SET_PROJECT_TOOL_ENABLEMENT.path;
pub const SET_PROJECT_TOOL_ENABLEMENT_METHOD: &str = SET_PROJECT_TOOL_ENABLEMENT.method.as_str();

/// The research tool state of the Project. It names no server and no key.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProjectResearchTool {
    NotOffered,
    Disabled,
    Enabled,
    Quarantined,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SetProjectToolEnablementInput {
    pub registration_id: String,
    pub enabled: bool,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SetProjectToolEnablementRequest {
    pub command_schema: String,
    pub set_project_tool_enablement_input: SetProjectToolEnablementInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SetProjectToolEnablementNoEffectReason {
    EnablementUnchanged,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SetProjectToolEnablementRefusalReason {
    RegistrationNotOffered,
    RegistrationUnavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SetProjectToolEnablementEffect {
    AuthoritativeApplied {
        enabled: bool,
        revision: String,
        project_activity_position: String,
    },
    NoEffect {
        reason: SetProjectToolEnablementNoEffectReason,
    },
    Refused {
        reason: SetProjectToolEnablementRefusalReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SetProjectToolEnablementResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub project: ControlledProject,
    pub effect: SetProjectToolEnablementEffect,
}
