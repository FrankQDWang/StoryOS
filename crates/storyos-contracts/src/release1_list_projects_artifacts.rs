use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_list_projects::{
    LIST_PROJECTS, LIST_PROJECTS_REQUEST_SCHEMA_ID, LIST_PROJECTS_RESPONSE_SCHEMA_ID,
    ListProjectsResponse, ProjectLifecycleState, ProjectListItem,
};
use crate::release1_operation_registry::{
    OperationArtifacts, RegisteredOperation, fixture_triple, method, operation_schemas,
};
use crate::release1_wire::{U64_WIRE, json_bytes};

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::query(
        &LIST_PROJECTS,
        &["server_derived_user"],
    )],
    schemas: || {
        operation_schemas(
            &LIST_PROJECTS,
            (REQUEST_SCHEMA_PATH, request_schema_bytes()),
            (RESPONSE_SCHEMA_PATH, response_schema_bytes()),
        )
        .into()
    },
    openapi: || method(&LIST_PROJECTS, method_openapi()),
    typescript_types: typescript_type_declarations,
    typescript_client: typescript_client_source,
    typescript_declarations,
    fixtures: || {
        fixture_triple(
            FIXTURE_PATHS,
            &LIST_PROJECTS,
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
    "generated/json-schema/storyos-public-release-1/project-list-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/project-list-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/list-projects.json",
    "generated/golden-wire/storyos-public-release-1/list-projects.invalid.json",
    "generated/golden-wire/storyos-public-release-1/list-projects.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    json_bytes(&json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": LIST_PROJECTS_REQUEST_SCHEMA_ID,
        "title": "StoryOS Project List Request",
        "type": "object",
        "additionalProperties": false,
        "maxProperties": 0
    }))
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = serde_json::to_value(schema_for!(ListProjectsResponse))
        .expect("list response schema serializes");
    schema["$id"] = json!(LIST_PROJECTS_RESPONSE_SCHEMA_ID);
    schema["title"] = json!("StoryOS Project List Response");
    schema["properties"]["schema_id"]["const"] = json!(LIST_PROJECTS_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["owner_user_id"]["format"] = json!("uuid");
    if let Some(item) = schema["$defs"].get_mut("ProjectListItem") {
        item["properties"]["revision"] = json!({
            "type": "string",
            "pattern": U64_WIRE
        });
        let title = &mut item["properties"]["title"];
        title["minLength"] = json!(1);
        title["x-storyos-max-utf8-bytes"] = json!(1024);
    }
    if let Some(scope) = schema["$defs"].get_mut("ProjectScope") {
        scope["properties"]["owner_user_id"]["format"] = json!("uuid");
        scope["properties"]["project_id"]["format"] = json!("uuid");
    }
    json_bytes(&schema)
}

pub(super) fn method_openapi() -> String {
    let response_schema = RESPONSE_SCHEMA_PATH
        .strip_prefix("generated/")
        .expect("schema is a generated artifact");
    let responses = LIST_PROJECTS
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
        "    get:\n      operationId: {}\n      summary: List Projects owned by the current User\n      responses:\n{responses}",
        LIST_PROJECTS.operation_id,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}",
        ProjectLifecycleState::decl(&config),
        ProjectListItem::decl(&config),
        ListProjectsResponse::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    format!(
        "\nexport async function listProjects(options = {{}}) {{\n  return queryJson({{ ...options, path: \"{}\" }});\n}}\n",
        LIST_PROJECTS.path,
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    "export declare function listProjects(options: StoryOSQueryOptions): Promise<ListProjectsResponse>;\n"
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&list_fixture(vec![
        json!({
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000002"
            },
            "title": "Project A",
            "lifecycle": {"kind": "active"},
            "revision": "1",
            "open": {
                "kind": "current_chapter",
                "current_chapter_id": "018f0000-0000-7001-8000-000000000003"
            }
        }),
        json!({
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000012"
            },
            "title": "Empty Novel",
            "lifecycle": {"kind": "active"},
            "revision": "1",
            "open": {"kind": "empty"}
        }),
    ]))
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = list_fixture(Vec::new());
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("owner_user_id");
    json_bytes(&value)
}

pub(super) fn boundary_fixture_bytes() -> Vec<u8> {
    json_bytes(&list_fixture(Vec::new()))
}

fn list_fixture(projects: Vec<Value>) -> Value {
    json!({
        "schema_id": LIST_PROJECTS_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000013",
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "projects": projects
    })
}
