use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_accept_proposal::{
    ACCEPT_PROPOSAL, ACCEPT_PROPOSAL_DIGEST_PROFILE, ACCEPT_PROPOSAL_REQUEST_SCHEMA_ID,
    ACCEPT_PROPOSAL_RESPONSE_SCHEMA_ID, AcceptProposalConflictReason, AcceptProposalEffect,
    AcceptProposalInput, AcceptProposalInvalidReason, AcceptProposalRefusalReason,
    AcceptProposalRequest, AcceptProposalResponse, AcceptanceReceipt, AcceptanceReceiptResult,
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/accept-proposal-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/accept-proposal-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/accept-proposal.json",
    "generated/golden-wire/storyos-public-release-1/accept-proposal.invalid.json",
    "generated/golden-wire/storyos-public-release-1/accept-proposal.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<AcceptProposalRequest>(
        ACCEPT_PROPOSAL_REQUEST_SCHEMA_ID,
        "StoryOS Accept Proposal Request",
    );
    schema["properties"]["command_schema"]["const"] = json!(ACCEPT_PROPOSAL_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["AcceptProposalInput"]["properties"];
    input["proposal_revision_id"]["format"] = json!("uuid");
    input["validation_receipt_id"]["format"] = json!("uuid");
    input["selected_operation_id"]["format"] = json!("uuid");
    input["expected_authoritative_revision_id"]["format"] = json!("uuid");
    input["editor_session_id"]["format"] = json!("uuid");
    input["correlation_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<AcceptProposalResponse>(
        ACCEPT_PROPOSAL_RESPONSE_SCHEMA_ID,
        "StoryOS Accept Proposal Response",
    );
    schema["properties"]["schema_id"]["const"] = json!(ACCEPT_PROPOSAL_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = ACCEPT_PROPOSAL
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
            "  {}:\n    post:\n      operationId: {}\n      summary: Accept one exact pending Proposal Operation\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: proposal_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        ACCEPT_PROPOSAL.path, ACCEPT_PROPOSAL.operation_id, request_schema, responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        AcceptProposalInput::decl(&config),
        AcceptProposalRequest::decl(&config),
        AcceptanceReceiptResult::decl(&config),
        AcceptanceReceipt::decl(&config),
        AcceptProposalRefusalReason::decl(&config),
        AcceptProposalInvalidReason::decl(&config),
        AcceptProposalConflictReason::decl(&config),
        AcceptProposalEffect::decl(&config),
        AcceptProposalResponse::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    format!(
        concat!(
            "\nexport async function digestAcceptProposal(request, cryptoImpl = globalThis.crypto) {{\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"digestAcceptProposal requires request\");\n",
            "  const canonical = canonicalJson(request);\n",
            "  const bytes = new TextEncoder().encode(JSON.stringify(canonical));\n",
            "  const digest = new Uint8Array(await cryptoImpl.subtle.digest(\"SHA-256\", bytes));\n",
            "  return {{ algorithm: \"sha256\", profile: \"{}\", value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, \"0\")).join(\"\") }};\n}}\n",
            "\nexport async function acceptProposal({{ projectId, proposalId, request, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"acceptProposal requires projectId\");\n",
            "  if (typeof proposalId !== \"string\" || proposalId.length === 0) throw new TypeError(\"acceptProposal requires proposalId\");\n",
            "  if (!request || typeof request !== \"object\") throw new TypeError(\"acceptProposal requires request\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"acceptProposal requires security bindings\");\n",
            "  return commandJson({{ ...options, method: \"POST\", path: `{}`, body: request, commandHeaders: {{ \"idempotency-key\": idempotencyKey, \"x-storyos-anti-forgery\": antiForgery }} }});\n}}\n",
        ),
        ACCEPT_PROPOSAL_DIGEST_PROFILE,
        ACCEPT_PROPOSAL
            .path
            .replace("{project_id}", "${encodeURIComponent(projectId)}")
            .replace("{proposal_id}", "${encodeURIComponent(proposalId)}"),
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    concat!(
        "export declare function digestAcceptProposal(request: AcceptProposalRequest, cryptoImpl?: Crypto): Promise<DigestValue>;\n",
        "export declare function acceptProposal(options: StoryOSQueryOptions & { projectId: string; proposalId: string; request: AcceptProposalRequest; idempotencyKey: string; antiForgery: string }): Promise<AcceptProposalResponse>;\n",
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture("2026-09-18T12:00:00.000Z"))
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = command_fixture("2026-09-18T12:00:00.000Z");
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
        "schema_id": ACCEPT_PROPOSAL_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000c10",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "command_id": "018f0000-0000-7001-8000-000000000c11",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000c12",
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000c13",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000201"
            },
            "command_digest": {
                "algorithm": "sha256",
                "profile": ACCEPT_PROPOSAL_DIGEST_PROFILE,
                "value_hex_lowercase": "c".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000c14",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000c12",
            "proposal_id": "018f0000-0000-7001-8000-000000000b02",
            "proposal_revision_id": "018f0000-0000-7001-8000-000000000b03",
            "validation_receipt_id": "018f0000-0000-7001-8000-000000000b08",
            "selected_operation_id": "018f0000-0000-7001-8000-000000000b04",
            "prior_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000b06"],
            "resulting_authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000c15"],
            "authoritative_commit_ids": ["018f0000-0000-7001-8000-000000000c16"],
            "result": "applied",
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
            "kind": "applied",
            "author_action_sequence": "2",
            "authoritative_commit_id": "018f0000-0000-7001-8000-000000000c16",
            "authoritative_revision": {
                "revision_id": "018f0000-0000-7001-8000-000000000c15",
                "body": "Keep the narrator voice in this passage.",
                "blocks": [{
                    "manuscript_block_id": "018f0000-0000-7001-8000-000000000b05",
                    "block_kind": "paragraph",
                    "text": "Keep the narrator voice in this passage."
                }]
            },
            "project_activity_position": "3"
        }
    })
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
