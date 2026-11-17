use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One manuscript Tool use of a Run (ADR 0095 section 1). No entry has a result text or a
/// ToolCall identity.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManuscriptUseInspect {
    /// The Agent read the Chapter with a settled `read_chapter` call. The title is the title at
    /// the time of the read.
    ChapterRead { chapter_id: String, title: String },
    /// The Agent searched the manuscript for the exact text `query` with a settled
    /// `search_manuscript` call.
    KeywordSearch { query: String },
    /// The Agent searched the manuscript by meaning for the question `question` with a settled
    /// `hybrid_search_manuscript` call. The entry has no passage text, no vector, and no score.
    HybridSearch { question: String },
}
