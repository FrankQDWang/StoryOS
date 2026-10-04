use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_operation_registry::{
    OperationArtifacts, RegisteredOperation, fixture_triple, method, operation_schemas,
};
use crate::release1_replan_proposal::{
    REPLAN_PROPOSAL, REPLAN_PROPOSAL_DIGEST_PROFILE, REPLAN_PROPOSAL_REQUEST_SCHEMA_ID,
    REPLAN_PROPOSAL_RESPONSE_SCHEMA_ID, ReplanProposalConflictReason, ReplanProposalEffect,
    ReplanProposalInput, ReplanProposalRefusalReason, ReplanProposalRequest,
    ReplanProposalResponse, ReplanReceipt, ReplanReceiptResult, ReplanSourceCondition,
};
use crate::release1_wire::{generated_ref, json_bytes, schema_value};

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
    typescript_client: typescript_client_source,
    typescript_declarations,
    fixtures: || {
        fixture_triple(
            FIXTURE_PATHS,
            &REPLAN_PROPOSAL,
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
    "generated/json-schema/storyos-public-release-1/replan-proposal-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/replan-proposal-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/replan-proposal.json",
    "generated/golden-wire/storyos-public-release-1/replan-proposal.invalid.json",
    "generated/golden-wire/storyos-public-release-1/replan-proposal.boundary.json",
];

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

pub(super) fn typescript_client_source() -> String {
    format!(
        concat!(
            "\nexport async function digestReplanProposal(request, cryptoImpl = globalThis.crypto) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"digestReplanProposal requires request\");\n",
            "  const canonical = canonicalJson(request);\n",
            "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
            "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
            "  return {{ algorithm: \"sha256\", profile: \"{}\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
            "\nexport async function replanProposal({{ projectId, proposalId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"replanProposal requires projectId\");\n",
            "  if (typeof proposalId !== \"string\" || proposalId.length === 0) throw new TypeError(\"replanProposal requires proposalId\");\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"replanProposal requires request\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"replanProposal requires security bindings\");\n",
            "  return commandJson({{ ...options, method: \"POST\", path: `{}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
        ),
        REPLAN_PROPOSAL_DIGEST_PROFILE,
        REPLAN_PROPOSAL
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}")
            .replace("{proposal_id}", "${encodeURIComponent(proposalId)}"),
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    concat!(
        "export declare function digestReplanProposal(request: ReplanProposalRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function replanProposal(options: StoryOSQueryOptions & { projectId: string; proposalId: string; request: ReplanProposalRequest; idempotencyKey: string; antiForgery: string }): Promise<ReplanProposalResponse>;\n",
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture("2026-09-27T12:00:00.000Z"))
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = command_fixture("2026-09-27T12:00:00.000Z");
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
            "created_at": created_at
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
