use super::{
    DeleteMemoryEntry, DeleteMemoryEntryConflict, DeleteMemoryEntryNoEffect,
    DeleteMemoryEntryRefusal, LastMemoryVersion, delete_memory_entry,
};
use crate::{
    CurrentMemoryEntry, MemoryEntryState, MemoryEntryType, MemoryVersionWrite, MemoryWrite,
    MemoryWriteAction, TransitionOutcome, decide_memory_write,
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

#[test]
fn the_deleted_version_has_the_form_of_a_delete_by_write_memory() {
    let entry = CurrentMemoryEntry {
        memory_entry_id: "entry-a".to_owned(),
        number: 3,
        version: 2,
        entry_type: MemoryEntryType::Character,
        title: "阿青".to_owned(),
        description: "渡口的船夫。".to_owned(),
        text: "阿青在渡口撑船。".to_owned(),
    };
    let by_tool = decide_memory_write(
        &MemoryWrite {
            action: MemoryWriteAction::Delete,
            entry: Some(3),
            entry_type: None,
            title: None,
            description: None,
            text: None,
        },
        std::slice::from_ref(&entry),
        4,
    );
    let by_author = delete_memory_entry(&delete(Some(last(2, MemoryEntryState::Current)), 2));
    assert_eq!(Ok(by_author), by_tool.map(TransitionOutcome::Applied));
}
