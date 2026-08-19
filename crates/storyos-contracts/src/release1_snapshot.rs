use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ProjectScope, QueryOperation};

pub const GET_SNAPSHOT_REQUEST_SCHEMA_ID: &str = "storyos.query.snapshot.request.v1";
pub const GET_SNAPSHOT_RESPONSE_SCHEMA_ID: &str = "storyos.query.snapshot.response.v1";
pub const ACTIVITY_STREAM_REQUEST_SCHEMA_ID: &str = "storyos.stream.activity-stream.request.v1";
pub const ACTIVITY_STREAM_RESPONSE_SCHEMA_ID: &str = "storyos.stream.activity-stream.response.v1";

pub(super) const GET_SNAPSHOT: QueryOperation = QueryOperation {
    operation_id: "getSnapshot",
    method: "GET",
    path: "/api/v1/projects/{project_id}/snapshots/{snapshot_id}",
    request_schema: GET_SNAPSHOT_REQUEST_SCHEMA_ID,
    response_schema: GET_SNAPSHOT_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Authorized canonical Snapshot"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Protocol or cursor conflict"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Request refused"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.getSnapshot.positive.v1",
        "storyos.golden.getSnapshot.invalid.v1",
        "storyos.golden.getSnapshot.boundary.v1",
    ],
};

pub(super) const ACTIVITY_STREAM: QueryOperation = QueryOperation {
    operation_id: "activityStream",
    method: "GET",
    path: "/api/v1/projects/{project_id}/activity",
    request_schema: ACTIVITY_STREAM_REQUEST_SCHEMA_ID,
    response_schema: ACTIVITY_STREAM_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Project Activity SSE stream"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Protocol or cursor conflict"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.activityStream.positive.v1",
        "storyos.golden.activityStream.invalid.v1",
        "storyos.golden.activityStream.boundary.v1",
    ],
};

pub const GET_SNAPSHOT_PATH: &str = GET_SNAPSHOT.path;
pub const GET_SNAPSHOT_METHOD: &str = GET_SNAPSHOT.method;
pub const ACTIVITY_STREAM_PATH: &str = ACTIVITY_STREAM.path;
pub const ACTIVITY_STREAM_METHOD: &str = ACTIVITY_STREAM.method;

/// Closed `snapshot_kind` selected by the protocol SnapshotDescriptor.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotKind {
    Canonical,
}

/// Closed empty maps for Stage 1 `canonical` SnapshotDescriptor watermarks.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CanonicalSnapshotMaps {}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetSnapshotRequest {
    pub project_id: String,
    pub snapshot_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SnapshotDescriptor {
    pub snapshot_id: String,
    pub project_scope: ProjectScope,
    pub snapshot_kind: SnapshotKind,
    pub project_activity_position: String,
    pub source_watermarks: CanonicalSnapshotMaps,
    pub projection_generations: CanonicalSnapshotMaps,
    pub redaction_profile: String,
    pub schema_profile: String,
    pub replay_generation: String,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetSnapshotResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: ProjectScope,
    pub snapshot: SnapshotDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ActivityStreamRequest {
    pub project_id: String,
    pub snapshot_id: String,
    pub protocol_release: String,
}
