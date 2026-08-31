use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ProjectScope, QueryOperation};
use crate::release1_snapshot::SnapshotDescriptor;

pub const GET_STATISTICS_REQUEST_SCHEMA_ID: &str = "storyos.query.statistics.request.v1";
pub const GET_STATISTICS_RESPONSE_SCHEMA_ID: &str = "storyos.query.statistics.response.v1";

pub(super) const GET_STATISTICS: QueryOperation = QueryOperation {
    operation_id: "getStatistics",
    method: "GET",
    path: "/api/v1/projects/{project_id}/manuscript/statistics",
    request_schema: GET_STATISTICS_REQUEST_SCHEMA_ID,
    response_schema: GET_STATISTICS_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Rebuildable Chapter and manuscript statistics"),
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
        "storyos.golden.getStatistics.positive.v1",
        "storyos.golden.getStatistics.invalid.v1",
        "storyos.golden.getStatistics.boundary.v1",
    ],
};

pub const GET_STATISTICS_PATH: &str = GET_STATISTICS.path;
pub const GET_STATISTICS_METHOD: &str = GET_STATISTICS.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ManuscriptStatisticsCompleteness {
    Complete,
    Truncated,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ChapterStatistics {
    pub chapter_id: String,
    pub word_count: String,
    pub character_count: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ManuscriptTotals {
    pub chapter_count: String,
    pub word_count: String,
    pub character_count: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetStatisticsResponse {
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
    pub completeness: ManuscriptStatisticsCompleteness,
    pub lag: String,
    pub counting_profile: String,
    pub current_chapter: Option<ChapterStatistics>,
    pub manuscript: ManuscriptTotals,
    pub next_cursor: Option<String>,
    pub page_count: String,
    pub page_bytes: String,
    pub redaction_profile: String,
    pub limit_profile_revision: String,
}
