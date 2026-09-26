use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_proposal_generation_decision::{
    COMPLETE_READY_PARTIAL_PROPOSAL, COMPLETE_READY_PARTIAL_PROPOSAL_DIGEST_PROFILE,
    COMPLETE_READY_PARTIAL_PROPOSAL_REQUEST_SCHEMA_ID,
    COMPLETE_READY_PARTIAL_PROPOSAL_RESPONSE_SCHEMA_ID, CONTINUE_PROPOSAL_GENERATION,
    CONTINUE_PROPOSAL_GENERATION_DIGEST_PROFILE, CONTINUE_PROPOSAL_GENERATION_REQUEST_SCHEMA_ID,
    CONTINUE_PROPOSAL_GENERATION_RESPONSE_SCHEMA_ID, CompleteReadyPartialProposalEffect,
    CompleteReadyPartialProposalInput, CompleteReadyPartialProposalRefusalReason,
    CompleteReadyPartialProposalRequest, CompleteReadyPartialProposalResponse,
    ContinueProposalGenerationEffect, ContinueProposalGenerationInput,
    ContinueProposalGenerationRefusalReason, ContinueProposalGenerationRequest,
    ContinueProposalGenerationResponse, ProposalGenerationConflictReason,
    ProposalGenerationReceipt, ProposalGenerationReceiptResult, ProposalGenerationUndoDisposition,
};

pub(super) const COMPLETE_REQUEST_SCHEMA_PATH: &str = "generated/json-schema/storyos-public-release-1/complete-ready-partial-proposal-request.schema.json";
pub(super) const COMPLETE_RESPONSE_SCHEMA_PATH: &str = "generated/json-schema/storyos-public-release-1/complete-ready-partial-proposal-response.schema.json";
pub(super) const CONTINUE_REQUEST_SCHEMA_PATH: &str = "generated/json-schema/storyos-public-release-1/continue-proposal-generation-request.schema.json";
pub(super) const CONTINUE_RESPONSE_SCHEMA_PATH: &str = "generated/json-schema/storyos-public-release-1/continue-proposal-generation-response.schema.json";
pub(super) const COMPLETE_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/complete-ready-partial-proposal.json",
    "generated/golden-wire/storyos-public-release-1/complete-ready-partial-proposal.invalid.json",
    "generated/golden-wire/storyos-public-release-1/complete-ready-partial-proposal.boundary.json",
];
pub(super) const CONTINUE_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/continue-proposal-generation.json",
    "generated/golden-wire/storyos-public-release-1/continue-proposal-generation.invalid.json",
    "generated/golden-wire/storyos-public-release-1/continue-proposal-generation.boundary.json",
];

pub(super) fn complete_request_schema_bytes() -> Vec<u8> {
    request_schema::<CompleteReadyPartialProposalRequest>(
        COMPLETE_READY_PARTIAL_PROPOSAL_REQUEST_SCHEMA_ID,
        "StoryOS Complete Ready Partial Proposal Request",
        "CompleteReadyPartialProposalInput",
    )
}

pub(super) fn complete_response_schema_bytes() -> Vec<u8> {
    response_schema::<CompleteReadyPartialProposalResponse>(
        COMPLETE_READY_PARTIAL_PROPOSAL_RESPONSE_SCHEMA_ID,
        "StoryOS Complete Ready Partial Proposal Response",
    )
}

pub(super) fn continue_request_schema_bytes() -> Vec<u8> {
    request_schema::<ContinueProposalGenerationRequest>(
        CONTINUE_PROPOSAL_GENERATION_REQUEST_SCHEMA_ID,
        "StoryOS Continue Proposal Generation Request",
        "ContinueProposalGenerationInput",
    )
}

pub(super) fn continue_response_schema_bytes() -> Vec<u8> {
    response_schema::<ContinueProposalGenerationResponse>(
        CONTINUE_PROPOSAL_GENERATION_RESPONSE_SCHEMA_ID,
        "StoryOS Continue Proposal Generation Response",
    )
}

pub(super) fn openapi() -> String {
    format!(
        "{}{}",
        operation_openapi(
            &COMPLETE_READY_PARTIAL_PROPOSAL,
            COMPLETE_REQUEST_SCHEMA_PATH,
            COMPLETE_RESPONSE_SCHEMA_PATH,
            "Complete one ready partial Proposal",
        ),
        operation_openapi(
            &CONTINUE_PROPOSAL_GENERATION,
            CONTINUE_REQUEST_SCHEMA_PATH,
            CONTINUE_RESPONSE_SCHEMA_PATH,
            "Continue one Proposal Generation",
        ),
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        CompleteReadyPartialProposalInput::decl(&config),
        CompleteReadyPartialProposalRequest::decl(&config),
        ContinueProposalGenerationInput::decl(&config),
        ContinueProposalGenerationRequest::decl(&config),
        ProposalGenerationReceiptResult::decl(&config),
        ProposalGenerationReceipt::decl(&config),
        CompleteReadyPartialProposalRefusalReason::decl(&config),
        ContinueProposalGenerationRefusalReason::decl(&config),
        ProposalGenerationConflictReason::decl(&config),
        ProposalGenerationUndoDisposition::decl(&config),
        CompleteReadyPartialProposalEffect::decl(&config),
        ContinueProposalGenerationEffect::decl(&config),
        CompleteReadyPartialProposalResponse::decl(&config),
        ContinueProposalGenerationResponse::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    format!(
        "{}{}",
        client_source(
            "digestCompleteReadyPartialProposal",
            "completeReadyPartialProposal",
            COMPLETE_READY_PARTIAL_PROPOSAL_DIGEST_PROFILE,
            COMPLETE_READY_PARTIAL_PROPOSAL.path,
        ),
        client_source(
            "digestContinueProposalGeneration",
            "continueProposalGeneration",
            CONTINUE_PROPOSAL_GENERATION_DIGEST_PROFILE,
            CONTINUE_PROPOSAL_GENERATION.path,
        ),
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    concat!(
        "export declare function digestCompleteReadyPartialProposal(request: CompleteReadyPartialProposalRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function completeReadyPartialProposal(options: StoryOSQueryOptions & { projectId: string; proposalId: string; request: CompleteReadyPartialProposalRequest; idempotencyKey: string; antiForgery: string }): Promise<CompleteReadyPartialProposalResponse>;\n",
        "export declare function digestContinueProposalGeneration(request: ContinueProposalGenerationRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function continueProposalGeneration(options: StoryOSQueryOptions & { projectId: string; proposalId: string; request: ContinueProposalGenerationRequest; idempotencyKey: string; antiForgery: string }): Promise<ContinueProposalGenerationResponse>;\n",
    )
}

pub(super) fn complete_fixture_bytes() -> Vec<u8> {
    json_bytes(&complete_fixture("2026-09-25T12:00:00.000Z"))
}

pub(super) fn complete_invalid_fixture_bytes() -> Vec<u8> {
    let mut value = complete_fixture("2026-09-25T12:00:00.000Z");
    value.as_object_mut().expect("fixture").remove("project");
    json_bytes(&value)
}

pub(super) fn complete_boundary_fixture_bytes() -> Vec<u8> {
    json_bytes(&complete_fixture("1970-01-01T00:00:00.000Z"))
}

pub(super) fn continue_fixture_bytes() -> Vec<u8> {
    json_bytes(&continue_fixture("2026-09-25T12:00:00.000Z"))
}

pub(super) fn continue_invalid_fixture_bytes() -> Vec<u8> {
    let mut value = continue_fixture("2026-09-25T12:00:00.000Z");
    value.as_object_mut().expect("fixture").remove("project");
    json_bytes(&value)
}

pub(super) fn continue_boundary_fixture_bytes() -> Vec<u8> {
    json_bytes(&continue_fixture("1970-01-01T00:00:00.000Z"))
}

fn complete_fixture(created_at: &str) -> Value {
    let mut value = shared_fixture(
        COMPLETE_READY_PARTIAL_PROPOSAL_RESPONSE_SCHEMA_ID,
        COMPLETE_READY_PARTIAL_PROPOSAL_DIGEST_PROFILE,
        "proposal_generation_completed",
        created_at,
        "018f0000-0000-7001-8000-000000000e10",
    );
    value["effect"] = json!({
        "kind": "completed",
        "author_action_sequence": "4",
        "undo_disposition": "forward",
        "generation_id": "018f0000-0000-7001-8000-000000000e16",
        "prior_generation_state": "ready_partial",
        "resulting_generation_state": "ready",
        "preserved_validation": "pending",
        "preserved_closure": "open",
        "preserved_operation_resolution": "pending",
        "generation_event_ref": "018f0000-0000-7001-8000-000000000e17"
    });
    value
}

fn continue_fixture(created_at: &str) -> Value {
    let mut value = shared_fixture(
        CONTINUE_PROPOSAL_GENERATION_RESPONSE_SCHEMA_ID,
        CONTINUE_PROPOSAL_GENERATION_DIGEST_PROFILE,
        "proposal_generation_started",
        created_at,
        "018f0000-0000-7001-8000-000000000e20",
    );
    value["effect"] = json!({
        "kind": "started",
        "author_action_sequence": "5",
        "undo_disposition": "forward",
        "prior_generation_id": "018f0000-0000-7001-8000-000000000e26",
        "new_generation_id": "018f0000-0000-7001-8000-000000000e27",
        "prior_generation_state": "ready_partial",
        "resulting_generation_state": "generating",
        "prior_run_id": "018f0000-0000-7001-8000-000000000e28",
        "resulting_run_id": "018f0000-0000-7001-8000-000000000e29",
        "preserved_validation": "pending",
        "preserved_closure": "open",
        "preserved_operation_resolution": "pending",
        "generation_event_ref": "018f0000-0000-7001-8000-000000000e2a"
    });
    value
}

fn shared_fixture(
    schema_id: &str,
    profile: &str,
    result: &str,
    created_at: &str,
    prefix: &str,
) -> Value {
    json!({
        "schema_id": schema_id,
        "correlation_id": prefix,
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "command_id": "018f0000-0000-7001-8000-000000000e11",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000e12",
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000e13",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000201"
            },
            "command_digest": {
                "algorithm": "sha256",
                "profile": profile,
                "value_hex_lowercase": "e".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000e14",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000e12",
            "proposal_id": "018f0000-0000-7001-8000-000000000b02",
            "proposal_revision_id": "018f0000-0000-7001-8000-000000000b03",
            "expected_target_revisions": ["018f0000-0000-7001-8000-000000000b06"],
            "prior_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
            "resulting_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
            "authoritative_commit_ids": [],
            "result": result,
            "created_at": created_at
        },
        "project": {
            "project_id": "018f0000-0000-7001-8000-000000000201",
            "title": "Open Proposal Novel",
            "open": {
                "kind": "current_chapter",
                "current_chapter_id": "018f0000-0000-7001-8000-000000000301"
            }
        }
    })
}

fn request_schema<T: schemars::JsonSchema>(
    schema_id: &str,
    title: &str,
    input_name: &str,
) -> Vec<u8> {
    let mut schema = schema_value::<T>(schema_id, title);
    schema["properties"]["command_schema"]["const"] = json!(schema_id);
    let input = &mut schema["$defs"][input_name]["properties"];
    for field in [
        "proposal_revision_id",
        "generation_id",
        "prior_generation_id",
        "editor_session_id",
        "correlation_id",
    ] {
        if input.get(field).is_some() {
            input[field]["format"] = json!("uuid");
        }
    }
    json_bytes(&schema)
}

fn response_schema<T: schemars::JsonSchema>(schema_id: &str, title: &str) -> Vec<u8> {
    let mut schema = schema_value::<T>(schema_id, title);
    schema["properties"]["schema_id"]["const"] = json!(schema_id);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

fn operation_openapi(
    operation: &crate::release1::QueryOperation,
    request_path: &str,
    response_path: &str,
    summary: &str,
) -> String {
    let request_schema = generated_ref(request_path);
    let response_schema = generated_ref(response_path);
    let responses = operation
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
            "  {}:\n    post:\n      operationId: {}\n      summary: {}\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: proposal_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        operation.path, operation.operation_id, summary, request_schema, responses,
    )
}

fn client_source(digest_name: &str, function_name: &str, profile: &str, path: &str) -> String {
    let route = path
        .replace("{project_id}", "${encodeURIComponent(projectId)}")
        .replace("{proposal_id}", "${encodeURIComponent(proposalId)}");
    format!(
        "\nexport async function {digest_name}(request, cryptoImpl = globalThis.crypto) {{\n  if (!request || typeof request !== \"object\") throw new TypeError(\"{digest_name} requires request\");\n  const canonical = canonicalJson(request);\n  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n  return {{ algorithm: \"sha256\", profile: \"{profile}\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n\nexport async function {function_name}({{ projectId, proposalId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"{function_name} requires projectId\");\n  if (typeof proposalId !== \"string\" || proposalId.length === 0) throw new TypeError(\"{function_name} requires proposalId\");\n  if (!request || typeof request !== \"object\") throw new TypeError(\"{function_name} requires request\");\n  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"{function_name} requires security bindings\");\n  return commandJson({{ ...options, method: \"POST\", path: `{route}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
    )
}

fn generated_ref(path: &str) -> &str {
    path.strip_prefix("generated/")
        .expect("schema is a generated artifact")
}

fn schema_value<T: schemars::JsonSchema>(schema_id: &str, title: &str) -> Value {
    let mut schema = serde_json::to_value(schema_for!(T)).expect("contract schema serializes");
    schema["$id"] = Value::String(schema_id.to_owned());
    schema["title"] = Value::String(title.to_owned());
    schema
}

fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("contract JSON should serialize");
    bytes.push(b'\n');
    bytes
}
