use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_delete_chapter::{
    DELETE_CHAPTER, DELETE_CHAPTER_DIGEST_PROFILE, DELETE_CHAPTER_REQUEST_SCHEMA_ID,
    DELETE_CHAPTER_RESPONSE_SCHEMA_ID, DeleteChapterConflictReason, DeleteChapterEffect,
    DeleteChapterInput, DeleteChapterNoEffectReason, DeleteChapterRefusalReason,
    DeleteChapterRequest, DeleteChapterResponse,
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/delete-chapter-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/delete-chapter-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/delete-chapter.json",
    "generated/golden-wire/storyos-public-release-1/delete-chapter.invalid.json",
    "generated/golden-wire/storyos-public-release-1/delete-chapter.boundary.json",
];

const U64_WIRE: &str = "^(?:0|[1-9][0-9]{0,18}|1[0-7][0-9]{18}|18[0-3][0-9]{17}|184[0-3][0-9]{16}|1844[0-5][0-9]{15}|18446[0-6][0-9]{14}|184467[0-3][0-9]{13}|1844674[0-3][0-9]{12}|184467440[0-6][0-9]{10}|1844674407[0-2][0-9]{9}|18446744073[0-6][0-9]{8}|1844674407370[0-8][0-9]{6}|18446744073709[0-4][0-9]{5}|184467440737095[0-4][0-9]{3}|1844674407370955[0-9]{2}|18446744073709551[0-5]|1844674407370955160|1844674407370955161[0-5])$";

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<DeleteChapterRequest>(
        DELETE_CHAPTER_REQUEST_SCHEMA_ID,
        "StoryOS Delete Chapter Request",
    );
    schema["properties"]["command_schema"]["const"] = json!(DELETE_CHAPTER_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["DeleteChapterInput"]["properties"];
    input["correlation_id"]["format"] = json!("uuid");
    input["expected_chapter_revision"] = json!({"type": "string", "pattern": U64_WIRE});
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<DeleteChapterResponse>(
        DELETE_CHAPTER_RESPONSE_SCHEMA_ID,
        "StoryOS Delete Chapter Response",
    );
    schema["properties"]["schema_id"]["const"] = json!(DELETE_CHAPTER_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn method_openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = DELETE_CHAPTER
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
            "    delete:\n      operationId: {}\n      summary: Remove one named Chapter\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: chapter_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        DELETE_CHAPTER.operation_id, request_schema, responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        DeleteChapterInput::decl(&config),
        DeleteChapterRequest::decl(&config),
        DeleteChapterNoEffectReason::decl(&config),
        DeleteChapterConflictReason::decl(&config),
        DeleteChapterRefusalReason::decl(&config),
        DeleteChapterEffect::decl(&config),
        DeleteChapterResponse::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    format!(
        concat!(
            "\nexport async function digestDeleteChapter(request, cryptoImpl = globalThis.crypto) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"digestDeleteChapter requires request\");\n",
            "  const canonical = canonicalJson(request);\n",
            "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
            "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
            "  return {{ algorithm: \"sha256\", profile: \"{}\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
            "\nexport async function deleteChapter({{ projectId, chapterId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"deleteChapter requires projectId\");\n",
            "  if (typeof chapterId !== \"string\" || chapterId.length === 0) throw new TypeError(\"deleteChapter requires chapterId\");\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"deleteChapter requires request\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"deleteChapter requires security bindings\");\n",
            "  return commandJson({{ ...options, method: \"DELETE\", path: `{}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
        ),
        DELETE_CHAPTER_DIGEST_PROFILE,
        DELETE_CHAPTER
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}")
            .replace("{chapter_id}", "${encodeURIComponent(chapterId)}"),
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    concat!(
        "export declare function digestDeleteChapter(request: DeleteChapterRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function deleteChapter(options: StoryOSQueryOptions & { projectId: string; chapterId: string; request: DeleteChapterRequest; idempotencyKey: string; antiForgery: string }): Promise<DeleteChapterResponse>;\n",
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture("2026-08-27T12:00:00.000Z"))
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = command_fixture("2026-08-27T12:00:00.000Z");
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
        "schema_id": DELETE_CHAPTER_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000b10",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "command_id": "018f0000-0000-7001-8000-000000000b11",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000b12",
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000b13",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000201"
            },
            "command_kind": "deleteChapter",
            "command_digest": {
                "algorithm": "sha256",
                "profile": DELETE_CHAPTER_DIGEST_PROFILE,
                "value_hex_lowercase": "e".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000b14",
            "producer_cause": "author_command_admission",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000b12",
            "expected_heads": [],
            "prior_heads": [],
            "resulting_heads": [],
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
            "project_id": "018f0000-0000-7001-8000-000000000201",
            "title": "Empty Novel",
            "open": {
                "kind": "current_chapter",
                "current_chapter_id": "018f0000-0000-7001-8000-000000000916"
            }
        },
        "effect": {
            "kind": "authoritative_applied",
            "chapter_id": "018f0000-0000-7001-8000-000000000917",
            "volume_id": "018f0000-0000-7001-8000-000000000815",
            "tree_revision": "5",
            "project_activity_position": "5"
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
