use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::ProjectActivityKind;
use crate::release1_operation_registry::{
    FixtureMembership, OperationArtifacts, RegisteredOperation, method, operation_schemas,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
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
        OperationCatalog::new("chapter:update", Owner::Release, Binding::PROJECT_COMMAND)
            .activity(&[ProjectActivityKind::ChapterUpdated], &[])
            .action_coverage(&["rename", "reorder"]),
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
    fixtures: &[FixtureMembership {
        path: FIXTURE_PATH,
        bytes: |_| fixture_bytes(),
    }],
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/update-chapter-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/update-chapter-response.schema.json";
pub(super) const FIXTURE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/update-chapter.json";

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

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture())
}

fn command_fixture() -> Value {
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
            "created_at": "2026-08-27T12:00:00.000Z"
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
