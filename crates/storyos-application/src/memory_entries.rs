//! The query of the current Memory Entries of a Project for the settings page (ADR 0094 section
//! 9). The list has each current entry that the Agent can read. It does not read the
//! Conversation Memory Settings, and it has no pages: the Host limits of S7-D1a and S7-D1a2 bound
//! it.

use std::future::Future;

use storyos_core::{MemoryEntryOrigin, MemoryEntryType};

use crate::{ProjectReadError, ProjectScope};

/// The first source reference of the current version of an entry. `available` is true when the
/// Project has the record now (Memory contract, section 10).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryEntrySourceItem {
    Conversation {
        conversation_id: String,
        available: bool,
    },
    /// `title` is the title of the Chapter when it is in the manuscript tree now.
    Chapter {
        chapter_id: String,
        title: Option<String>,
        available: bool,
    },
}

/// The conflict flag of the current version of an entry (ADR 0094 section 7).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryEntryFlagItem {
    pub reason: String,
    pub source: MemoryEntrySourceItem,
    /// The time of the flag version, RFC 3339 in UTC with milliseconds.
    pub flagged_at: String,
}

/// The current version of one Memory Entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryEntryListItem {
    pub memory_entry_id: String,
    pub number: u64,
    pub version: u64,
    pub entry_type: MemoryEntryType,
    pub title: String,
    pub description: String,
    pub text: String,
    pub origin: MemoryEntryOrigin,
    /// `None` when the version has no source reference that decodes.
    pub source: Option<MemoryEntrySourceItem>,
    /// RFC 3339 in UTC with milliseconds.
    pub changed_at: String,
    /// `Some` when the current version is a flag version.
    pub flag: Option<MemoryEntryFlagItem>,
}

/// Reads the current Memory Entries under an already authenticated exact Project Scope. The read
/// writes no row.
pub trait MemoryEntryReader: Sync {
    /// Gives each current entry of the Project, or `None` when the Project is not visible in the
    /// Project Scope.
    fn read_memory_entries(
        &self,
        scope: &ProjectScope,
    ) -> impl Future<Output = Result<Option<Vec<MemoryEntryListItem>>, ProjectReadError>> + Send;
}

/// The current entries of the Project in the order of the Memory Index: the type list of ADR 0094
/// section 3, and then the entry number.
pub async fn list_memory_entries(
    reader: &impl MemoryEntryReader,
    scope: &ProjectScope,
) -> Result<Option<Vec<MemoryEntryListItem>>, ProjectReadError> {
    let mut entries = reader.read_memory_entries(scope).await?;
    if let Some(entries) = entries.as_mut() {
        entries.sort_by_key(|entry| (entry.entry_type, entry.number));
    }
    Ok(entries)
}
