use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_operation_registry::{
    FixtureMembership, OperationArtifacts, RegisteredOperation, method, operation_schemas,
};
use crate::release1_replan_proposal::{
    REPLAN_PROPOSAL, REPLAN_PROPOSAL_DIGEST_PROFILE, REPLAN_PROPOSAL_REQUEST_SCHEMA_ID,
    REPLAN_PROPOSAL_RESPONSE_SCHEMA_ID, ReplanProposalConflictReason, ReplanProposalEffect,
    ReplanProposalInput, ReplanProposalRefusalReason, ReplanProposalRequest,
    ReplanProposalResponse, ReplanReceipt, ReplanReceiptResult, ReplanSourceCondition,
};
use crate::release1_route_catalog::{
    AuthorCause, CauseProfiles, CurrentProducerCause, OperationCatalog, Owner,
};
use crate::release1_wire::{generated_ref, json_bytes, schema_value};

const CAUSES: CauseProfiles = CauseProfiles {
    author: AuthorCause {
        semantic_payload: "explicit author ReplanProposal cause",
        request_schema: "storyos.command.replan-proposal.author-cause.request.v1",
        response_schema: "storyos.command.replan-proposal.author-cause.response.v1",
    },
    current_producer: CurrentProducerCause {
        semantic_cause: "Exact current Proposal Revision producer: AgentRunDecision",
        semantic_payload: "exact current producer ReplanProposal cause",
        cause_binding: "exact_current_proposal_revision_producer",
        request_schema: "storyos.command.replan-proposal.current-producer-cause.request.v1",
        response_schema: "storyos.command.replan-proposal.current-producer-cause.response.v1",
        recovery_rule: "The request binds the exact current AgentRunDecision producer and never fabricates an Editor Session, writer generation, or AuthorCommandAdmission.",
    },
};

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::command(
        &REPLAN_PROPOSAL,
        &[
            "server_derived_project_scope",
            "project_active",
            "editor_session_writer_generation",
            "exact_conflict_or_recovery_conflict_source",
            "expected_current_proposal_head",
            "expected_current_target_revisions",
            "explicit_editor_control",
        ],
        OperationCatalog::conditional_by_cause("proposal:replan", Owner::Core, &CAUSES),
    )],
    schemas: || {
        operation_schemas(
            &REPLAN_PROPOSAL,
            (REQUEST_SCHEMA_PATH, request_schema_bytes()),
            (RESPONSE_SCHEMA_PATH, response_schema_bytes()),
        )
        .into()
    },
    openapi: || method(&REPLAN_PROPOSAL, openapi()),
    typescript_types: typescript_type_declarations,
    fixtures: &[FixtureMembership {
        path: FIXTURE_PATH,
        bytes: |_| fixture_bytes(),
    }],
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/replan-proposal-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/replan-proposal-response.schema.json";
pub(super) const FIXTURE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/replan-proposal.json";

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<ReplanProposalRequest>(
        REPLAN_PROPOSAL_REQUEST_SCHEMA_ID,
        "StoryOS Replan Proposal Request",
    );
    schema["properties"]["command_schema"]["const"] = json!(REPLAN_PROPOSAL_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["ReplanProposalInput"]["properties"];
    input["conflicted_proposal_revision_id"]["format"] = json!("uuid");
    input["expected_current_proposal_head"]["format"] = json!("uuid");
    input["editor_session_id"]["format"] = json!("uuid");
    input["correlation_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<ReplanProposalResponse>(
        REPLAN_PROPOSAL_RESPONSE_SCHEMA_ID,
        "StoryOS Replan Proposal Response",
    );
    schema["properties"]["schema_id"]["const"] = json!(REPLAN_PROPOSAL_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = REPLAN_PROPOSAL
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
            "    post:\n      operationId: {}\n      summary: Replan a conflicted Proposal\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: proposal_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        REPLAN_PROPOSAL.operation_id, request_schema, responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        ReplanSourceCondition::decl(&config),
        ReplanProposalInput::decl(&config),
        ReplanProposalRequest::decl(&config),
        ReplanReceiptResult::decl(&config),
        ReplanReceipt::decl(&config),
        ReplanProposalRefusalReason::decl(&config),
        ReplanProposalConflictReason::decl(&config),
        ReplanProposalEffect::decl(&config),
        ReplanProposalResponse::decl(&config),
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture())
}

fn command_fixture() -> Value {
    json!({
        "schema_id": REPLAN_PROPOSAL_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000e20",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "command_id": "018f0000-0000-7001-8000-000000000e21",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000e22",
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000e23",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000201"
            },
            "command_digest": {
                "algorithm": "sha256",
                "profile": REPLAN_PROPOSAL_DIGEST_PROFILE,
                "value_hex_lowercase": "e".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000e24",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000e22",
            "proposal_id": "018f0000-0000-7001-8000-000000000b02",
            "source_proposal_revision_id": "018f0000-0000-7001-8000-000000000b03",
            "resulting_proposal_revision_id": "018f0000-0000-7001-8000-000000000e26",
            "expected_current_target_revisions": ["018f0000-0000-7001-8000-000000000b06"],
            "prior_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
            "resulting_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
            "authoritative_commit_ids": [],
            "result": "resolved",
            "created_at": "2026-09-27T12:00:00.000Z"
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
            "author_action_sequence": "5",
            "undo_disposition": "forward",
            "resulting_proposal_revision_id": "018f0000-0000-7001-8000-000000000e26",
            "resulting_validation": "pending",
            "preserved_generation": "ready",
            "preserved_closure": "open",
            "source_condition": {
                "kind": "proposal_conflict",
                "proposal_conflict_ref": "018f0000-0000-7001-8000-000000000e25"
            },
            "state_event_refs": ["018f0000-0000-7001-8000-000000000e27"]
        }
    })
}
