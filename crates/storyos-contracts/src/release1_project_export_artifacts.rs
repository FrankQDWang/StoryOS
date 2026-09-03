use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1::PUBLIC_PROTOCOL_RELEASE;
use crate::release1_project_export::{
    EXPORT_PROJECT_ARCHIVE, EXPORT_PROJECT_ARCHIVE_DIGEST_PROFILE,
    EXPORT_PROJECT_ARCHIVE_REQUEST_SCHEMA_ID, EXPORT_PROJECT_ARCHIVE_RESPONSE_SCHEMA_ID,
    ExportProjectArchiveEffect, ExportProjectArchiveInput, ExportProjectArchiveRefusalReason,
    ExportProjectArchiveRequest, ExportProjectArchiveResponse, ProjectExportRef,
};
pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/export-project-archive-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/export-project-archive-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/export-project-archive.json",
    "generated/golden-wire/storyos-public-release-1/export-project-archive.invalid.json",
    "generated/golden-wire/storyos-public-release-1/export-project-archive.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<ExportProjectArchiveRequest>(
        EXPORT_PROJECT_ARCHIVE_REQUEST_SCHEMA_ID,
        "StoryOS Export Project Archive Request",
    );
    schema["properties"]["command_schema"]["const"] =
        json!(EXPORT_PROJECT_ARCHIVE_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["ExportProjectArchiveInput"]["properties"];
    input["correlation_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<ExportProjectArchiveResponse>(
        EXPORT_PROJECT_ARCHIVE_RESPONSE_SCHEMA_ID,
        "StoryOS Export Project Archive Response",
    );
    schema["properties"]["schema_id"]["const"] = json!(EXPORT_PROJECT_ARCHIVE_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = EXPORT_PROJECT_ARCHIVE
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
            "  {}:\n    post:\n      operationId: {}\n      summary: Admit one durable Project Export Archive operation\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        EXPORT_PROJECT_ARCHIVE.path, EXPORT_PROJECT_ARCHIVE.operation_id, request_schema, responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        ExportProjectArchiveInput::decl(&config),
        ExportProjectArchiveRequest::decl(&config),
        ExportProjectArchiveRefusalReason::decl(&config),
        ProjectExportRef::decl(&config),
        ExportProjectArchiveEffect::decl(&config),
        ExportProjectArchiveResponse::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    format!(
        concat!(
            "\nexport async function digestExportProjectArchive(request, cryptoImpl = globalThis.crypto) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"digestExportProjectArchive requires request\");\n",
            "  const canonical = canonicalJson(request);\n",
            "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
            "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
            "  return {{ algorithm: \"sha256\", profile: \"{}\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
            "\nexport async function exportProjectArchive({{ projectId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"exportProjectArchive requires projectId\");\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"exportProjectArchive requires request\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"exportProjectArchive requires security bindings\");\n",
            "  return commandJson({{ ...options, method: \"POST\", path: `{}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
        ),
        EXPORT_PROJECT_ARCHIVE_DIGEST_PROFILE,
        EXPORT_PROJECT_ARCHIVE
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}"),
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    concat!(
        "export declare function digestExportProjectArchive(request: ExportProjectArchiveRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function exportProjectArchive(options: StoryOSQueryOptions & { projectId: string; request: ExportProjectArchiveRequest; idempotencyKey: string; antiForgery: string }): Promise<ExportProjectArchiveResponse>;\n",
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture("2026-09-01T12:00:00.000Z"))
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = command_fixture("2026-09-01T12:00:00.000Z");
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
        "schema_id": EXPORT_PROJECT_ARCHIVE_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000920",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "command_id": "018f0000-0000-7001-8000-000000000921",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000922",
        "acknowledgement": "accepted",
        "operation_ref": {
            "kind": "project_export",
            "export_id": "018f0000-0000-7001-8000-000000000926"
        },
        "project": {
            "project_id": "018f0000-0000-7001-8000-000000000201",
            "title": "Empty Novel",
            "open": {"kind": "empty"}
        },
        "effect": {
            "kind": "admitted",
            "export_id": "018f0000-0000-7001-8000-000000000926",
            "archive_profile": "storyos.project-export.v1",
            "archive_path_profile": "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1",
            "source_snapshot": {
                "snapshot_id": "018f0000-0000-7001-8000-000000000040",
                "project_scope": {
                    "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                    "project_id": "018f0000-0000-7001-8000-000000000201"
                },
                "snapshot_kind": "canonical",
                "project_activity_position": "2",
                "source_watermarks": {},
                "projection_generations": {},
                "redaction_profile": "storyos.author.v1",
                "schema_profile": PUBLIC_PROTOCOL_RELEASE,
                "replay_generation": "1",
                "created_at": created_at,
                "expires_at": null
            }
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
