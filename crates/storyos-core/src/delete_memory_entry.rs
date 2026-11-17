//! Pure Core classification of one delete of a Memory Entry by the author on the settings page
//! (ADR 0094 sections 6 and 9).

use super::TransitionOutcome;
use crate::transition_outcome::reason_codes;
use crate::{MemoryEntryState, MemoryEntryType, MemoryVersionWrite, MemoryWriteAction};

/// The last version of one Memory Entry of the Project, in each state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LastMemoryVersion {
    pub memory_entry_id: String,
    pub number: u64,
    pub version: u64,
    pub state: MemoryEntryState,
    pub entry_type: MemoryEntryType,
    pub title: String,
    pub description: String,
}

/// The facts of one delete: the last version of the named entry, or `None` when the Project has
/// no entry with the identity, and the version that the page showed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteMemoryEntry {
    pub last: Option<LastMemoryVersion>,
    pub expected_version: u64,
}

pub type DeleteMemoryEntryResult = TransitionOutcome<
    MemoryVersionWrite,
    DeleteMemoryEntryNoEffect,
    DeleteMemoryEntryConflict,
    DeleteMemoryEntryRefusal,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeleteMemoryEntryNoEffect {
    AlreadyDeleted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeleteMemoryEntryConflict {
    EntryChanged,
}

/// A refusal that the adapter returns before Admission. No Receipt records it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeleteMemoryEntryRefusal {
    EntryNotFound,
}

reason_codes!(DeleteMemoryEntryNoEffect { AlreadyDeleted => "already_deleted" });
reason_codes!(DeleteMemoryEntryConflict { EntryChanged => "entry_changed" });

/// Classify one delete. The applied outcome is the next version of the entry with the state
/// `deleted`: it keeps the type, the title, and the description, and it has no text, as a delete
/// by `write_memory` does.
pub fn delete_memory_entry(command: &DeleteMemoryEntry) -> DeleteMemoryEntryResult {
    let Some(last) = &command.last else {
        return TransitionOutcome::Refused(DeleteMemoryEntryRefusal::EntryNotFound);
    };
    if last.state == MemoryEntryState::Deleted {
        return TransitionOutcome::NoEffect(DeleteMemoryEntryNoEffect::AlreadyDeleted);
    }
    if last.version != command.expected_version {
        return TransitionOutcome::Conflicted(DeleteMemoryEntryConflict::EntryChanged);
    }
    TransitionOutcome::Applied(MemoryVersionWrite {
        action: MemoryWriteAction::Delete,
        memory_entry_id: Some(last.memory_entry_id.clone()),
        number: last.number,
        version: last.version + 1,
        state: MemoryEntryState::Deleted,
        entry_type: last.entry_type,
        title: last.title.clone(),
        description: last.description.clone(),
        text: String::new(),
    })
}

#[cfg(test)]
#[path = "delete_memory_entry_tests.rs"]
mod tests;
