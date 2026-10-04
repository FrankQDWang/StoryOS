use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_operation_registry::{
    OperationArtifacts, RegisteredOperation, fixture_triple, method, operation_schemas,
};
use crate::release1_update_chapter::{
    UPDATE_CHAPTER, UPDATE_CHAPTER_DIGEST_PROFILE, UPDATE_CHAPTER_REQUEST_SCHEMA_ID,
    UPDATE_CHAPTER_RESPONSE_SCHEMA_ID, UpdateChapterConflictReason, UpdateChapterEffect,
    UpdateChapterInput, UpdateChapterNoEffectReason, UpdateChapterRefusalReason,
    UpdateChapterRequest, UpdateChapterResponse,
};
use crate::release1_wire::{U64_WIRE, generated_ref, json_bytes, schema_value};

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::command(
        &UPDATE_CHAPTER,
        &[
            "server_derived_project_scope",
            "chapter_scope_join",
            "expected_tree_revision",
        ],
    )],
    schemas: || {
        operation_schemas(
            &UPDATE_CHAPTER,
            (REQUEST_SCHEMA_PATH, request_schema_bytes()),
            (RESPONSE_SCHEMA_PATH, response_schema_bytes()),
        )
        .into()
    },
    openapi: || method(&UPDATE_CHAPTER, method_openapi()),
    typescript_types: typescript_type_declarations,
    typescript_client: typescript_client_source,
    typescript_declarations,
    fixtures: || {
        fixture_triple(
            FIXTURE_PATHS,
            &UPDATE_CHAPTER,
            [
                |_| fixture_bytes(),
                |_| invalid_fixture_bytes(),
                |_| boundary_fixture_bytes(),
            ],
        )
        .into()
    },
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/update-chapter-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/update-chapter-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/update-chapter.json",
    "generated/golden-wire/storyos-public-release-1/update-chapter.invalid.json",
    "generated/golden-wire/storyos-public-release-1/update-chapter.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<UpdateChapterRequest>(
        UPDATE_CHAPTER_REQUEST_SCHEMA_ID,
        "StoryOS Update Chapter Request",
    );
    schema["properties"]["command_schema"]["const"] = json!(UPDATE_CHAPTER_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["UpdateChapterInput"]["properties"];
    input["correlation_id"]["format"] = json!("uuid");
    input["expected_tree_revision"] = json!({"type": "string", "pattern": U64_WIRE});
    input["order"] = json!({"type": "string", "pattern": U64_WIRE});
    let title = &mut input["title"];
    title["minLength"] = json!(1);
    title["x-storyos-max-utf8-bytes"] = json!(1024);
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<UpdateChapterResponse>(
        UPDATE_CHAPTER_RESPONSE_SCHEMA_ID,
        "StoryOS Update Chapter Response",
    );
    schema["properties"]["schema_id"]["const"] = json!(UPDATE_CHAPTER_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn method_openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = UPDATE_CHAPTER
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
            "    patch:\n      operationId: {}\n      summary: Rename or reorder one named Chapter\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: chapter_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        UPDATE_CHAPTER.operation_id, request_schema, responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        UpdateChapterInput::decl(&config),
        UpdateChapterRequest::decl(&config),
        UpdateChapterNoEffectReason::decl(&config),
        UpdateChapterConflictReason::decl(&config),
        UpdateChapterRefusalReason::decl(&config),
        UpdateChapterEffect::decl(&config),
        UpdateChapterResponse::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    format!(
        concat!(
            "\nexport async function digestUpdateChapter(request, cryptoImpl = globalThis.crypto) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"digestUpdateChapter requires request\");\n",
            "  const canonical = canonicalJson(request);\n",
            "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
            "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
            "  return {{ algorithm: \"sha256\", profile: \"{}\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
            "\nexport async function updateChapter({{ projectId, chapterId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"updateChapter requires projectId\");\n",
            "  if (typeof chapterId !== \"string\" || chapterId.length === 0) throw new TypeError(\"updateChapter requires chapterId\");\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"updateChapter requires request\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"updateChapter requires security bindings\");\n",
            "  return commandJson({{ ...options, method: \"PATCH\", path: `{}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
        ),
        UPDATE_CHAPTER_DIGEST_PROFILE,
        UPDATE_CHAPTER
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}")
            .replace("{chapter_id}", "${encodeURIComponent(chapterId)}"),
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    concat!(
        "export declare function digestUpdateChapter(request: UpdateChapterRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function updateChapter(options: StoryOSQueryOptions & { projectId: string; chapterId: string; request: UpdateChapterRequest; idempotencyKey: string; antiForgery: string }): Promise<UpdateChapterResponse>;\n",
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
        "schema_id": UPDATE_CHAPTER_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000a10",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "command_id": "018f0000-0000-7001-8000-000000000a11",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000a12",
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000a13",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000201"
            },
            "command_kind": "updateChapter",
            "command_digest": {
                "algorithm": "sha256",
                "profile": UPDATE_CHAPTER_DIGEST_PROFILE,
                "value_hex_lowercase": "d".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000a14",
            "producer_cause": "author_command_admission",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000a12",
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
                "kind": "empty"
            }
        },
        "effect": {
            "kind": "authoritative_applied",
            "chapter_id": "018f0000-0000-7001-8000-000000000815",
            "title": "Chapter B",
            "tree_revision": "5",
            "order": "2",
            "project_activity_position": "4"
        }
    })
}
