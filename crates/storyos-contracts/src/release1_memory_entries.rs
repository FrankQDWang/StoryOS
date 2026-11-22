//! The wire forms of the query that lists the current Memory Entries of a Project for the settings
//! page, and of the command that deletes one entry (ADR 0094 sections 6 and 9).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, HttpMethod, QueryOperation};
use crate::release1_author_edit::DomainReceipt;

pub const LIST_MEMORY_ENTRIES_REQUEST_SCHEMA_ID: &str = "storyos.query.memory-entries.request.v1";
pub const LIST_MEMORY_ENTRIES_RESPONSE_SCHEMA_ID: &str = "storyos.query.memory-entries.response.v1";
pub const DELETE_MEMORY_ENTRY_REQUEST_SCHEMA_ID: &str =
    "storyos.command.delete-memory-entry.request.v1";
pub const DELETE_MEMORY_ENTRY_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.delete-memory-entry.response.v1";
pub const DELETE_MEMORY_ENTRY_DIGEST_PROFILE: &str = "storyos.command.deleteMemoryEntry.jcs.v1";

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

pub(super) const DELETE_MEMORY_ENTRY: QueryOperation = QueryOperation {
    operation_id: "deleteMemoryEntry",
    method: HttpMethod::Delete,
    path: "/api/v1/projects/{project_id}/memory-entries/{memory_entry_id}",
    request_schema: DELETE_MEMORY_ENTRY_REQUEST_SCHEMA_ID,
    response_schema: DELETE_MEMORY_ENTRY_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Memory Entry deleted"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency binding conflict"),
        (412, "Session binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Delete Memory Entry refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixture: "storyos.golden.deleteMemoryEntry.positive.v1",
};

pub const LIST_MEMORY_ENTRIES_PATH: &str = LIST_MEMORY_ENTRIES.path;
pub const LIST_MEMORY_ENTRIES_METHOD: &str = LIST_MEMORY_ENTRIES.method.as_str();
pub const DELETE_MEMORY_ENTRY_PATH: &str = DELETE_MEMORY_ENTRY.path;
pub const DELETE_MEMORY_ENTRY_METHOD: &str = DELETE_MEMORY_ENTRY.method.as_str();

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

/// The conflict flag of the current version of an entry (ADR 0094 section 7): the reason, the
/// source of the conflict, and the time of the flag version.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct MemoryEntryFlagInspect {
    pub reason: String,
    pub source: MemoryEntrySourceInspect,
    pub flagged_at: String,
}

/// A string or null. The field is required.
fn nullable_title_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"type": ["string", "null"]})
}

/// The Chapter at which an intent pass made its result, in the form of the Chapter source of an
/// entry: the `title` is `null` when the Chapter is unavailable.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct MemoryIntentChapterInspect {
    pub chapter_id: String,
    #[schemars(required, schema_with = "nullable_title_schema")]
    pub title: Option<String>,
    pub availability: MemorySourceAvailability,
}

/// The current result of the intent pass of a character entry of the background work (ADR 0101
/// sections 4 and 6): the intent, at most three questions to the author, the Chapter at which the
/// pass made the result, and the time of the result. The `chapter` is `null` when the pass had no
/// Chapter.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct MemoryEntryIntentInspect {
    pub text: String,
    pub questions: Vec<String>,
    #[schemars(required)]
    pub chapter: Option<MemoryIntentChapterInspect>,
    pub recorded_at: String,
}

/// The current version of one Memory Entry. The `source` is `null` when the version has no source
/// reference that the Host can decode. Only an entry with a conflict flag has `flag`. Only a
/// character entry of the background work with a current intent result has `intent`.
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
    // The field can be absent, but its value cannot be null.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "MemoryEntryFlagInspect")]
    #[ts(optional)]
    pub flag: Option<MemoryEntryFlagInspect>,
    // The field can be absent, but its value cannot be null.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "MemoryEntryIntentInspect")]
    #[ts(optional)]
    pub intent: Option<MemoryEntryIntentInspect>,
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

/// The entry of the route and the version of it that the page showed.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DeleteMemoryEntryInput {
    pub expected_version: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DeleteMemoryEntryRequest {
    pub command_schema: String,
    pub delete_memory_entry_input: DeleteMemoryEntryInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DeleteMemoryEntryNoEffectReason {
    AlreadyDeleted,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DeleteMemoryEntryConflictReason {
    EntryChanged,
}

/// The effect of one delete. An applied delete gives the `deleted` version of the entry and the
/// Memory revision of the Project after it.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeleteMemoryEntryEffect {
    AuthoritativeApplied {
        memory_entry_id: String,
        version: String,
        memory_revision: String,
        project_activity_position: String,
    },
    NoEffect {
        reason: DeleteMemoryEntryNoEffectReason,
    },
    Conflicted {
        reason: DeleteMemoryEntryConflictReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DeleteMemoryEntryResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: DomainReceipt,
    pub project: ControlledProject,
    pub effect: DeleteMemoryEntryEffect,
}
