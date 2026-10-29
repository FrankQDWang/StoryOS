use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_operation_registry::{
    FixtureMembership, OperationArtifacts, RegisteredOperation, method, operation_schemas,
};
use crate::release1_reopen_rejected_operations::{
    REOPEN_REJECTED_OPERATIONS, REOPEN_REJECTED_OPERATIONS_DIGEST_PROFILE,
    REOPEN_REJECTED_OPERATIONS_REQUEST_SCHEMA_ID, REOPEN_REJECTED_OPERATIONS_RESPONSE_SCHEMA_ID,
    ReopenReceipt, ReopenReceiptResult, ReopenRejectedOperationsConflictReason,
    ReopenRejectedOperationsEffect, ReopenRejectedOperationsInput,
    ReopenRejectedOperationsRefusalReason, ReopenRejectedOperationsRequest,
    ReopenRejectedOperationsResponse,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_wire::{generated_ref, json_bytes, schema_value};

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::command(
        &REOPEN_REJECTED_OPERATIONS,
        &[
            "server_derived_project_scope",
            "project_active",
            "editor_session_writer_generation",
            "current_open_ready_proposal_revision",
            "selected_rejected_operations",
            "matching_rejection_event_refs",
            "expected_target_revisions",
            "explicit_editor_control",
        ],
        OperationCatalog::new(
            "proposal:reopen-rejected-operations",
            Owner::Core,
            Binding::EDITOR_COMMAND,
        ),
    )],
    schemas: || {
        operation_schemas(
            &REOPEN_REJECTED_OPERATIONS,
            (REQUEST_SCHEMA_PATH, request_schema_bytes()),
            (RESPONSE_SCHEMA_PATH, response_schema_bytes()),
        )
        .into()
    },
    openapi: || method(&REOPEN_REJECTED_OPERATIONS, openapi()),
    typescript_types: typescript_type_declarations,
    fixtures: &[FixtureMembership {
        path: FIXTURE_PATH,
        bytes: |_| fixture_bytes(),
    }],
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/reopen-rejected-operations-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str = "generated/json-schema/storyos-public-release-1/reopen-rejected-operations-response.schema.json";
pub(super) const FIXTURE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/reopen-rejected-operations.json";

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<ReopenRejectedOperationsRequest>(
        REOPEN_REJECTED_OPERATIONS_REQUEST_SCHEMA_ID,
        "StoryOS Reopen Rejected Operations Request",
    );
    schema["properties"]["command_schema"]["const"] =
        json!(REOPEN_REJECTED_OPERATIONS_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["ReopenRejectedOperationsInput"]["properties"];
    input["proposal_revision_id"]["format"] = json!("uuid");
    input["editor_session_id"]["format"] = json!("uuid");
    input["correlation_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<ReopenRejectedOperationsResponse>(
        REOPEN_REJECTED_OPERATIONS_RESPONSE_SCHEMA_ID,
        "StoryOS Reopen Rejected Operations Response",
    );
    schema["properties"]["schema_id"]["const"] =
        json!(REOPEN_REJECTED_OPERATIONS_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = REOPEN_REJECTED_OPERATIONS
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
            "    post:\n      operationId: {}\n      summary: Reopen rejected Proposal Operations\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: proposal_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        REOPEN_REJECTED_OPERATIONS.operation_id, request_schema, responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        ReopenRejectedOperationsInput::decl(&config),
        ReopenRejectedOperationsRequest::decl(&config),
        ReopenReceiptResult::decl(&config),
        ReopenReceipt::decl(&config),
        ReopenRejectedOperationsRefusalReason::decl(&config),
        ReopenRejectedOperationsConflictReason::decl(&config),
        ReopenRejectedOperationsEffect::decl(&config),
        ReopenRejectedOperationsResponse::decl(&config),
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture())
}

fn command_fixture() -> Value {
    json!({
        "schema_id": REOPEN_REJECTED_OPERATIONS_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000d20",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "command_id": "018f0000-0000-7001-8000-000000000d21",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000d22",
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000d23",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000201"
            },
            "command_digest": {
                "algorithm": "sha256",
                "profile": REOPEN_REJECTED_OPERATIONS_DIGEST_PROFILE,
                "value_hex_lowercase": "e".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000d24",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000d22",
            "proposal_id": "018f0000-0000-7001-8000-000000000b02",
            "source_proposal_revision_id": "018f0000-0000-7001-8000-000000000b03",
            "resulting_proposal_revision_id": "018f0000-0000-7001-8000-000000000b09",
            "selected_rejected_operation_ids": ["018f0000-0000-7001-8000-000000000b04"],
            "rejection_event_refs": ["018f0000-0000-7001-8000-000000000d15"],
            "expected_target_revisions": ["018f0000-0000-7001-8000-000000000b06"],
            "prior_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
            "resulting_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
            "authoritative_commit_ids": [],
            "result": "resolved",
            "created_at": "2026-09-19T12:00:00.000Z"
        },
        "project": {
            "project_id": "018f0000-0000-7001-8000-000000000201",
            "title": "Open Proposal Novel",
            "open": {
                "kind": "current_chapter",
                "current_chapter_id": "018f0000-0000-7001-8000-000000000301"
            }
        },
        "effect": {
            "kind": "resolved",
            "author_action_sequence": "4",
            "undo_disposition": "forward",
            "operation_ids": ["018f0000-0000-7001-8000-000000000b04"],
            "rejection_event_refs": ["018f0000-0000-7001-8000-000000000d15"],
            "prior_resolution": "rejected",
            "resulting_resolution": "pending",
            "resulting_proposal_revision_id": "018f0000-0000-7001-8000-000000000b09",
            "resulting_validation": "pending",
            "preserved_generation": "ready",
            "preserved_closure": "open",
            "state_event_refs": ["018f0000-0000-7001-8000-000000000d25"]
        }
    })
}
