//! The Memory Entry of a Project (ADR 0094 section 3): its closed lists, its source references,
//! and the pin of the Memory revision of a Run. A Memory Entry is Data-only Context. No rule of
//! the Host reads its text as an instruction or as a permission.

/// The type of a Memory Entry. The order of the variants is the order of the groups of the
/// Memory Index (ADR 0094 section 4).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MemoryEntryType {
    WritingPreference,
    Foreshadowing,
    Character,
    Relationship,
    MajorEvent,
    Other,
}

impl MemoryEntryType {
    pub const ALL: [Self; 6] = [
        Self::WritingPreference,
        Self::Foreshadowing,
        Self::Character,
        Self::Relationship,
        Self::MajorEvent,
        Self::Other,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::WritingPreference => "writing_preference",
            Self::Foreshadowing => "foreshadowing",
            Self::Character => "character",
            Self::Relationship => "relationship",
            Self::MajorEvent => "major_event",
            Self::Other => "other",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}

/// The origin of a Memory Entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryEntryOrigin {
    AuthorRequest,
    Conversation,
    Chapter,
}

impl MemoryEntryOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuthorRequest => "author_request",
            Self::Conversation => "conversation",
            Self::Chapter => "chapter",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [Self::AuthorRequest, Self::Conversation, Self::Chapter]
            .into_iter()
            .find(|origin| origin.as_str() == value)
    }
}

/// The state of one version of a Memory Entry. A `Deleted` version keeps the title and the
/// description as the mark of the delete.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryEntryState {
    Current,
    Deleted,
}

impl MemoryEntryState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Deleted => "deleted",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [Self::Current, Self::Deleted]
            .into_iter()
            .find(|state| state.as_str() == value)
    }
}

/// One source reference of a Memory Entry version. A reference to a Project Conversation names
/// the Run and the ToolCall when the origin is the request of the author.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MemorySourceReference {
    Conversation {
        conversation_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        run_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_call_id: Option<String>,
    },
    Chapter {
        chapter_id: String,
    },
}

/// A source reference as a read gives it. A reference that does not decode, or that names a
/// record that the Project does not have now, is `Unavailable` (Memory contract, section 10).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemorySourceReferenceRead {
    Available(MemorySourceReference),
    Unavailable,
}

/// Decodes the source references of a version. The function `present` tells if the Project has
/// the record of a reference now. The read does not fail for a reference that is unavailable.
pub fn read_memory_source_references(
    value: &serde_json::Value,
    present: impl Fn(&MemorySourceReference) -> bool,
) -> Vec<MemorySourceReferenceRead> {
    let Some(references) = value.as_array() else {
        return Vec::new();
    };
    references
        .iter()
        .map(
            |reference| match serde_json::from_value::<MemorySourceReference>(reference.clone()) {
                Ok(decoded) if present(&decoded) => MemorySourceReferenceRead::Available(decoded),
                Ok(_) | Err(_) => MemorySourceReferenceRead::Unavailable,
            },
        )
        .collect()
}

/// The Memory revision that `createAgentRun` pins for a Run. A Run whose captured Conversation
/// Memory Settings do not have `use_enabled` has no pin. The revision 0 has no entry, so a
/// Project with no change of Memory gives no pin either, and its requests stay as before.
pub fn memory_revision_pin(use_enabled: bool, memory_revision: u64) -> Option<u64> {
    (use_enabled && memory_revision > 0).then_some(memory_revision)
}

#[cfg(test)]
#[path = "memory_entry_tests.rs"]
mod tests;
