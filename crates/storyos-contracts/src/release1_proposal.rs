use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ProjectScope, QueryOperation};

pub const GET_PROPOSAL_REQUEST_SCHEMA_ID: &str = "storyos.query.proposal.request.v1";
pub const GET_PROPOSAL_RESPONSE_SCHEMA_ID: &str = "storyos.query.proposal.response.v1";

pub(super) const GET_PROPOSAL: QueryOperation = QueryOperation {
    operation_id: "getProposal",
    method: "GET",
    path: "/api/v1/projects/{project_id}/proposals/{proposal_id}",
    request_schema: GET_PROPOSAL_REQUEST_SCHEMA_ID,
    response_schema: GET_PROPOSAL_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Current Block Proposal"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Active release conflict"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Request refused"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.getProposal.positive.v1",
        "storyos.golden.getProposal.invalid.v1",
        "storyos.golden.getProposal.boundary.v1",
    ],
};

pub const GET_PROPOSAL_PATH: &str = GET_PROPOSAL.path;
pub const GET_PROPOSAL_METHOD: &str = GET_PROPOSAL.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetProposalRequest {}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProposalSourceInspect {
    AgentRunDecision { run_id: String, decision_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OptionalValidationReceiptInspect {
    Absent,
    Present {
        validation_receipt_id: String,
        result: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct BlockProposalInspect {
    pub proposal_id: String,
    pub kind: String,
    pub revision_id: String,
    pub generation: String,
    pub validation: String,
    pub closure: String,
    pub operation_id: String,
    pub operation_resolution: String,
    pub chapter_id: String,
    pub manuscript_block_id: String,
    pub base_authoritative_revision_id: String,
    pub reservation_state: String,
    pub candidate_text: String,
    pub source: ProposalSourceInspect,
    pub validation_receipt: OptionalValidationReceiptInspect,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetProposalResponse {
    pub schema_id: String,
    pub correlation_id: String,
    #[ts(type = "ProjectScope")]
    pub project_scope: ProjectScope,
    pub proposal: BlockProposalInspect,
}
