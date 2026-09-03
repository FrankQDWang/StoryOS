use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1::PUBLIC_PROTOCOL_RELEASE;
use crate::release1_readable_export_query::{
    GET_HUMAN_READABLE_MANUSCRIPT_EXPORT, GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_REQUEST_SCHEMA_ID,
    GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_RESPONSE_SCHEMA_ID,
    GetHumanReadableManuscriptExportResponse,
};

pub(super) const REQUEST_SCHEMA_PATH: &str = "generated/json-schema/storyos-public-release-1/human-readable-manuscript-export-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str = "generated/json-schema/storyos-public-release-1/human-readable-manuscript-export-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/get-human-readable-manuscript-export.json",
    "generated/golden-wire/storyos-public-release-1/get-human-readable-manuscript-export.invalid.json",
    "generated/golden-wire/storyos-public-release-1/get-human-readable-manuscript-export.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    json_bytes(&json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_REQUEST_SCHEMA_ID,
        "title": "StoryOS Human-Readable Manuscript Export Request",
        "type": "object",
        "additionalProperties": false,
        "required": ["project_id", "export_id"],
        "properties": {
            "project_id": {"type": "string", "format": "uuid"},
            "export_id": {"type": "string", "format": "uuid"}
        }
    }))
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = serde_json::to_value(schema_for!(GetHumanReadableManuscriptExportResponse))
        .expect("human-readable export response schema serializes");
    schema["$id"] = json!(GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_RESPONSE_SCHEMA_ID);
    schema["title"] = json!("StoryOS Human-Readable Manuscript Export Response");
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let response_schema = RESPONSE_SCHEMA_PATH
        .strip_prefix("generated/")
        .expect("schema is a generated artifact");
    let responses = GET_HUMAN_READABLE_MANUSCRIPT_EXPORT
        .responses
        .iter()
        .map(|(status, description)| {
            let content = if *status == 200 {
                format!(
                    "          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n"
                )
            } else {
                String::new()
            };
            format!("        '{status}':\n          description: {description}\n{content}")
        })
        .collect::<String>();
    format!(
        concat!(
            "  {}:\n    get:\n      operationId: {}\n      summary: Inspect one human-readable manuscript export\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: export_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "      responses:\n{}",
        ),
        GET_HUMAN_READABLE_MANUSCRIPT_EXPORT.path,
        GET_HUMAN_READABLE_MANUSCRIPT_EXPORT.operation_id,
        responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}",
        GetHumanReadableManuscriptExportResponse::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    let path = GET_HUMAN_READABLE_MANUSCRIPT_EXPORT
        .path
        .replace("{project_id}", "${encodeURIComponent(projectId)}")
        .replace("{export_id}", "${encodeURIComponent(exportId)}");
    format!(
        concat!(
            "\nexport async function getHumanReadableManuscriptExport({{ projectId, exportId, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"getHumanReadableManuscriptExport requires projectId\");\n",
            "  if (typeof exportId !== \"string\" || exportId.length === 0) throw new TypeError(\"getHumanReadableManuscriptExport requires exportId\");\n",
            "  return queryJson({{ ...options, path: `{}` }});\n}}\n"
        ),
        path
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    "export declare function getHumanReadableManuscriptExport(options: StoryOSQueryOptions & { projectId: string; exportId: string }): Promise<GetHumanReadableManuscriptExportResponse>;\n"
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&export_fixture())
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = export_fixture();
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("source_snapshot");
    json_bytes(&value)
}

pub(super) fn boundary_fixture_bytes() -> Vec<u8> {
    let mut boundary = export_fixture();
    boundary["manuscript_utf8"] = json!("\n");
    json_bytes(&boundary)
}

fn export_fixture() -> Value {
    json!({
        "status": "ready",
        "schema_id": GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_RESPONSE_SCHEMA_ID,
        "query_id": "018f0000-0000-7001-8000-000000000071",
        "correlation_id": "018f0000-0000-7001-8000-000000000072",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "export_id": "018f0000-0000-7001-8000-000000000916",
        "export_profile": "storyos.readable-export.utf8-lf.v1",
        "content_sha256": "a".repeat(64),
        "manuscript_utf8": "# Volume A\n\n## Chapter A\n\nHello world\n",
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
            "created_at": "2026-08-31T12:00:00.000Z",
            "expires_at": null
        }
    })
}

fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("contract JSON should serialize");
    bytes.push(b'\n');
    bytes
}
