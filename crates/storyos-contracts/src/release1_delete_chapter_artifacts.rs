use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::ProjectActivityKind;
use crate::release1_delete_chapter::{
    DELETE_CHAPTER, DELETE_CHAPTER_DIGEST_PROFILE, DELETE_CHAPTER_REQUEST_SCHEMA_ID,
    DELETE_CHAPTER_RESPONSE_SCHEMA_ID, DeleteChapterConflictReason, DeleteChapterEffect,
    DeleteChapterInput, DeleteChapterNoEffectReason, DeleteChapterRefusalReason,
    DeleteChapterRequest, DeleteChapterResponse,
};
use crate::release1_operation_registry::{
    FixtureMembership, OperationArtifacts, RegisteredOperation, method, operation_schemas,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_wire::{U64_WIRE, generated_ref, json_bytes, schema_value};

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::command(
        &DELETE_CHAPTER,
        &[
            "server_derived_project_scope",
            "chapter_scope_join",
            "expected_tree_revision",
        ],
        OperationCatalog::new("chapter:delete", Owner::Release, Binding::PROJECT_COMMAND)
            .activity(&[ProjectActivityKind::ChapterDeleted], &[]),
    )],
    schemas: || {
        operation_schemas(
            &DELETE_CHAPTER,
            (REQUEST_SCHEMA_PATH, request_schema_bytes()),
            (RESPONSE_SCHEMA_PATH, response_schema_bytes()),
        )
        .into()
    },
    openapi: || method(&DELETE_CHAPTER, method_openapi()),
    typescript_types: typescript_type_declarations,
    fixtures: &[FixtureMembership {
        path: FIXTURE_PATH,
        bytes: |_| fixture_bytes(),
    }],
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/delete-chapter-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/delete-chapter-response.schema.json";
pub(super) const FIXTURE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/delete-chapter.json";

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<DeleteChapterRequest>(
        DELETE_CHAPTER_REQUEST_SCHEMA_ID,
        "StoryOS Delete Chapter Request",
    );
    schema["properties"]["command_schema"]["const"] = json!(DELETE_CHAPTER_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["DeleteChapterInput"]["properties"];
    input["correlation_id"]["format"] = json!("uuid");
    input["expected_tree_revision"] = json!({"type": "string", "pattern": U64_WIRE});
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<DeleteChapterResponse>(
        DELETE_CHAPTER_RESPONSE_SCHEMA_ID,
        "StoryOS Delete Chapter Response",
    );
    schema["properties"]["schema_id"]["const"] = json!(DELETE_CHAPTER_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn method_openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = DELETE_CHAPTER
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
            "    delete:\n      operationId: {}\n      summary: Remove one named Chapter\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: chapter_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        DELETE_CHAPTER.operation_id, request_schema, responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        DeleteChapterInput::decl(&config),
        DeleteChapterRequest::decl(&config),
        DeleteChapterNoEffectReason::decl(&config),
        DeleteChapterConflictReason::decl(&config),
        DeleteChapterRefusalReason::decl(&config),
        DeleteChapterEffect::decl(&config),
        DeleteChapterResponse::decl(&config),
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture())
}

fn command_fixture() -> Value {
    json!({
        "schema_id": DELETE_CHAPTER_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000b10",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "command_id": "018f0000-0000-7001-8000-000000000b11",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000b12",
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000b13",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000201"
            },
            "command_kind": "deleteChapter",
            "command_digest": {
                "algorithm": "sha256",
                "profile": DELETE_CHAPTER_DIGEST_PROFILE,
                "value_hex_lowercase": "e".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000b14",
            "producer_cause": "author_command_admission",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000b12",
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
                "kind": "current_chapter",
                "current_chapter_id": "018f0000-0000-7001-8000-000000000916"
            }
        },
        "effect": {
            "kind": "authoritative_applied",
            "chapter_id": "018f0000-0000-7001-8000-000000000917",
            "volume_id": "018f0000-0000-7001-8000-000000000815",
            "tree_revision": "5",
            "project_activity_position": "5"
        }
    })
}
