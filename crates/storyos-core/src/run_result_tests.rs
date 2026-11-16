use super::*;
use crate::{MemoryEntryState, MemoryWriteAction};

fn at(decision_position: i64, call_position: i32) -> RunCallPosition {
    RunCallPosition {
        decision_position,
        call_position,
    }
}

/// The store gives the Proposals and the Memory writes from two queries. The rule puts each one at
/// the position of its call, and the answer last.
#[test]
fn each_result_goes_at_the_position_of_its_call_and_the_answer_is_last() {
    let calls = vec![
        (at(1, 0), "proposal"),
        (at(0, 1), "memory_write change"),
        (at(0, 0), "memory_write create"),
        (at(2, 0), "memory_write delete"),
    ];

    assert_eq!(
        run_results(calls, Some("answer")),
        [
            "memory_write create",
            "memory_write change",
            "proposal",
            "memory_write delete",
            "answer"
        ]
    );
    assert_eq!(
        run_results(vec![(at(0, 0), "memory_write create")], None),
        ["memory_write create"]
    );
}

#[test]
fn the_action_of_a_version_comes_from_its_state_and_whether_it_is_the_first() {
    assert_eq!(
        [
            MemoryWriteAction::of_version(MemoryEntryState::Current, true),
            MemoryWriteAction::of_version(MemoryEntryState::Current, false),
            MemoryWriteAction::of_version(MemoryEntryState::Deleted, false),
        ],
        [
            MemoryWriteAction::Create,
            MemoryWriteAction::Change,
            MemoryWriteAction::Delete
        ]
    );
}
