//! The wire form of the query that lists the current Memory Entries of a Project for the settings
//! page (ADR 0094 section 9).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{HttpMethod, QueryOperation};

pub const LIST_MEMORY_ENTRIES_REQUEST_SCHEMA_ID: &str = "storyos.query.memory-entries.request.v1";
pub const LIST_MEMORY_ENTRIES_RESPONSE_SCHEMA_ID: &str = "storyos.query.memory-entries.response.v1";

pub(super) const LIST_MEMORY_ENTRIES: QueryOperation = QueryOperation {
    operation_id: "listMemoryEntries",
    method: HttpMethod::Get,
    path: "/api/v1/projects/{project_id}/memory-entries",
    request_schema: LIST_MEMORY_ENTRIES_REQUEST_SCHEMA_ID,
    response_schema: LIST_MEMORY_ENTRIES_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Current Memory Entries"),
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
    fixture: "storyos.golden.listMemoryEntries.positive.v1",
};

pub const LIST_MEMORY_ENTRIES_PATH: &str = LIST_MEMORY_ENTRIES.path;
pub const LIST_MEMORY_ENTRIES_METHOD: &str = LIST_MEMORY_ENTRIES.method.as_str();

/// The type of a Memory Entry (ADR 0094 section 3), in the order of the Memory Index.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MemoryEntryTypeInspect {
    WritingPreference,
    Foreshadowing,
    Character,
    Relationship,
    MajorEvent,
    Other,
}

/// What made the current version of an entry.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MemoryEntryOriginInspect {
    AuthorRequest,
    Conversation,
    Chapter,
}

/// `available` when the Project has the record of the source now (Memory contract, section 10).
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MemorySourceAvailability {
    Available,
    Unavailable,
}

/// The first source reference of the current version of an entry. The `title` of a Chapter is
/// `null` when the Chapter is unavailable.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MemoryEntrySourceInspect {
    Conversation {
        conversation_id: String,
        availability: MemorySourceAvailability,
    },
    Chapter {
        chapter_id: String,
        title: Option<String>,
        availability: MemorySourceAvailability,
    },
}

/// The current version of one Memory Entry. The `source` is `null` when the version has no source
/// reference that the Host can decode.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct MemoryEntryInspect {
    pub memory_entry_id: String,
    pub entry_number: String,
    pub version: String,
    pub entry_type: MemoryEntryTypeInspect,
    pub title: String,
    pub description: String,
    pub text: String,
    pub origin: MemoryEntryOriginInspect,
    #[schemars(required)]
    pub source: Option<MemoryEntrySourceInspect>,
    pub changed_at: String,
}

/// The current entries of a Project in the order of the Memory Index. The Host limits bound the
/// list, so it has no pages.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ListMemoryEntriesResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub entries: Vec<MemoryEntryInspect>,
}
