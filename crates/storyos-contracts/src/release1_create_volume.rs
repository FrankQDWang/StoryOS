use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, QueryOperation};
use crate::release1_author_edit::DomainReceipt;

pub const CREATE_VOLUME_REQUEST_SCHEMA_ID: &str = "storyos.command.create-volume.request.v1";
pub const CREATE_VOLUME_RESPONSE_SCHEMA_ID: &str = "storyos.command.create-volume.response.v1";
pub const CREATE_VOLUME_DIGEST_PROFILE: &str = "storyos.command.createVolume.jcs.v1";

pub(super) const CREATE_VOLUME: QueryOperation = QueryOperation {
    operation_id: "createVolume",
    method: "POST",
    path: "/api/v1/projects/{project_id}/volumes",
    request_schema: CREATE_VOLUME_REQUEST_SCHEMA_ID,
    response_schema: CREATE_VOLUME_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Volume created"),
        (201, "Volume created"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or tree revision conflict"),
        (412, "Session binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Create Volume refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.createVolume.positive.v1",
        "storyos.golden.createVolume.invalid.v1",
        "storyos.golden.createVolume.boundary.v1",
    ],
};

pub const CREATE_VOLUME_PATH: &str = CREATE_VOLUME.path;
pub const CREATE_VOLUME_METHOD: &str = CREATE_VOLUME.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateVolumeInput {
    pub title: String,
    pub expected_tree_revision: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateVolumeRequest {
    pub command_schema: String,
    pub create_volume_input: CreateVolumeInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CreateVolumeConflictReason {
    StaleTreeRevision,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CreateVolumeRefusalReason {
    ArchivedProject,
    InvalidTitle,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CreateVolumeEffect {
    AuthoritativeApplied {
        volume_id: String,
        title: String,
        tree_revision: String,
        order: String,
        project_activity_position: String,
    },
    Conflicted {
        reason: CreateVolumeConflictReason,
    },
    Refused {
        reason: CreateVolumeRefusalReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateVolumeResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub project: ControlledProject,
    pub effect: CreateVolumeEffect,
}
