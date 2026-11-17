use super::{
    DeleteMemoryEntry, DeleteMemoryEntryConflict, DeleteMemoryEntryNoEffect,
    DeleteMemoryEntryRefusal, LastMemoryVersion, delete_memory_entry,
};
use crate::{
    MemoryEntryState, MemoryEntryType, MemoryVersionWrite, MemoryWriteAction, TransitionOutcome,
};

fn last(version: u64, state: MemoryEntryState) -> LastMemoryVersion {
    LastMemoryVersion {
        memory_entry_id: "entry-a".to_owned(),
        number: 3,
        version,
        state,
        entry_type: MemoryEntryType::Character,
        title: "阿青".to_owned(),
        description: "渡口的船夫。".to_owned(),
    }
}

fn delete(last: Option<LastMemoryVersion>, expected_version: u64) -> DeleteMemoryEntry {
    DeleteMemoryEntry {
        last,
        expected_version,
    }
}

#[test]
fn each_row_of_the_outcome_table_classifies_in_order() {
    let deleted = MemoryVersionWrite {
        action: MemoryWriteAction::Delete,
        memory_entry_id: Some("entry-a".to_owned()),
        number: 3,
        version: 3,
        state: MemoryEntryState::Deleted,
        entry_type: MemoryEntryType::Character,
        title: "阿青".to_owned(),
        description: "渡口的船夫。".to_owned(),
        text: String::new(),
    };
    let cases = [
        (
            delete(Some(last(2, MemoryEntryState::Current)), 2),
            TransitionOutcome::Applied(deleted),
        ),
        // A deleted entry gives no effect for each expected version.
        (
            delete(Some(last(3, MemoryEntryState::Deleted)), 2),
            TransitionOutcome::NoEffect(DeleteMemoryEntryNoEffect::AlreadyDeleted),
        ),
        (
            delete(Some(last(2, MemoryEntryState::Current)), 1),
            TransitionOutcome::Conflicted(DeleteMemoryEntryConflict::EntryChanged),
        ),
        (
            delete(None, 1),
            TransitionOutcome::Refused(DeleteMemoryEntryRefusal::EntryNotFound),
        ),
    ];
    for (command, expected) in cases {
        assert_eq!(delete_memory_entry(&command), expected);
    }
}
