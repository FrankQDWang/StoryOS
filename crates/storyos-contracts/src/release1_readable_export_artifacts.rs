use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_readable_export::{
    EXPORT_HUMAN_READABLE_MANUSCRIPT, EXPORT_HUMAN_READABLE_MANUSCRIPT_DIGEST_PROFILE,
    EXPORT_HUMAN_READABLE_MANUSCRIPT_REQUEST_SCHEMA_ID,
    EXPORT_HUMAN_READABLE_MANUSCRIPT_RESPONSE_SCHEMA_ID, ExportAcknowledgement,
    ExportHumanReadableManuscriptEffect, ExportHumanReadableManuscriptInput,
    ExportHumanReadableManuscriptRefusalReason, ExportHumanReadableManuscriptRequest,
    ExportHumanReadableManuscriptResponse, HumanReadableManuscriptExportRef,
};

pub(super) const REQUEST_SCHEMA_PATH: &str = "generated/json-schema/storyos-public-release-1/export-human-readable-manuscript-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str = "generated/json-schema/storyos-public-release-1/export-human-readable-manuscript-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/export-human-readable-manuscript.json",
    "generated/golden-wire/storyos-public-release-1/export-human-readable-manuscript.invalid.json",
    "generated/golden-wire/storyos-public-release-1/export-human-readable-manuscript.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<ExportHumanReadableManuscriptRequest>(
        EXPORT_HUMAN_READABLE_MANUSCRIPT_REQUEST_SCHEMA_ID,
        "StoryOS Export Human-Readable Manuscript Request",
    );
    schema["properties"]["command_schema"]["const"] =
        json!(EXPORT_HUMAN_READABLE_MANUSCRIPT_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["ExportHumanReadableManuscriptInput"]["properties"];
    input["correlation_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<ExportHumanReadableManuscriptResponse>(
        EXPORT_HUMAN_READABLE_MANUSCRIPT_RESPONSE_SCHEMA_ID,
        "StoryOS Export Human-Readable Manuscript Response",
    );
    schema["properties"]["schema_id"]["const"] =
        json!(EXPORT_HUMAN_READABLE_MANUSCRIPT_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = EXPORT_HUMAN_READABLE_MANUSCRIPT
        .responses
        .iter()
        .map(|(status, description)| {
            let retry_after = if *status == 429 {
                "          headers:\n            Retry-After:\n              required: true\n              schema:\n                type: integer\n                minimum: 1\n                maximum: 60\n"
            } else {
                ""
            };
            let content = if *status == 202 {
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
            "  {}:\n    post:\n      operationId: {}\n      summary: Export one deterministic human-readable manuscript\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        EXPORT_HUMAN_READABLE_MANUSCRIPT.path,
        EXPORT_HUMAN_READABLE_MANUSCRIPT.operation_id,
        request_schema,
        responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        ExportHumanReadableManuscriptInput::decl(&config),
        ExportHumanReadableManuscriptRequest::decl(&config),
        ExportHumanReadableManuscriptRefusalReason::decl(&config),
        ExportAcknowledgement::decl(&config),
        HumanReadableManuscriptExportRef::decl(&config),
        ExportHumanReadableManuscriptEffect::decl(&config),
        ExportHumanReadableManuscriptResponse::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    format!(
        concat!(
            "\nexport async function digestExportHumanReadableManuscript(request, cryptoImpl = globalThis.crypto) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"digestExportHumanReadableManuscript requires request\");\n",
            "  const canonical = canonicalJson(request);\n",
            "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
            "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
            "  return {{ algorithm: \"sha256\", profile: \"{}\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
            "\nexport async function exportHumanReadableManuscript({{ projectId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"exportHumanReadableManuscript requires projectId\");\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"exportHumanReadableManuscript requires request\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"exportHumanReadableManuscript requires security bindings\");\n",
            "  return commandJson({{ ...options, method: \"POST\", path: `{}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
        ),
        EXPORT_HUMAN_READABLE_MANUSCRIPT_DIGEST_PROFILE,
        EXPORT_HUMAN_READABLE_MANUSCRIPT
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}"),
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    concat!(
        "export declare function digestExportHumanReadableManuscript(request: ExportHumanReadableManuscriptRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function exportHumanReadableManuscript(options: StoryOSQueryOptions & { projectId: string; request: ExportHumanReadableManuscriptRequest; idempotencyKey: string; antiForgery: string }): Promise<ExportHumanReadableManuscriptResponse>;\n",
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture("2026-08-31T12:00:00.000Z"))
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = command_fixture("2026-08-31T12:00:00.000Z");
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
        "schema_id": EXPORT_HUMAN_READABLE_MANUSCRIPT_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000910",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "command_id": "018f0000-0000-7001-8000-000000000911",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000912",
        "acknowledgement": "accepted",
        "operation_ref": {
            "kind": "human_readable_manuscript_export",
            "export_id": "018f0000-0000-7001-8000-000000000916"
        },
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000913",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000201"
            },
            "command_kind": "exportHumanReadableManuscript",
            "command_digest": {
                "algorithm": "sha256",
                "profile": EXPORT_HUMAN_READABLE_MANUSCRIPT_DIGEST_PROFILE,
                "value_hex_lowercase": "c".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000914",
            "producer_cause": "author_command_admission",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000912",
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
            "open": {"kind": "empty"}
        },
        "effect": {
            "kind": "authoritative_applied",
            "export_id": "018f0000-0000-7001-8000-000000000916",
            "content_sha256": "a".repeat(64),
            "export_profile": "storyos.readable-export.utf8-lf.v1",
            "project_activity_position": "2"
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
