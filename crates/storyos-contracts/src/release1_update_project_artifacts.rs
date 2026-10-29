use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::ProjectActivityKind;
use crate::release1_operation_registry::{
    FixtureMembership, OperationArtifacts, RegisteredOperation, method, operation_schemas,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_update_project::{
    UPDATE_PROJECT, UPDATE_PROJECT_DIGEST_PROFILE, UPDATE_PROJECT_REQUEST_SCHEMA_ID,
    UPDATE_PROJECT_RESPONSE_SCHEMA_ID, UpdateProjectConflictReason, UpdateProjectEffect,
    UpdateProjectInput, UpdateProjectNoEffectReason, UpdateProjectRequest, UpdateProjectResponse,
};
use crate::release1_wire::{U64_WIRE, generated_ref, json_bytes, schema_value};

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::command(
        &UPDATE_PROJECT,
        &[
            "server_derived_project_scope",
            "expected_project_revision",
            "project_active",
        ],
        OperationCatalog::new("project:update", Owner::Release, Binding::PROJECT_COMMAND)
            .activity(&[ProjectActivityKind::ProjectUpdated], &[]),
    )],
    schemas: || {
        operation_schemas(
            &UPDATE_PROJECT,
            (REQUEST_SCHEMA_PATH, request_schema_bytes()),
            (RESPONSE_SCHEMA_PATH, response_schema_bytes()),
        )
        .into()
    },
    openapi: || method(&UPDATE_PROJECT, method_openapi()),
    typescript_types: typescript_type_declarations,
    fixtures: &[FixtureMembership {
        path: FIXTURE_PATH,
        bytes: |_| fixture_bytes(),
    }],
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/update-project-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/update-project-response.schema.json";
pub(super) const FIXTURE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/update-project.json";

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<UpdateProjectRequest>(
        UPDATE_PROJECT_REQUEST_SCHEMA_ID,
        "StoryOS Update Project Request",
    );
    schema["properties"]["command_schema"]["const"] = json!(UPDATE_PROJECT_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["UpdateProjectInput"]["properties"];
    input["correlation_id"]["format"] = json!("uuid");
    input["expected_project_revision"] = json!({"type": "string", "pattern": U64_WIRE});
    let title = &mut input["title"];
    title["minLength"] = json!(1);
    title["x-storyos-max-utf8-bytes"] = json!(1024);
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<UpdateProjectResponse>(
        UPDATE_PROJECT_RESPONSE_SCHEMA_ID,
        "StoryOS Update Project Response",
    );
    schema["properties"]["schema_id"]["const"] = json!(UPDATE_PROJECT_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn method_openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = UPDATE_PROJECT
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
            "    patch:\n      operationId: {}\n      summary: Rename one exact Project\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        UPDATE_PROJECT.operation_id, request_schema, responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        UpdateProjectInput::decl(&config),
        UpdateProjectRequest::decl(&config),
        UpdateProjectNoEffectReason::decl(&config),
        UpdateProjectConflictReason::decl(&config),
        UpdateProjectEffect::decl(&config),
        UpdateProjectResponse::decl(&config),
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture())
}

fn command_fixture() -> Value {
    json!({
        "schema_id": UPDATE_PROJECT_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000710",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "command_id": "018f0000-0000-7001-8000-000000000711",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000712",
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000713",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000201"
            },
            "command_kind": "updateProject",
            "command_digest": {
                "algorithm": "sha256",
                "profile": UPDATE_PROJECT_DIGEST_PROFILE,
                "value_hex_lowercase": "c".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000714",
            "producer_cause": "author_command_admission",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000712",
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
            "created_at": "2026-08-25T08:05:00.000Z"
        },
        "project": {
            "project_id": "018f0000-0000-7001-8000-000000000201",
            "title": "Renamed Novel",
            "open": {"kind": "empty"}
        },
        "effect": {
            "kind": "authoritative_applied",
            "title": "Renamed Novel",
            "revision": "2",
            "project_activity_position": "1"
        }
    })
}
