use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{HttpMethod, ProjectScope, QueryOperation};

pub const LIST_RESEARCH_SOURCES_REQUEST_SCHEMA_ID: &str =
    "storyos.query.research-sources.request.v1";
pub const LIST_RESEARCH_SOURCES_RESPONSE_SCHEMA_ID: &str =
    "storyos.query.research-sources.response.v1";
pub const GET_RESEARCH_SOURCE_REQUEST_SCHEMA_ID: &str = "storyos.query.research-source.request.v1";
pub const GET_RESEARCH_SOURCE_RESPONSE_SCHEMA_ID: &str =
    "storyos.query.research-source.response.v1";

pub(super) const LIST_RESEARCH_SOURCES: QueryOperation = QueryOperation {
    operation_id: "listResearchSources",
    method: HttpMethod::Get,
    path: "/api/v1/projects/{project_id}/agent-runs/{run_id}/research-sources",
    request_schema: LIST_RESEARCH_SOURCES_REQUEST_SCHEMA_ID,
    response_schema: LIST_RESEARCH_SOURCES_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "The Source Snapshots and Source Gaps of one AgentRun"),
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
    fixture: "storyos.golden.listResearchSources.positive.v1",
};

pub(super) const GET_RESEARCH_SOURCE: QueryOperation = QueryOperation {
    operation_id: "getResearchSource",
    method: HttpMethod::Get,
    path: "/api/v1/projects/{project_id}/research-sources/{source_snapshot_id}",
    request_schema: GET_RESEARCH_SOURCE_REQUEST_SCHEMA_ID,
    response_schema: GET_RESEARCH_SOURCE_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "One Source Snapshot revision with its text"),
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
    fixture: "storyos.golden.getResearchSource.positive.v1",
};

pub const LIST_RESEARCH_SOURCES_PATH: &str = LIST_RESEARCH_SOURCES.path;
pub const LIST_RESEARCH_SOURCES_METHOD: &str = LIST_RESEARCH_SOURCES.method.as_str();
pub const GET_RESEARCH_SOURCE_PATH: &str = GET_RESEARCH_SOURCE.path;
pub const GET_RESEARCH_SOURCE_METHOD: &str = GET_RESEARCH_SOURCE.method.as_str();

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SourceSnapshotKind {
    SearchResult,
    Page,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SourceGapReason {
    Failed,
    Empty,
    Missing,
}

/// One immutable Source Snapshot revision, with no captured text.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SourceSnapshotInspect {
    pub source_snapshot_id: String,
    pub tool_call_id: String,
    pub kind: SourceSnapshotKind,
    // The URL that the server reported: the Evidence Locator.
    pub locator: String,
    pub title: Option<String>,
    pub published: Option<String>,
    pub captured_at: String,
    pub content_digest: String,
    // The byte length of the stored text, at most 256 KiB. StoryOS never cuts a source.
    pub text_bytes: String,
}

/// One requested URL with no Snapshot. It is never a citation.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SourceGapInspect {
    pub source_gap_id: String,
    pub tool_call_id: String,
    pub locator: String,
    pub reason: SourceGapReason,
    pub recorded_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ListResearchSourcesResponse {
    pub schema_id: String,
    pub correlation_id: String,
    #[ts(type = "ProjectScope")]
    pub project_scope: ProjectScope,
    pub run_id: String,
    // In ToolCall order, then in result order.
    pub snapshots: Vec<SourceSnapshotInspect>,
    // In ToolCall order, then in request order.
    pub gaps: Vec<SourceGapInspect>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetResearchSourceResponse {
    pub schema_id: String,
    pub correlation_id: String,
    #[ts(type = "ProjectScope")]
    pub project_scope: ProjectScope,
    pub snapshot: SourceSnapshotInspect,
    pub text: String,
}
