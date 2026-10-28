use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_operation_registry::{
    OperationArtifacts, RegisteredOperation, fixture_triple, method, operation_schemas,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_undo_latest_author_action::{
    UNDO_LATEST_AUTHOR_ACTION, UNDO_LATEST_AUTHOR_ACTION_DIGEST_PROFILE,
    UNDO_LATEST_AUTHOR_ACTION_REQUEST_SCHEMA_ID, UNDO_LATEST_AUTHOR_ACTION_RESPONSE_SCHEMA_ID,
    UndoLatestAuthorActionConflictReason, UndoLatestAuthorActionEffect,
    UndoLatestAuthorActionInput, UndoLatestAuthorActionRequest, UndoLatestAuthorActionResponse,
    UndoLatestAuthorActionUnavailableReason,
};
use crate::release1_wire::{generated_ref, json_bytes, schema_value};

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::command(
        &UNDO_LATEST_AUTHOR_ACTION,
        &[
            "server_derived_project_scope",
            "project_active",
            "editor_session_writer_generation",
            "author_undo_frontier_exact",
            "expected_authoritative_head",
        ],
        OperationCatalog::new("author-action:undo", Owner::Core, Binding::EDITOR_COMMAND),
    )],
    schemas: || {
        let [request, response] = operation_schemas(
            &UNDO_LATEST_AUTHOR_ACTION,
            (REQUEST_SCHEMA_PATH, request_schema_bytes()),
            (RESPONSE_SCHEMA_PATH, response_schema_bytes()),
        );
        vec![request, response]
    },
    openapi: || method(&UNDO_LATEST_AUTHOR_ACTION, openapi()),
    typescript_types: typescript_type_declarations,
    fixtures: || {
        fixture_triple(
            FIXTURE_PATHS,
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
    "generated/json-schema/storyos-public-release-1/undo-latest-author-action-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/undo-latest-author-action-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/undo-latest-author-action.json",
    "generated/golden-wire/storyos-public-release-1/undo-latest-author-action.invalid.json",
    "generated/golden-wire/storyos-public-release-1/undo-latest-author-action.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<UndoLatestAuthorActionRequest>(
        UNDO_LATEST_AUTHOR_ACTION_REQUEST_SCHEMA_ID,
        "StoryOS Undo Latest Author Action Request",
    );
    schema["properties"]["command_schema"]["const"] =
        json!(UNDO_LATEST_AUTHOR_ACTION_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["UndoLatestAuthorActionInput"]["properties"];
    input["correlation_id"]["format"] = json!("uuid");
    input["expected_authoritative_revision_id"]["format"] = json!("uuid");
    input["editor_session_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<UndoLatestAuthorActionResponse>(
        UNDO_LATEST_AUTHOR_ACTION_RESPONSE_SCHEMA_ID,
        "StoryOS Undo Latest Author Action Response",
    );
    schema["properties"]["schema_id"]["const"] =
        json!(UNDO_LATEST_AUTHOR_ACTION_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = UNDO_LATEST_AUTHOR_ACTION
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
            "    post:\n      operationId: {}\n      summary: Undo one exact admitted Author Action\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        UNDO_LATEST_AUTHOR_ACTION.operation_id, request_schema, responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        UndoLatestAuthorActionInput::decl(&config),
        UndoLatestAuthorActionRequest::decl(&config),
        UndoLatestAuthorActionConflictReason::decl(&config),
        UndoLatestAuthorActionUnavailableReason::decl(&config),
        UndoLatestAuthorActionEffect::decl(&config),
        UndoLatestAuthorActionResponse::decl(&config),
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture("2026-08-30T12:00:00.000Z"))
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = command_fixture("2026-08-30T12:00:00.000Z");
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
        "schema_id": UNDO_LATEST_AUTHOR_ACTION_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000930",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000002"
        },
        "command_id": "018f0000-0000-7001-8000-000000000931",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000932",
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000933",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000002"
            },
            "command_kind": "undoLatestAuthorAction",
            "command_digest": {
                "algorithm": "sha256",
                "profile": UNDO_LATEST_AUTHOR_ACTION_DIGEST_PROFILE,
                "value_hex_lowercase": "d".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000934",
            "producer_cause": "author_command_admission",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000932",
            "expected_heads": ["018f0000-0000-7001-8000-000000000805"],
            "prior_heads": ["018f0000-0000-7001-8000-000000000805"],
            "resulting_heads": ["018f0000-0000-7001-8000-000000000806"],
            "authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000806"],
            "proposal_revision_ids": [],
            "authoritative_commit_ids": ["018f0000-0000-7001-8000-000000000807"],
            "author_action_sequence": "2",
            "draft_artifact_refs": [],
            "artifact_lifecycle_event_refs": [],
            "condition_refs": [],
            "result": "authoritative_applied",
            "created_at": created_at
        },
        "project": {
            "project_id": "018f0000-0000-7001-8000-000000000002",
            "title": "Project A",
            "open": {
                "kind": "current_chapter",
                "current_chapter_id": "018f0000-0000-7001-8000-000000000803"
            }
        },
        "effect": {
            "kind": "compensated",
            "source_sequence": "1",
            "author_action_sequence": "2",
            "authoritative_commit_id": "018f0000-0000-7001-8000-000000000807",
            "authoritative_revision": {
                "revision_id": "018f0000-0000-7001-8000-000000000806",
                "body": "Hello",
                "blocks": [{
                    "manuscript_block_id": "018f0000-0000-7001-8000-000000000808",
                    "block_kind": "paragraph",
                    "text": "Hello"
                }]
            },
            "project_activity_position": "3"
        }
    })
}
