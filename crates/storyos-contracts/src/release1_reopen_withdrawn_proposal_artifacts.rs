use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_operation_registry::{
    OperationArtifacts, RegisteredOperation, fixture_triple, operation_schemas, path_items,
};
use crate::release1_reopen_withdrawn_proposal::{
    REOPEN_WITHDRAWN_PROPOSAL, REOPEN_WITHDRAWN_PROPOSAL_DIGEST_PROFILE,
    REOPEN_WITHDRAWN_PROPOSAL_REQUEST_SCHEMA_ID, REOPEN_WITHDRAWN_PROPOSAL_RESPONSE_SCHEMA_ID,
    ReopenWithdrawnProposalConflictReason, ReopenWithdrawnProposalEffect,
    ReopenWithdrawnProposalInput, ReopenWithdrawnProposalNoEffectReason,
    ReopenWithdrawnProposalRefusalReason, ReopenWithdrawnProposalRequest,
    ReopenWithdrawnProposalResponse, ReopenWithdrawnReceipt, ReopenWithdrawnReceiptResult,
};
use crate::release1_wire::{generated_ref, json_bytes, schema_value};

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::command(
        &REOPEN_WITHDRAWN_PROPOSAL,
        &[
            "proposal_scope_join",
            "current_closure_withdrawn",
            "exact_withdrawal_event_ref",
            "expected_target_revisions",
        ],
    )],
    schemas: || {
        operation_schemas(
            &REOPEN_WITHDRAWN_PROPOSAL,
            (REQUEST_SCHEMA_PATH, request_schema_bytes()),
            (RESPONSE_SCHEMA_PATH, response_schema_bytes()),
        )
        .into()
    },
    openapi: || path_items(openapi()),
    typescript_types: typescript_type_declarations,
    typescript_client: typescript_client_source,
    typescript_declarations,
    fixtures: || {
        fixture_triple(
            FIXTURE_PATHS,
            &REOPEN_WITHDRAWN_PROPOSAL,
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
    "generated/json-schema/storyos-public-release-1/reopen-withdrawn-proposal-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/reopen-withdrawn-proposal-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/reopen-withdrawn-proposal.json",
    "generated/golden-wire/storyos-public-release-1/reopen-withdrawn-proposal.invalid.json",
    "generated/golden-wire/storyos-public-release-1/reopen-withdrawn-proposal.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<ReopenWithdrawnProposalRequest>(
        REOPEN_WITHDRAWN_PROPOSAL_REQUEST_SCHEMA_ID,
        "StoryOS Reopen Withdrawn Proposal Request",
    );
    schema["properties"]["command_schema"]["const"] =
        json!(REOPEN_WITHDRAWN_PROPOSAL_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["ReopenWithdrawnProposalInput"]["properties"];
    input["proposal_revision_id"]["format"] = json!("uuid");
    input["withdrawal_event_ref"]["format"] = json!("uuid");
    input["expected_closure"]["const"] = json!("withdrawn");
    input["editor_session_id"]["format"] = json!("uuid");
    input["correlation_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<ReopenWithdrawnProposalResponse>(
        REOPEN_WITHDRAWN_PROPOSAL_RESPONSE_SCHEMA_ID,
        "StoryOS Reopen Withdrawn Proposal Response",
    );
    schema["properties"]["schema_id"]["const"] =
        json!(REOPEN_WITHDRAWN_PROPOSAL_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = REOPEN_WITHDRAWN_PROPOSAL
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
            "  {}:\n    post:\n      operationId: {}\n      summary: Reopen a withdrawn Proposal\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: proposal_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        REOPEN_WITHDRAWN_PROPOSAL.path,
        REOPEN_WITHDRAWN_PROPOSAL.operation_id,
        request_schema,
        responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        ReopenWithdrawnProposalInput::decl(&config),
        ReopenWithdrawnProposalRequest::decl(&config),
        ReopenWithdrawnReceiptResult::decl(&config),
        ReopenWithdrawnReceipt::decl(&config),
        ReopenWithdrawnProposalRefusalReason::decl(&config),
        ReopenWithdrawnProposalConflictReason::decl(&config),
        ReopenWithdrawnProposalNoEffectReason::decl(&config),
        ReopenWithdrawnProposalEffect::decl(&config),
        ReopenWithdrawnProposalResponse::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    format!(
        concat!(
            "\nexport async function digestReopenWithdrawnProposal(request, cryptoImpl = globalThis.crypto) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"digestReopenWithdrawnProposal requires request\");\n",
            "  const canonical = canonicalJson(request);\n",
            "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
            "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
            "  return {{ algorithm: \"sha256\", profile: \"{}\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
            "\nexport async function reopenWithdrawnProposal({{ projectId, proposalId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"reopenWithdrawnProposal requires projectId\");\n",
            "  if (typeof proposalId !== \"string\" || proposalId.length === 0) throw new TypeError(\"reopenWithdrawnProposal requires proposalId\");\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"reopenWithdrawnProposal requires request\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"reopenWithdrawnProposal requires security bindings\");\n",
            "  return commandJson({{ ...options, method: \"POST\", path: `{}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
        ),
        REOPEN_WITHDRAWN_PROPOSAL_DIGEST_PROFILE,
        REOPEN_WITHDRAWN_PROPOSAL
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}")
            .replace("{proposal_id}", "${encodeURIComponent(proposalId)}"),
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    concat!(
        "export declare function digestReopenWithdrawnProposal(request: ReopenWithdrawnProposalRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function reopenWithdrawnProposal(options: StoryOSQueryOptions & { projectId: string; proposalId: string; request: ReopenWithdrawnProposalRequest; idempotencyKey: string; antiForgery: string }): Promise<ReopenWithdrawnProposalResponse>;\n",
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture("2026-09-28T12:00:00.000Z"))
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = command_fixture("2026-09-28T12:00:00.000Z");
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
        "schema_id": REOPEN_WITHDRAWN_PROPOSAL_RESPONSE_SCHEMA_ID,
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
                "profile": REOPEN_WITHDRAWN_PROPOSAL_DIGEST_PROFILE,
                "value_hex_lowercase": "e".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000e24",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000e22",
            "proposal_id": "018f0000-0000-7001-8000-000000000b02",
            "source_proposal_revision_id": "018f0000-0000-7001-8000-000000000b03",
            "resulting_proposal_revision_id": "018f0000-0000-7001-8000-000000000b09",
            "withdrawal_event_ref": "018f0000-0000-7001-8000-000000000e15",
            "expected_target_revisions": ["018f0000-0000-7001-8000-000000000b06"],
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
            "author_action_sequence": "4",
            "undo_disposition": "forward",
            "resulting_proposal_revision_id": "018f0000-0000-7001-8000-000000000b09",
            "prior_closure": "withdrawn",
            "resulting_closure": "open",
            "resulting_validation": "pending",
            "preserved_generation": "ready",
            "preserved_operation_resolution": "pending",
            "withdrawal_event_ref": "018f0000-0000-7001-8000-000000000e15",
            "state_event_refs": ["018f0000-0000-7001-8000-000000000b09"]
        }
    })
}
