use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, QueryOperation};
use crate::release1_author_edit::DomainReceipt;

pub const DELETE_VOLUME_REQUEST_SCHEMA_ID: &str = "storyos.command.delete-volume.request.v1";
pub const DELETE_VOLUME_RESPONSE_SCHEMA_ID: &str = "storyos.command.delete-volume.response.v1";
pub const DELETE_VOLUME_DIGEST_PROFILE: &str = "storyos.command.deleteVolume.jcs.v1";

pub(super) const DELETE_VOLUME: QueryOperation = QueryOperation {
    operation_id: "deleteVolume",
    method: "DELETE",
    path: "/api/v1/projects/{project_id}/volumes/{volume_id}",
    request_schema: DELETE_VOLUME_REQUEST_SCHEMA_ID,
    response_schema: DELETE_VOLUME_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Volume removed"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or tree revision conflict"),
        (412, "Session binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Delete Volume refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.deleteVolume.positive.v1",
        "storyos.golden.deleteVolume.invalid.v1",
        "storyos.golden.deleteVolume.boundary.v1",
    ],
};

pub const DELETE_VOLUME_PATH: &str = DELETE_VOLUME.path;
pub const DELETE_VOLUME_METHOD: &str = DELETE_VOLUME.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DeleteVolumeInput {
    pub expected_tree_revision: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DeleteVolumeRequest {
    pub command_schema: String,
    pub delete_volume_input: DeleteVolumeInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DeleteVolumeNoEffectReason {
    AlreadyRemoved,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DeleteVolumeConflictReason {
    StaleTreeRevision,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DeleteVolumeRefusalReason {
    ArchivedProject,
    InvalidVolumeJoin,
    NonemptyVolume,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeleteVolumeEffect {
    AuthoritativeApplied {
        volume_id: String,
        tree_revision: String,
        project_activity_position: String,
    },
    NoEffect {
        reason: DeleteVolumeNoEffectReason,
    },
    Conflicted {
        reason: DeleteVolumeConflictReason,
    },
    Refused {
        reason: DeleteVolumeRefusalReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DeleteVolumeResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub project: ControlledProject,
    pub effect: DeleteVolumeEffect,
}
