use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_operation_registry::{
    FixtureMembership, OperationArtifacts, RegisteredOperation, method, operation_schemas,
};
use crate::release1_reject_proposal_operations::{
    AuthorUndoDisposition, BoundedAuthorNote, ProposalRejectionReason, REJECT_PROPOSAL_OPERATIONS,
    REJECT_PROPOSAL_OPERATIONS_DIGEST_PROFILE, REJECT_PROPOSAL_OPERATIONS_REQUEST_SCHEMA_ID,
    REJECT_PROPOSAL_OPERATIONS_RESPONSE_SCHEMA_ID, RejectProposalOperationsConflictReason,
    RejectProposalOperationsEffect, RejectProposalOperationsInput,
    RejectProposalOperationsRefusalReason, RejectProposalOperationsRequest,
    RejectProposalOperationsResponse, RejectionReceipt, RejectionReceiptResult,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_wire::{generated_ref, json_bytes, schema_value};

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::command(
        &REJECT_PROPOSAL_OPERATIONS,
        &[
            "server_derived_project_scope",
            "project_active",
            "editor_session_writer_generation",
            "current_open_ready_proposal_revision",
            "selected_pending_operations",
            "expected_target_revisions",
            "explicit_editor_control",
        ],
        OperationCatalog::new(
            "proposal:reject-operations",
            Owner::Core,
            Binding::EDITOR_COMMAND,
        ),
    )],
    schemas: || {
        operation_schemas(
            &REJECT_PROPOSAL_OPERATIONS,
            (REQUEST_SCHEMA_PATH, request_schema_bytes()),
            (RESPONSE_SCHEMA_PATH, response_schema_bytes()),
        )
        .into()
    },
    openapi: || method(&REJECT_PROPOSAL_OPERATIONS, openapi()),
    typescript_types: typescript_type_declarations,
    fixtures: &[FixtureMembership {
        path: FIXTURE_PATH,
        bytes: |_| fixture_bytes(),
    }],
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/reject-proposal-operations-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str = "generated/json-schema/storyos-public-release-1/reject-proposal-operations-response.schema.json";
pub(super) const FIXTURE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/reject-proposal-operations.json";

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<RejectProposalOperationsRequest>(
        REJECT_PROPOSAL_OPERATIONS_REQUEST_SCHEMA_ID,
        "StoryOS Reject Proposal Operations Request",
    );
    schema["properties"]["command_schema"]["const"] =
        json!(REJECT_PROPOSAL_OPERATIONS_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["RejectProposalOperationsInput"]["properties"];
    input["proposal_revision_id"]["format"] = json!("uuid");
    input["editor_session_id"]["format"] = json!("uuid");
    input["correlation_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<RejectProposalOperationsResponse>(
        REJECT_PROPOSAL_OPERATIONS_RESPONSE_SCHEMA_ID,
        "StoryOS Reject Proposal Operations Response",
    );
    schema["properties"]["schema_id"]["const"] =
        json!(REJECT_PROPOSAL_OPERATIONS_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = REJECT_PROPOSAL_OPERATIONS
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
            "    post:\n      operationId: {}\n      summary: Reject one pending Proposal Operation\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: proposal_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        REJECT_PROPOSAL_OPERATIONS.operation_id, request_schema, responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        BoundedAuthorNote::decl(&config),
        ProposalRejectionReason::decl(&config),
        RejectProposalOperationsInput::decl(&config),
        RejectProposalOperationsRequest::decl(&config),
        RejectionReceiptResult::decl(&config),
        RejectionReceipt::decl(&config),
        RejectProposalOperationsRefusalReason::decl(&config),
        RejectProposalOperationsConflictReason::decl(&config),
        AuthorUndoDisposition::decl(&config),
        RejectProposalOperationsEffect::decl(&config),
        RejectProposalOperationsResponse::decl(&config),
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture())
}

fn command_fixture() -> Value {
    json!({
        "schema_id": REJECT_PROPOSAL_OPERATIONS_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000d10",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "command_id": "018f0000-0000-7001-8000-000000000d11",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000d12",
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000d13",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000201"
            },
            "command_digest": {
                "algorithm": "sha256",
                "profile": REJECT_PROPOSAL_OPERATIONS_DIGEST_PROFILE,
                "value_hex_lowercase": "d".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000d14",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000d12",
            "proposal_id": "018f0000-0000-7001-8000-000000000b02",
            "proposal_revision_id": "018f0000-0000-7001-8000-000000000b03",
            "selected_pending_operation_ids": ["018f0000-0000-7001-8000-000000000b04"],
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
            "author_action_sequence": "3",
            "undo_disposition": "forward",
            "operation_ids": ["018f0000-0000-7001-8000-000000000b04"],
            "prior_resolution": "pending",
            "resulting_resolution": "rejected",
            "rejection_reason": { "kind": "author_declined", "note": { "kind": "omitted" } },
            "preserved_generation": "ready",
            "preserved_validation": "valid",
            "preserved_closure": "open",
            "resolution_event_refs": ["018f0000-0000-7001-8000-000000000d15"]
        }
    })
}
