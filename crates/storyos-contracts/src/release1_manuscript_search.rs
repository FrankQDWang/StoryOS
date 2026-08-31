use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ProjectScope, QueryOperation};
use crate::release1_snapshot::SnapshotDescriptor;

pub const SEARCH_MANUSCRIPT_REQUEST_SCHEMA_ID: &str = "storyos.query.manuscript-search.request.v1";
pub const SEARCH_MANUSCRIPT_RESPONSE_SCHEMA_ID: &str =
    "storyos.query.manuscript-search.response.v1";

pub(super) const SEARCH_MANUSCRIPT: QueryOperation = QueryOperation {
    operation_id: "searchManuscript",
    method: "POST",
    path: "/api/v1/projects/{project_id}/queries/manuscript-search",
    request_schema: SEARCH_MANUSCRIPT_REQUEST_SCHEMA_ID,
    response_schema: SEARCH_MANUSCRIPT_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Bounded manuscript search page"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Snapshot expired"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Request refused"),
        (429, "Rate limited"),
        (503, "Projection not ready"),
    ],
    fixtures: &[
        "storyos.golden.searchManuscript.positive.v1",
        "storyos.golden.searchManuscript.invalid.v1",
        "storyos.golden.searchManuscript.boundary.v1",
    ],
};

pub const SEARCH_MANUSCRIPT_PATH: &str = SEARCH_MANUSCRIPT.path;
pub const SEARCH_MANUSCRIPT_METHOD: &str = SEARCH_MANUSCRIPT.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ManuscriptSearchSelection {
    CurrentChapter,
    Manuscript,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SearchManuscriptRequest {
    pub schema_id: String,
    pub selection: ManuscriptSearchSelection,
    pub query_text: String,
    #[serde(default)]
    pub required_watermark: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ManuscriptSearchMatch {
    pub chapter_id: String,
    pub manuscript_block_id: String,
    pub start: String,
    pub end: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ManuscriptSearchCompleteness {
    Complete,
    Truncated,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SearchManuscriptResponse {
    pub schema_id: String,
    pub query_id: String,
    pub correlation_id: String,
    #[ts(type = "ProjectScope")]
    pub project_scope: ProjectScope,
    #[ts(type = "SnapshotDescriptor")]
    pub source_snapshot: SnapshotDescriptor,
    pub projection_kind: String,
    pub projection_generation: String,
    pub projection_watermark: String,
    pub required_watermark: Option<String>,
    pub completeness: ManuscriptSearchCompleteness,
    pub lag: String,
    pub items: Vec<ManuscriptSearchMatch>,
    pub next_cursor: Option<String>,
    pub page_count: String,
    pub page_bytes: String,
    pub redaction_profile: String,
    pub limit_profile_revision: String,
}
