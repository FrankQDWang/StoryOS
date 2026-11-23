//! The reads of the Subruns of a Run of the author (ADR 0103 section 3A, "The reads"): the field
//! `subruns` of the Run read, and the query of one Subrun.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{HttpMethod, ProjectScope, QueryOperation};

pub const GET_SUBRUN_REQUEST_SCHEMA_ID: &str = "storyos.query.subrun.request.v1";
pub const GET_SUBRUN_RESPONSE_SCHEMA_ID: &str = "storyos.query.subrun.response.v1";

const RESPONSES: &[(u16, &str)] = &[
    (200, "Record of one Subrun"),
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
];

pub(super) const GET_SUBRUN: QueryOperation = QueryOperation {
    operation_id: "getSubrun",
    method: HttpMethod::Get,
    path: "/api/v1/projects/{project_id}/subruns/{subrun_id}",
    request_schema: GET_SUBRUN_REQUEST_SCHEMA_ID,
    response_schema: GET_SUBRUN_RESPONSE_SCHEMA_ID,
    responses: RESPONSES,
    fixture: "storyos.golden.getSubrun.positive.v1",
};

pub const GET_SUBRUN_PATH: &str = GET_SUBRUN.path;
pub const GET_SUBRUN_METHOD: &str = GET_SUBRUN.method.as_str();

/// The state of a Subrun. `working`: the Subrun did not end, also while its parent Run is
/// paused. `completed`: the Subrun ended `completed` with a final answer. `no_result`: each other
/// end that is not a cancellation. `stopped`: the Subrun is `cancelled`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SubrunStateInspect {
    Working,
    Completed,
    NoResult,
    Stopped,
}

/// One Subrun Request of a Run, in the order of the `start_subrun` calls of the Run.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SubrunEntryInspect {
    pub subrun_id: String,
    pub title: String,
    pub state: SubrunStateInspect,
}

/// One entry of the RunSteps of a Subrun. No entry has a Chapter text, a passage text, or a
/// Memory Entry text.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SubrunStepInspect {
    /// A text that the Subrun wrote in a response beside its function calls.
    Text { text: String },
    /// A settled `read_chapter` call. The title is the title at the time of the read.
    ChapterRead { chapter_id: String, title: String },
    /// A settled `search_manuscript` call and the count of the matches of its result.
    KeywordSearch { query: String, matches: u32 },
    /// A settled `hybrid_search_manuscript` call and the count of the passages of its result.
    HybridSearch { question: String, passages: u32 },
    /// A settled `read_memory_entry` call. The title is the title at the time of the read.
    MemoryRead { title: String },
    /// A settled `search_memory` call and the count of the entries of its result.
    MemorySearch { query: String, entries: u32 },
}

/// The record of one Subrun. `step_count` is the count of its RunSteps that have a Model Attempt
/// with a settled response. `answer` is the final answer, only for the state `completed`.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetSubrunResponse {
    pub schema_id: String,
    pub correlation_id: String,
    #[ts(type = "ProjectScope")]
    pub project_scope: ProjectScope,
    pub subrun_id: String,
    pub title: String,
    pub brief: String,
    pub state: SubrunStateInspect,
    pub step_count: u32,
    pub steps: Vec<SubrunStepInspect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub answer: Option<String>,
}
