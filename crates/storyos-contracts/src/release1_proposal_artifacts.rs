use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_proposal::{
    BlockProposalInspect, GET_PROPOSAL, GET_PROPOSAL_REQUEST_SCHEMA_ID,
    GET_PROPOSAL_RESPONSE_SCHEMA_ID, GetProposalResponse, OptionalValidationReceiptInspect,
    ProposalSourceInspect,
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/proposal-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/proposal-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/get-proposal.json",
    "generated/golden-wire/storyos-public-release-1/get-proposal.invalid.json",
    "generated/golden-wire/storyos-public-release-1/get-proposal.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    json_bytes(&json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": GET_PROPOSAL_REQUEST_SCHEMA_ID,
        "title": "StoryOS Proposal Request",
        "type": "object",
        "additionalProperties": false,
        "required": ["project_id", "proposal_id"],
        "properties": {
            "project_id": {"type": "string", "format": "uuid"},
            "proposal_id": {"type": "string", "format": "uuid"}
        }
    }))
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = serde_json::to_value(schema_for!(GetProposalResponse))
        .expect("proposal response schema serializes");
    schema["$id"] = json!(GET_PROPOSAL_RESPONSE_SCHEMA_ID);
    schema["title"] = json!("StoryOS Proposal Response");
    schema["additionalProperties"] = json!(false);
    schema["properties"]["schema_id"]["const"] = json!(GET_PROPOSAL_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    if let Some(scope) = schema["$defs"].get_mut("ProjectScope") {
        scope["properties"]["owner_user_id"]["format"] = json!("uuid");
        scope["properties"]["project_id"]["format"] = json!("uuid");
    }
    if let Some(proposal) = schema["$defs"].get_mut("BlockProposalInspect") {
        for field in [
            "proposal_id",
            "revision_id",
            "operation_id",
            "chapter_id",
            "manuscript_block_id",
            "base_authoritative_revision_id",
        ] {
            proposal["properties"][field]["format"] = json!("uuid");
        }
        proposal["properties"]["kind"]["const"] = json!("block_edit");
    }
    if let Some(source) = schema["$defs"].get_mut("ProposalSourceInspect") {
        source["properties"]["run_id"]["format"] = json!("uuid");
        source["properties"]["decision_id"]["format"] = json!("uuid");
    }
    if let Some(receipt) = schema["$defs"].get_mut("OptionalValidationReceiptInspect") {
        receipt["properties"]["validation_receipt_id"]["format"] = json!("uuid");
    }
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let response_schema = RESPONSE_SCHEMA_PATH
        .strip_prefix("generated/")
        .expect("schema is a generated artifact");
    let responses = GET_PROPOSAL
        .responses
        .iter()
        .map(|(status, description)| {
            let content = if *status == 200 {
                format!(
                    "          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n"
                )
            } else {
                String::new()
            };
            format!("        '{status}':\n          description: {description}\n{content}")
        })
        .collect::<String>();
    format!(
        concat!(
            "  {}:\n    get:\n      operationId: {}\n      summary: Inspect one current Block Proposal\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: proposal_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "      responses:\n{}",
        ),
        GET_PROPOSAL.path, GET_PROPOSAL.operation_id, responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}",
        ProposalSourceInspect::decl(&config),
        OptionalValidationReceiptInspect::decl(&config),
        BlockProposalInspect::decl(&config),
        GetProposalResponse::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    let path = GET_PROPOSAL
        .path
        .replace("{project_id}", "${encodeURIComponent(projectId)}")
        .replace("{proposal_id}", "${encodeURIComponent(proposalId)}");
    format!(
        concat!(
            "\nexport async function getProposal({{ projectId, proposalId, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"getProposal requires projectId\");\n",
            "  if (typeof proposalId !== \"string\" || proposalId.length === 0) throw new TypeError(\"getProposal requires proposalId\");\n",
            "  return queryJson({{ ...options, path: `{path}` }});\n}}\n"
        ),
        path = path
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    "export declare function getProposal(options: StoryOSQueryOptions & { projectId: string; proposalId: string }): Promise<GetProposalResponse>;\n"
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&proposal_fixture())
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = proposal_fixture();
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("proposal");
    json_bytes(&value)
}

pub(super) fn boundary_fixture_bytes() -> Vec<u8> {
    let mut boundary = proposal_fixture();
    boundary["proposal"]["validation"] = json!("pending");
    boundary["proposal"]["validation_receipt"] = json!({ "kind": "absent" });
    json_bytes(&boundary)
}

fn proposal_fixture() -> Value {
    json!({
        "schema_id": GET_PROPOSAL_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000b01",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal": {
            "proposal_id": "018f0000-0000-7001-8000-000000000b02",
            "kind": "block_edit",
            "revision_id": "018f0000-0000-7001-8000-000000000b03",
            "generation": "ready",
            "validation": "valid",
            "closure": "open",
            "operation_id": "018f0000-0000-7001-8000-000000000b04",
            "operation_resolution": "pending",
            "chapter_id": "018f0000-0000-7001-8000-000000000301",
            "manuscript_block_id": "018f0000-0000-7001-8000-000000000b05",
            "base_authoritative_revision_id": "018f0000-0000-7001-8000-000000000b06",
            "reservation_state": "unresolved",
            "candidate_text": "Guard the narrator voice in this passage.",
            "source": {
                "kind": "agent_run_decision",
                "run_id": "018f0000-0000-7001-8000-000000000a34",
                "decision_id": "018f0000-0000-7001-8000-000000000b07"
            },
            "validation_receipt": {
                "kind": "present",
                "validation_receipt_id": "018f0000-0000-7001-8000-000000000b08",
                "result": "valid"
            }
        }
    })
}

fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("contract JSON should serialize");
    bytes.push(b'\n');
    bytes
}
