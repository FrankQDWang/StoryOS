use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_author_edit_artifacts as author_edit_artifacts;
use crate::release1_takeover::{
    TAKE_OVER_PROJECT_WRITER, TAKE_OVER_PROJECT_WRITER_REQUEST_SCHEMA_ID,
    TAKE_OVER_PROJECT_WRITER_RESPONSE_SCHEMA_ID, TakeOverProjectWriterRequest,
    TakeOverProjectWriterResponse, TakeOverProjectWriterResult, TakeoverCompareFailedReason,
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/take-over-project-writer-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/take-over-project-writer-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/take-over-project-writer.json",
    "generated/golden-wire/storyos-public-release-1/take-over-project-writer.invalid.json",
    "generated/golden-wire/storyos-public-release-1/take-over-project-writer.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    schema_bytes::<TakeOverProjectWriterRequest>(
        TAKE_OVER_PROJECT_WRITER_REQUEST_SCHEMA_ID,
        "StoryOS Take Over Project Writer Request",
    )
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    schema_bytes::<TakeOverProjectWriterResponse>(
        TAKE_OVER_PROJECT_WRITER_RESPONSE_SCHEMA_ID,
        "StoryOS Take Over Project Writer Response",
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}",
        TakeOverProjectWriterRequest::decl(&config),
        TakeoverCompareFailedReason::decl(&config),
        TakeOverProjectWriterResult::decl(&config),
        TakeOverProjectWriterResponse::decl(&config),
    )
}

pub(super) fn openapi() -> String {
    let request_schema = REQUEST_SCHEMA_PATH
        .strip_prefix("generated/")
        .expect("request schema is generated");
    let response_schema = RESPONSE_SCHEMA_PATH
        .strip_prefix("generated/")
        .expect("response schema is generated");
    let responses = TAKE_OVER_PROJECT_WRITER
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
        "  {}:\n    post:\n      operationId: {}\n      summary: Take over the current Project writer generation\n      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n        - name: editor_session_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{request_schema}'\n      responses:\n{responses}",
        TAKE_OVER_PROJECT_WRITER.path, TAKE_OVER_PROJECT_WRITER.operation_id
    )
}

pub(super) fn typescript_client_source() -> String {
    format!(
        concat!(
            "\nexport async function digestTakeOverProjectWriter(request, cryptoImpl = globalThis.crypto) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"digestTakeOverProjectWriter requires request\");\n",
            "  const canonical = canonicalJson(request);\n",
            "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
            "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
            "  return {{ algorithm: \"sha256\", profile: \"storyos.command.takeOverProjectWriter.jcs.v1\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
            "\nexport async function takeOverProjectWriter({{ projectId, editorSessionId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"takeOverProjectWriter requires projectId\");\n",
            "  if (typeof editorSessionId !== \"string\" || editorSessionId.length === 0) throw new TypeError(\"takeOverProjectWriter requires editorSessionId\");\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"takeOverProjectWriter requires request\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"takeOverProjectWriter requires security bindings\");\n",
            "  return commandJson({{ ...options, path: `{}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
        ),
        TAKE_OVER_PROJECT_WRITER
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}")
            .replace(
                "{editor_session_id}",
                "${encodeURIComponent(editorSessionId)}"
            ),
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    concat!(
        "export declare function digestTakeOverProjectWriter(request: TakeOverProjectWriterRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function takeOverProjectWriter(options: StoryOSQueryOptions & { projectId: string; editorSessionId: string; request: TakeOverProjectWriterRequest; idempotencyKey: string; antiForgery: string }): Promise<TakeOverProjectWriterResponse>;\n",
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&fixture())
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = fixture();
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("project_scope");
    json_bytes(&value)
}

pub(super) fn boundary_fixture_bytes() -> Vec<u8> {
    let mut value = fixture();
    value["result"] = json!({
        "kind": "takeover_compare_failed",
        "observed_writer_generation": "1",
        "current_writer_generation": "2",
        "current_writer_projection": {
            "kind": "current_writer",
            "writer_generation": "2"
        },
        "current_snapshot_id": "018f0000-0000-7001-8000-000000000032",
        "current_snapshot_activity_position": "1",
        "current_heads": ["018f0000-0000-7001-8000-000000000004"],
        "reason": "writer_generation_advanced_after_admission"
    });
    json_bytes(&value)
}

fn fixture() -> Value {
    json!({
        "schema_id": TAKE_OVER_PROJECT_WRITER_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000040",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000002"
        },
        "command_id": "018f0000-0000-7001-8000-000000000041",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000042",
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000043",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000002"
            },
            "command_kind": "takeOverProjectWriter",
            "command_digest": {
                "algorithm": "sha256",
                "profile": "storyos.command.takeOverProjectWriter.jcs.v1",
                "value_hex_lowercase": "c".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000044",
            "producer_cause": "author_command_admission",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000042",
            "expected_heads": ["018f0000-0000-7001-8000-000000000004"],
            "prior_heads": ["018f0000-0000-7001-8000-000000000004"],
            "resulting_heads": ["018f0000-0000-7001-8000-000000000004"],
            "authoritative_revision_ids": [],
            "proposal_revision_ids": [],
            "authoritative_commit_ids": [],
            "author_action_sequence": null,
            "draft_artifact_refs": [],
            "artifact_lifecycle_event_refs": [],
            "condition_refs": [],
            "result": "no_effect",
            "created_at": "2026-08-19T06:00:00.000Z"
        },
        "result": {
            "kind": "takeover_applied",
            "prior_editor_session_id": "018f0000-0000-7001-8000-000000000021",
            "prior_writer_generation": "1",
            "resulting_editor_session_id": "018f0000-0000-7001-8000-000000000040",
            "resulting_writer_generation": "2",
            "resulting_snapshot_id": "018f0000-0000-7001-8000-000000000032",
            "resulting_snapshot_activity_position": "1",
            "resulting_heads": ["018f0000-0000-7001-8000-000000000004"]
        }
    })
}

fn apply_u64_wire_constraints(schema: &mut Value, canonical_u64: &Value) {
    if schema["properties"]
        .get("observed_writer_generation")
        .is_some()
    {
        schema["properties"]["observed_writer_generation"] = canonical_u64.clone();
    }
    if schema["$defs"].get("DomainReceipt").is_some() {
        schema["$defs"]["DomainReceipt"]["properties"]["author_action_sequence"] =
            json!({"anyOf": [canonical_u64, {"type": "null"}]});
    }
    if schema["$defs"].get("EditorWriterProjection").is_some() {
        let variants = schema["$defs"]["EditorWriterProjection"]["oneOf"]
            .as_array_mut()
            .expect("writer projection variants must exist");
        variants[0]["properties"]["writer_generation"] = canonical_u64.clone();
        variants[1]["properties"]["observed_writer_generation"] = canonical_u64.clone();
    }
    if schema["$defs"].get("TakeOverProjectWriterResult").is_some() {
        let variants = schema["$defs"]["TakeOverProjectWriterResult"]["oneOf"]
            .as_array_mut()
            .expect("takeover result variants must exist");
        for variant in variants.iter_mut() {
            variant["additionalProperties"] = Value::Bool(false);
        }
        let applied = &mut variants[0];
        applied["properties"]["prior_writer_generation"] = canonical_u64.clone();
        applied["properties"]["resulting_writer_generation"] = canonical_u64.clone();
        applied["properties"]["resulting_snapshot_activity_position"] = canonical_u64.clone();
        let failed = &mut variants[1];
        failed["properties"]["observed_writer_generation"] = canonical_u64.clone();
        failed["properties"]["current_writer_generation"] = canonical_u64.clone();
        failed["properties"]["current_snapshot_activity_position"] = canonical_u64.clone();
    }
}

fn schema_bytes<T: schemars::JsonSchema>(schema_id: &str, title: &str) -> Vec<u8> {
    let mut schema = serde_json::to_value(schema_for!(T)).expect("contract schema serializes");
    schema["$id"] = Value::String(schema_id.to_owned());
    schema["title"] = Value::String(title.to_owned());
    let canonical_u64 = author_edit_artifacts::canonical_u64_wire_schema();
    apply_u64_wire_constraints(&mut schema, &canonical_u64);
    json_bytes(&schema)
}

fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("contract JSON should serialize");
    bytes.push(b'\n');
    bytes
}
