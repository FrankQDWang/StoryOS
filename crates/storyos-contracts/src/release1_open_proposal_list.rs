use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{HttpMethod, ProjectScope, QueryOperation};

pub const LIST_CHAPTER_OPEN_PROPOSALS_REQUEST_SCHEMA_ID: &str =
    "storyos.query.chapter-open-proposals.request.v1";
pub const LIST_CHAPTER_OPEN_PROPOSALS_RESPONSE_SCHEMA_ID: &str =
    "storyos.query.chapter-open-proposals.response.v1";
/// The fixed maximum number of entries in one open Proposal list.
pub const OPEN_PROPOSAL_LIST_MAX_ENTRIES: usize = 64;

pub(super) const LIST_CHAPTER_OPEN_PROPOSALS: QueryOperation = QueryOperation {
    operation_id: "listChapterOpenProposals",
    method: HttpMethod::Get,
    path: "/api/v1/projects/{project_id}/chapters/{chapter_id}/open-proposals",
    request_schema: LIST_CHAPTER_OPEN_PROPOSALS_REQUEST_SCHEMA_ID,
    response_schema: LIST_CHAPTER_OPEN_PROPOSALS_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Open Proposals of one Chapter"),
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
    fixture: "storyos.golden.listChapterOpenProposals.positive.v1",
};

pub const LIST_CHAPTER_OPEN_PROPOSALS_PATH: &str = LIST_CHAPTER_OPEN_PROPOSALS.path;
pub const LIST_CHAPTER_OPEN_PROPOSALS_METHOD: &str = LIST_CHAPTER_OPEN_PROPOSALS.method.as_str();

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OpenProposalSource {
    AgentRunDecision { run_id: String, decision_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct OpenProposalEntry {
    pub proposal_id: String,
    pub revision_id: String,
    pub kind: String,
    pub source: OpenProposalSource,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OpenProposalListCompleteness {
    Complete,
    Truncated,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ListChapterOpenProposalsResponse {
    pub schema_id: String,
    pub correlation_id: String,
    #[ts(type = "ProjectScope")]
    pub project_scope: ProjectScope,
    pub chapter_id: String,
    pub completeness: OpenProposalListCompleteness,
    pub proposals: Vec<OpenProposalEntry>,
}
