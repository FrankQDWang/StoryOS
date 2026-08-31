use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1::{LIMIT_PROFILE_REVISION, PUBLIC_PROTOCOL_RELEASE};
use crate::release1_author_edit_artifacts as author_edit_artifacts;
use crate::release1_manuscript_search::{
    ManuscriptSearchCompleteness, ManuscriptSearchMatch, ManuscriptSearchSelection,
    SEARCH_MANUSCRIPT, SEARCH_MANUSCRIPT_REQUEST_SCHEMA_ID, SEARCH_MANUSCRIPT_RESPONSE_SCHEMA_ID,
    SearchManuscriptRequest, SearchManuscriptResponse,
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/manuscript-search-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/manuscript-search-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/search-manuscript.json",
    "generated/golden-wire/storyos-public-release-1/search-manuscript.invalid.json",
    "generated/golden-wire/storyos-public-release-1/search-manuscript.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = serde_json::to_value(schema_for!(SearchManuscriptRequest))
        .expect("manuscript search request schema serializes");
    schema["$id"] = json!(SEARCH_MANUSCRIPT_REQUEST_SCHEMA_ID);
    schema["title"] = json!("StoryOS Manuscript Search Request");
    schema["additionalProperties"] = json!(false);
    schema["properties"]["schema_id"]["const"] = json!(SEARCH_MANUSCRIPT_REQUEST_SCHEMA_ID);
    let query_text = &mut schema["properties"]["query_text"];
    query_text["minLength"] = json!(1);
    query_text["x-storyos-max-utf8-bytes"] = json!(1_048_576);
    schema["properties"]["required_watermark"] = json!({
        "anyOf": [
            author_edit_artifacts::canonical_u64_wire_schema(),
            { "type": "null" }
        ]
    });
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = serde_json::to_value(schema_for!(SearchManuscriptResponse))
        .expect("manuscript search response schema serializes");
    schema["$id"] = json!(SEARCH_MANUSCRIPT_RESPONSE_SCHEMA_ID);
    schema["title"] = json!("StoryOS Manuscript Search Response");
    schema["additionalProperties"] = json!(false);
    schema["properties"]["schema_id"]["const"] = json!(SEARCH_MANUSCRIPT_RESPONSE_SCHEMA_ID);
    schema["properties"]["query_id"]["format"] = json!("uuid");
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["projection_kind"]["const"] = json!("manuscript_search");
    let canonical_u64 = author_edit_artifacts::canonical_u64_wire_schema();
    schema["properties"]["projection_generation"] = canonical_u64.clone();
    schema["properties"]["projection_watermark"] = canonical_u64.clone();
    schema["properties"]["required_watermark"] = json!({
        "anyOf": [canonical_u64.clone(), { "type": "null" }]
    });
    schema["properties"]["lag"] = canonical_u64.clone();
    schema["properties"]["page_count"] = canonical_u64.clone();
    schema["properties"]["page_bytes"] = canonical_u64.clone();
    if let Some(scope) = schema["$defs"].get_mut("ProjectScope") {
        scope["properties"]["owner_user_id"]["format"] = json!("uuid");
        scope["properties"]["project_id"]["format"] = json!("uuid");
    }
    if let Some(snapshot) = schema["$defs"].get_mut("SnapshotDescriptor") {
        snapshot["properties"]["snapshot_id"]["format"] = json!("uuid");
        snapshot["properties"]["project_activity_position"] = canonical_u64.clone();
        snapshot["properties"]["replay_generation"] = canonical_u64;
        snapshot["properties"]["created_at"]["format"] = json!("date-time");
        snapshot["properties"]["expires_at"]["format"] = json!("date-time");
    }
    if let Some(item) = schema["$defs"].get_mut("ManuscriptSearchMatch") {
        item["properties"]["chapter_id"]["format"] = json!("uuid");
        item["properties"]["manuscript_block_id"]["format"] = json!("uuid");
        item["properties"]["start"] = author_edit_artifacts::canonical_u64_wire_schema();
        item["properties"]["end"] = author_edit_artifacts::canonical_u64_wire_schema();
    }
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let request_schema = REQUEST_SCHEMA_PATH
        .strip_prefix("generated/")
        .expect("schema is a generated artifact");
    let response_schema = RESPONSE_SCHEMA_PATH
        .strip_prefix("generated/")
        .expect("schema is a generated artifact");
    let responses = SEARCH_MANUSCRIPT
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
            "  {}:\n    post:\n      operationId: {}\n      summary: Search the current Chapter or manuscript\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        SEARCH_MANUSCRIPT.path, SEARCH_MANUSCRIPT.operation_id, request_schema, responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        ManuscriptSearchSelection::decl(&config),
        SearchManuscriptRequest::decl(&config),
        ManuscriptSearchMatch::decl(&config),
        ManuscriptSearchCompleteness::decl(&config),
        SearchManuscriptResponse::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    let path = SEARCH_MANUSCRIPT
        .path
        .replace("{project_id}", "${encodeURIComponent(projectId)}");
    format!(
        concat!(
            "\nexport async function searchManuscript({{ projectId, request, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"searchManuscript requires projectId\");\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"searchManuscript requires request\");\n",
            "  return queryPostJson({{ ...options, path: `{path}`, body: request }});\n}}\n"
        ),
        path = path
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    "export declare function searchManuscript(options: StoryOSQueryOptions & { projectId: string; request: SearchManuscriptRequest }): Promise<SearchManuscriptResponse>;\n"
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&search_fixture())
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = search_fixture();
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("source_snapshot");
    json_bytes(&value)
}

pub(super) fn boundary_fixture_bytes() -> Vec<u8> {
    let mut boundary = search_fixture();
    boundary["completeness"] = json!("truncated");
    boundary["page_count"] = json!("500");
    json_bytes(&boundary)
}

fn search_fixture() -> Value {
    json!({
        "schema_id": SEARCH_MANUSCRIPT_RESPONSE_SCHEMA_ID,
        "query_id": "018f0000-0000-7001-8000-000000000061",
        "correlation_id": "018f0000-0000-7001-8000-000000000062",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000002"
        },
        "source_snapshot": {
            "snapshot_id": "018f0000-0000-7001-8000-000000000032",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000002"
            },
            "snapshot_kind": "canonical",
            "project_activity_position": "4",
            "source_watermarks": {},
            "projection_generations": {},
            "redaction_profile": "storyos.author.v1",
            "schema_profile": PUBLIC_PROTOCOL_RELEASE,
            "replay_generation": "1",
            "created_at": "2026-08-31T00:00:00.000Z",
            "expires_at": null
        },
        "projection_kind": "manuscript_search",
        "projection_generation": "4",
        "projection_watermark": "4",
        "required_watermark": null,
        "completeness": "complete",
        "lag": "0",
        "items": [],
        "next_cursor": null,
        "page_count": "0",
        "page_bytes": "2",
        "redaction_profile": "storyos.author.v1",
        "limit_profile_revision": LIMIT_PROFILE_REVISION
    })
}

fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("contract JSON should serialize");
    bytes.push(b'\n');
    bytes
}
