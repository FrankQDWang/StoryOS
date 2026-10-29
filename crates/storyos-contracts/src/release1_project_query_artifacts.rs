use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1::{
    GET_PROJECT, GetProjectResponse, PROJECT_REQUEST_SCHEMA_ID, PROJECT_RESPONSE_SCHEMA_ID,
};
use crate::release1_operation_registry::{
    FixtureMembership, GeneratedSchema, OperationArtifacts, RegisteredOperation, method,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_wire::{json_bytes, path_request_schema, query_openapi, schema_value};

const PROJECT_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/project-request.schema.json";

pub(super) const PROJECT_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/project-response.schema.json";

const PROJECT_FIXTURE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/get-project.json";

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::query(
        &GET_PROJECT,
        &["server_derived_project_scope", "project_visibility"],
        OperationCatalog::new(
            "project:read",
            Owner::Release,
            Binding::CURRENT_PROJECT_QUERY,
        ),
    )],
    schemas: || {
        vec![
            GeneratedSchema {
                schema_id: PROJECT_REQUEST_SCHEMA_ID,
                path: PROJECT_REQUEST_SCHEMA_PATH,
                bytes: json_bytes(&path_request_schema(
                    PROJECT_REQUEST_SCHEMA_ID,
                    &["project_id"],
                )),
            },
            GeneratedSchema {
                schema_id: PROJECT_RESPONSE_SCHEMA_ID,
                path: PROJECT_RESPONSE_SCHEMA_PATH,
                bytes: json_bytes(&schema_value::<GetProjectResponse>(
                    PROJECT_RESPONSE_SCHEMA_ID,
                    "StoryOS Project Query Response",
                )),
            },
        ]
    },
    openapi: || {
        method(
            &GET_PROJECT,
            query_openapi(
                &GET_PROJECT,
                "Read one controlled StoryOS Project",
                PROJECT_RESPONSE_SCHEMA_PATH,
                &["project_id"],
            ),
        )
    },
    typescript_types: || format!("export {}", GetProjectResponse::decl(&Config::default())),
    fixtures: &[FixtureMembership {
        path: PROJECT_FIXTURE_PATH,
        bytes: |_| project_fixture_bytes(),
    }],
};

fn project_fixture() -> Value {
    json!({
        "schema_id": PROJECT_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000005",
        "project_scope": {"owner_user_id": "018f0000-0000-7001-8000-000000000001", "project_id": "018f0000-0000-7001-8000-000000000002"},
        "project": {"project_id": "018f0000-0000-7001-8000-000000000002", "title": "受控项目", "open": {"kind": "current_chapter", "current_chapter_id": "018f0000-0000-7001-8000-000000000003"}}
    })
}

fn project_fixture_bytes() -> Vec<u8> {
    json_bytes(&project_fixture())
}
