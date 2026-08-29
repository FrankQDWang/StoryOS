use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_set_current_chapter::{
    SET_CURRENT_CHAPTER, SET_CURRENT_CHAPTER_DIGEST_PROFILE, SET_CURRENT_CHAPTER_REQUEST_SCHEMA_ID,
    SET_CURRENT_CHAPTER_RESPONSE_SCHEMA_ID, SetCurrentChapterConflictReason,
    SetCurrentChapterEffect, SetCurrentChapterInput, SetCurrentChapterNoEffectReason,
    SetCurrentChapterRefusalReason, SetCurrentChapterRequest, SetCurrentChapterResponse,
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/set-current-chapter-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/set-current-chapter-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/set-current-chapter.json",
    "generated/golden-wire/storyos-public-release-1/set-current-chapter.invalid.json",
    "generated/golden-wire/storyos-public-release-1/set-current-chapter.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<SetCurrentChapterRequest>(
        SET_CURRENT_CHAPTER_REQUEST_SCHEMA_ID,
        "StoryOS Set Current Chapter Request",
    );
    schema["properties"]["command_schema"]["const"] = json!(SET_CURRENT_CHAPTER_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["SetCurrentChapterInput"]["properties"];
    input["correlation_id"]["format"] = json!("uuid");
    input["chapter_id"]["format"] = json!("uuid");
    input["expected_current_chapter_id"]["format"] = json!("uuid");
    input["expected_target_revision_id"]["format"] = json!("uuid");
    input["editor_session_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<SetCurrentChapterResponse>(
        SET_CURRENT_CHAPTER_RESPONSE_SCHEMA_ID,
        "StoryOS Set Current Chapter Response",
    );
    schema["properties"]["schema_id"]["const"] = json!(SET_CURRENT_CHAPTER_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = SET_CURRENT_CHAPTER
        .responses
        .iter()
        .map(|(status, description)| {
            let retry_after = if *status == 429 {
                "          headers:\n            Retry-After:\n              required: true\n              schema:\n                type: integer\n                minimum: 1\n                maximum: 60\n"
            } else {
                ""
            };
            let content = if *status == 200 {
                format!(
                    "          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n"
                )
            } else {
                String::new()
            };
            format!("        '{status}':\n          description: {description}\n{retry_after}{content}")
        })
        .collect::<String>();
    format!(
        concat!(
            "  {}:\n    put:\n      operationId: {}\n      summary: Make one eligible Chapter current\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        SET_CURRENT_CHAPTER.path, SET_CURRENT_CHAPTER.operation_id, request_schema, responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        SetCurrentChapterInput::decl(&config),
        SetCurrentChapterRequest::decl(&config),
        SetCurrentChapterNoEffectReason::decl(&config),
        SetCurrentChapterConflictReason::decl(&config),
        SetCurrentChapterRefusalReason::decl(&config),
        SetCurrentChapterEffect::decl(&config),
        SetCurrentChapterResponse::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    format!(
        concat!(
            "\nexport async function digestSetCurrentChapter(request, cryptoImpl = globalThis.crypto) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"digestSetCurrentChapter requires request\");\n",
            "  const canonical = canonicalJson(request);\n",
            "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
            "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
            "  return {{ algorithm: \"sha256\", profile: \"{}\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
            "\nexport async function setCurrentChapter({{ projectId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"setCurrentChapter requires projectId\");\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"setCurrentChapter requires request\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"setCurrentChapter requires security bindings\");\n",
            "  return commandJson({{ ...options, method: \"PUT\", path: `{}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
        ),
        SET_CURRENT_CHAPTER_DIGEST_PROFILE,
        SET_CURRENT_CHAPTER
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}"),
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    concat!(
        "export declare function digestSetCurrentChapter(request: SetCurrentChapterRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function setCurrentChapter(options: StoryOSQueryOptions & { projectId: string; request: SetCurrentChapterRequest; idempotencyKey: string; antiForgery: string }): Promise<SetCurrentChapterResponse>;\n",
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture("2026-08-29T12:00:00.000Z"))
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = command_fixture("2026-08-29T12:00:00.000Z");
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("project");
    json_bytes(&value)
}

pub(super) fn boundary_fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture("1970-01-01T00:00:00.000Z"))
}

fn command_fixture(created_at: &str) -> Value {
    json!({
        "schema_id": SET_CURRENT_CHAPTER_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000920",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000002"
        },
        "command_id": "018f0000-0000-7001-8000-000000000921",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000922",
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000923",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000002"
            },
            "command_kind": "setCurrentChapter",
            "command_digest": {
                "algorithm": "sha256",
                "profile": SET_CURRENT_CHAPTER_DIGEST_PROFILE,
                "value_hex_lowercase": "d".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000924",
            "producer_cause": "author_command_admission",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000922",
            "expected_heads": ["018f0000-0000-7001-8000-000000000805"],
            "prior_heads": ["018f0000-0000-7001-8000-000000000805"],
            "resulting_heads": ["018f0000-0000-7001-8000-000000000805"],
            "authoritative_revision_ids": [],
            "proposal_revision_ids": [],
            "authoritative_commit_ids": [],
            "author_action_sequence": null,
            "draft_artifact_refs": [],
            "artifact_lifecycle_event_refs": [],
            "condition_refs": [],
            "result": "authoritative_applied",
            "created_at": created_at
        },
        "project": {
            "project_id": "018f0000-0000-7001-8000-000000000002",
            "title": "Project A",
            "open": {
                "kind": "current_chapter",
                "current_chapter_id": "018f0000-0000-7001-8000-000000000803"
            }
        },
        "effect": {
            "kind": "authoritative_applied",
            "current_chapter_id": "018f0000-0000-7001-8000-000000000803",
            "base_snapshot_id": "018f0000-0000-7001-8000-000000000925",
            "project_activity_position": "4"
        }
    })
}

fn generated_ref(path: &str) -> &str {
    path.strip_prefix("generated/")
        .expect("schema is a generated artifact")
}

fn schema_value<T: schemars::JsonSchema>(schema_id: &str, title: &str) -> Value {
    let mut schema = serde_json::to_value(schema_for!(T)).expect("contract schema serializes");
    schema["$id"] = Value::String(schema_id.to_owned());
    schema["title"] = Value::String(title.to_owned());
    schema
}

fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("contract JSON should serialize");
    bytes.push(b'\n');
    bytes
}
