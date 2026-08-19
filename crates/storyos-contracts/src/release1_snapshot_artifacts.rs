use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1::PUBLIC_PROTOCOL_RELEASE;
use crate::release1_author_edit_artifacts as author_edit_artifacts;
use crate::release1_snapshot::{
    ACTIVITY_STREAM, ACTIVITY_STREAM_REQUEST_SCHEMA_ID, ACTIVITY_STREAM_RESPONSE_SCHEMA_ID,
    ActivityStreamRequest, CanonicalSnapshotMaps, GET_SNAPSHOT, GET_SNAPSHOT_REQUEST_SCHEMA_ID,
    GET_SNAPSHOT_RESPONSE_SCHEMA_ID, GetSnapshotRequest, GetSnapshotResponse, SnapshotDescriptor,
    SnapshotKind,
};

pub(super) const SNAPSHOT_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/snapshot-request.schema.json";
pub(super) const SNAPSHOT_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/snapshot-response.schema.json";
pub(super) const ACTIVITY_STREAM_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/activity-stream-request.schema.json";
pub(super) const ACTIVITY_STREAM_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/activity-stream-response.schema.json";
pub(super) const SNAPSHOT_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/get-snapshot.json",
    "generated/golden-wire/storyos-public-release-1/get-snapshot.invalid.json",
    "generated/golden-wire/storyos-public-release-1/get-snapshot.boundary.json",
];
pub(super) const ACTIVITY_STREAM_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/activity-stream.json",
    "generated/golden-wire/storyos-public-release-1/activity-stream.invalid.json",
    "generated/golden-wire/storyos-public-release-1/activity-stream.boundary.json",
];

pub(super) fn snapshot_request_schema_bytes() -> Vec<u8> {
    let mut schema = typed_schema::<GetSnapshotRequest>(
        GET_SNAPSHOT_REQUEST_SCHEMA_ID,
        "StoryOS Snapshot Request",
    );
    schema["properties"]["project_id"]["format"] = json!("uuid");
    schema["properties"]["snapshot_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn snapshot_response_schema_bytes() -> Vec<u8> {
    let mut schema = typed_schema::<GetSnapshotResponse>(
        GET_SNAPSHOT_RESPONSE_SCHEMA_ID,
        "StoryOS Snapshot Response",
    );
    let canonical_u64 = author_edit_artifacts::canonical_u64_wire_schema();
    schema["$defs"]["SnapshotDescriptor"]["properties"]["project_activity_position"] =
        canonical_u64.clone();
    schema["$defs"]["SnapshotDescriptor"]["properties"]["replay_generation"] = canonical_u64;
    schema["$defs"]["SnapshotDescriptor"]["properties"]["created_at"]["format"] =
        json!("date-time");
    schema["$defs"]["SnapshotDescriptor"]["properties"]["expires_at"]["format"] =
        json!("date-time");
    json_bytes(&schema)
}

pub(super) fn activity_stream_request_schema_bytes() -> Vec<u8> {
    let mut schema = typed_schema::<ActivityStreamRequest>(
        ACTIVITY_STREAM_REQUEST_SCHEMA_ID,
        "StoryOS Activity Stream Request",
    );
    schema["properties"]["project_id"]["format"] = json!("uuid");
    schema["properties"]["snapshot_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn activity_stream_response_schema_bytes() -> Vec<u8> {
    json_bytes(&json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": ACTIVITY_STREAM_RESPONSE_SCHEMA_ID,
        "title": "StoryOS Activity Stream SSE Frame",
        "type": "object",
        "additionalProperties": false,
        "required": ["event", "data_schema"],
        "properties": {
            "event": { "type": "string", "const": "storyos.project-activity" },
            "data_schema": { "type": "string", "const": "storyos.project-activity.v1" }
        }
    }))
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        SnapshotKind::decl(&config),
        CanonicalSnapshotMaps::decl(&config),
        SnapshotDescriptor::decl(&config),
        GetSnapshotRequest::decl(&config),
        GetSnapshotResponse::decl(&config),
        ActivityStreamRequest::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    let snapshot_path = GET_SNAPSHOT
        .path
        .replace("{project_id}", "${encodeURIComponent(projectId)}")
        .replace("{snapshot_id}", "${encodeURIComponent(snapshotId)}");
    let activity_path = ACTIVITY_STREAM
        .path
        .replace("{project_id}", "${encodeURIComponent(projectId)}");
    format!(
        concat!(
            "\nexport async function getSnapshot({{ projectId, snapshotId, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"getSnapshot requires projectId\");\n",
            "  if (typeof snapshotId !== \"string\" || snapshotId.length === 0) throw new TypeError(\"getSnapshot requires snapshotId\");\n",
            "  return queryJson({{ ...options, path: `{}` }});\n",
            "}}\n",
            "\nexport async function activityStream({{ projectId, snapshotId, protocolRelease, lastEventId, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"activityStream requires projectId\");\n",
            "  if (typeof snapshotId !== \"string\" || typeof protocolRelease !== \"string\") throw new TypeError(\"activityStream requires snapshot bindings\");\n",
            "  const headers = {{ accept: \"text/event-stream\" }};\n",
            "  if (typeof lastEventId === \"string\") headers[\"last-event-id\"] = lastEventId;\n",
            "  return queryText({{ ...options, path: `{}?snapshot_id=${{encodeURIComponent(snapshotId)}}&protocol_release=${{encodeURIComponent(protocolRelease)}}`, queryHeaders: headers }});\n",
            "}}\n",
        ),
        snapshot_path, activity_path,
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    "export declare function getSnapshot(options: StoryOSQueryOptions & { projectId: string; snapshotId: string }): Promise<GetSnapshotResponse>;\nexport declare function activityStream(options: StoryOSQueryOptions & { projectId: string; snapshotId: string; protocolRelease: string; lastEventId?: string }): Promise<string>;\n"
}

pub(super) fn snapshot_openapi() -> String {
    operation_openapi(
        &GET_SNAPSHOT,
        "Read one authorized canonical Snapshot",
        SNAPSHOT_RESPONSE_SCHEMA_PATH,
        &["project_id", "snapshot_id"],
    )
}

pub(super) fn activity_stream_openapi() -> String {
    let responses = ACTIVITY_STREAM
        .responses
        .iter()
        .map(|(status, description)| {
            format!("        '{status}':\n          description: {description}\n")
        })
        .collect::<String>();
    format!(
        "  {}:\n    get:\n      operationId: {}\n      summary: Stream Project Activity after an authorized Snapshot\n      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n        - name: snapshot_id\n          in: query\n          required: true\n          schema:\n            type: string\n            format: uuid\n        - name: protocol_release\n          in: query\n          required: true\n          schema:\n            type: string\n        - name: Last-Event-ID\n          in: header\n          required: false\n          schema:\n            type: string\n      responses:\n{responses}",
        ACTIVITY_STREAM.path, ACTIVITY_STREAM.operation_id,
    )
}

pub(super) fn snapshot_fixture_bytes() -> Vec<u8> {
    json_bytes(&snapshot_fixture())
}

pub(super) fn snapshot_invalid_fixture_bytes() -> Vec<u8> {
    let mut invalid = snapshot_fixture();
    invalid
        .as_object_mut()
        .expect("fixture is an object")
        .remove("snapshot");
    json_bytes(&invalid)
}

pub(super) fn snapshot_boundary_fixture_bytes() -> Vec<u8> {
    let mut boundary = snapshot_fixture();
    boundary["snapshot"]["expires_at"] = json!("2026-08-19T00:00:00.000Z");
    json_bytes(&boundary)
}

pub(super) fn activity_stream_fixture_bytes() -> Vec<u8> {
    json_bytes(&activity_fixture())
}

pub(super) fn activity_stream_invalid_fixture_bytes() -> Vec<u8> {
    json_bytes(&json!({
        "schema_id": ACTIVITY_STREAM_REQUEST_SCHEMA_ID,
        "project_id": "018f0000-0000-7001-8000-000000000002"
    }))
}

pub(super) fn activity_stream_boundary_fixture_bytes() -> Vec<u8> {
    let mut boundary = activity_fixture();
    boundary["protocol_release"] = json!("storyos.public.release.0");
    json_bytes(&boundary)
}

fn snapshot_fixture() -> Value {
    json!({
        "schema_id": GET_SNAPSHOT_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000091",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000002"
        },
        "snapshot": {
            "snapshot_id": "018f0000-0000-7001-8000-000000000032",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000002"
            },
            "snapshot_kind": "canonical",
            "project_activity_position": "0",
            "source_watermarks": {},
            "projection_generations": {},
            "redaction_profile": "storyos.author.v1",
            "schema_profile": PUBLIC_PROTOCOL_RELEASE,
            "replay_generation": "1",
            "created_at": "2026-07-21T10:16:30.000Z",
            "expires_at": null
        }
    })
}

fn activity_fixture() -> Value {
    json!({
        "schema_id": ACTIVITY_STREAM_REQUEST_SCHEMA_ID,
        "project_id": "018f0000-0000-7001-8000-000000000002",
        "snapshot_id": "018f0000-0000-7001-8000-000000000032",
        "protocol_release": PUBLIC_PROTOCOL_RELEASE
    })
}

fn typed_schema<T: schemars::JsonSchema>(schema_id: &str, title: &str) -> Value {
    let mut schema = serde_json::to_value(schema_for!(T)).expect("schema renders");
    schema["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
    schema["$id"] = json!(schema_id);
    schema["title"] = json!(title);
    schema
}

fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("schema JSON is serializable");
    bytes.push(b'\n');
    bytes
}

fn operation_openapi(
    operation: &crate::release1::QueryOperation,
    summary: &str,
    response_schema_path: &str,
    parameters: &[&str],
) -> String {
    let response_schema = response_schema_path
        .strip_prefix("generated/")
        .expect("response schema is generated");
    let parameter_yaml = parameters
        .iter()
        .map(|name| {
            format!(
                "        - name: {name}\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n"
            )
        })
        .collect::<String>();
    let responses = operation
        .responses
        .iter()
        .map(|(status, description)| {
            format!(
                "        '{status}':\n          description: {description}\n{}",
                if *status == 200 {
                    format!(
                        "          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n"
                    )
                } else {
                    String::new()
                }
            )
        })
        .collect::<String>();
    format!(
        "  {}:\n    get:\n      operationId: {}\n      summary: {summary}\n      parameters:\n{parameter_yaml}      responses:\n{responses}",
        operation.path, operation.operation_id,
    )
}
