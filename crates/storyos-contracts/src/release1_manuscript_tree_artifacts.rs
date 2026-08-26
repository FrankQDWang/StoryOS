use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1::PUBLIC_PROTOCOL_RELEASE;
use crate::release1_author_edit_artifacts as author_edit_artifacts;
use crate::release1_manuscript_tree::{
    GET_MANUSCRIPT_TREE, GET_MANUSCRIPT_TREE_REQUEST_SCHEMA_ID,
    GET_MANUSCRIPT_TREE_RESPONSE_SCHEMA_ID, GetManuscriptTreeResponse, ManuscriptChapterNode,
    ManuscriptVolumeNode,
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/manuscript-tree-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/manuscript-tree-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/get-manuscript-tree.json",
    "generated/golden-wire/storyos-public-release-1/get-manuscript-tree.invalid.json",
    "generated/golden-wire/storyos-public-release-1/get-manuscript-tree.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    json_bytes(&json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": GET_MANUSCRIPT_TREE_REQUEST_SCHEMA_ID,
        "type": "object",
        "additionalProperties": false,
        "required": ["project_id"],
        "properties": {
            "project_id": {"type": "string", "format": "uuid"}
        }
    }))
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = serde_json::to_value(schema_for!(GetManuscriptTreeResponse))
        .expect("manuscript tree response schema serializes");
    schema["$id"] = json!(GET_MANUSCRIPT_TREE_RESPONSE_SCHEMA_ID);
    schema["title"] = json!("StoryOS Manuscript Tree Response");
    schema["properties"]["schema_id"]["const"] = json!(GET_MANUSCRIPT_TREE_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    let canonical_u64 = author_edit_artifacts::canonical_u64_wire_schema();
    schema["properties"]["tree_revision"] = canonical_u64.clone();
    if let Some(scope) = schema["$defs"].get_mut("ProjectScope") {
        scope["properties"]["owner_user_id"]["format"] = json!("uuid");
        scope["properties"]["project_id"]["format"] = json!("uuid");
    }
    if let Some(snapshot) = schema["$defs"].get_mut("SnapshotDescriptor") {
        snapshot["properties"]["snapshot_id"]["format"] = json!("uuid");
        snapshot["properties"]["project_activity_position"] = canonical_u64.clone();
        snapshot["properties"]["replay_generation"] = canonical_u64.clone();
        snapshot["properties"]["created_at"]["format"] = json!("date-time");
        snapshot["properties"]["expires_at"]["format"] = json!("date-time");
    }
    if let Some(volume) = schema["$defs"].get_mut("ManuscriptVolumeNode") {
        volume["properties"]["volume_id"]["format"] = json!("uuid");
        volume["properties"]["order"] = canonical_u64.clone();
        let title = &mut volume["properties"]["title"];
        title["minLength"] = json!(1);
        title["x-storyos-max-utf8-bytes"] = json!(1024);
    }
    if let Some(chapter) = schema["$defs"].get_mut("ManuscriptChapterNode") {
        chapter["properties"]["chapter_id"]["format"] = json!("uuid");
        chapter["properties"]["order"] = canonical_u64;
        let title = &mut chapter["properties"]["title"];
        title["minLength"] = json!(1);
        title["x-storyos-max-utf8-bytes"] = json!(1024);
    }
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let response_schema = RESPONSE_SCHEMA_PATH
        .strip_prefix("generated/")
        .expect("schema is a generated artifact");
    let responses = GET_MANUSCRIPT_TREE
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
        "  {}:\n    get:\n      operationId: {}\n      summary: Read the ordered canonical manuscript tree\n      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n      responses:\n{responses}",
        GET_MANUSCRIPT_TREE.path, GET_MANUSCRIPT_TREE.operation_id,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}",
        ManuscriptChapterNode::decl(&config),
        ManuscriptVolumeNode::decl(&config),
        GetManuscriptTreeResponse::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    let path = GET_MANUSCRIPT_TREE
        .path
        .replace("{project_id}", "${encodeURIComponent(projectId)}");
    format!(
        "\nexport async function getManuscriptTree({{ projectId, ...options }} = {{}}) {{\n  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"getManuscriptTree requires projectId\");\n  return queryJson({{ ...options, path: `{path}` }});\n}}\n"
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    "export declare function getManuscriptTree(options: StoryOSQueryOptions & { projectId: string }): Promise<GetManuscriptTreeResponse>;\n"
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&tree_fixture(Vec::new()))
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = tree_fixture(Vec::new());
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("snapshot");
    json_bytes(&value)
}

pub(super) fn boundary_fixture_bytes() -> Vec<u8> {
    let mut boundary = tree_fixture(Vec::new());
    boundary["snapshot"]["expires_at"] = json!("2026-08-19T00:00:00.000Z");
    json_bytes(&boundary)
}

fn tree_fixture(volumes: Vec<Value>) -> Value {
    json!({
        "schema_id": GET_MANUSCRIPT_TREE_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000014",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000002"
        },
        "tree_revision": "1",
        "snapshot": {
            "snapshot_id": "018f0000-0000-7001-8000-000000000032",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000002"
            },
            "snapshot_kind": "canonical",
            "project_activity_position": "1",
            "source_watermarks": {},
            "projection_generations": {},
            "redaction_profile": "storyos.author.v1",
            "schema_profile": PUBLIC_PROTOCOL_RELEASE,
            "replay_generation": "1",
            "created_at": "2026-07-21T10:16:30.000Z",
            "expires_at": null
        },
        "volumes": volumes
    })
}

fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("contract JSON should serialize");
    bytes.push(b'\n');
    bytes
}
