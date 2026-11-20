use super::{MemoryEntryOrigin, MemoryEntryState, MemoryEntryType, memory_revision_pin};

/// The closed lists of the baseline (ADR 0094 section 3) and their tokens.
#[test]
fn each_closed_list_has_its_tokens_and_refuses_another_token() {
    let types: Vec<_> = MemoryEntryType::ALL.map(MemoryEntryType::as_str).into();
    let origins = ["author_request", "conversation", "chapter"]
        .map(|token| MemoryEntryOrigin::parse(token).map(MemoryEntryOrigin::as_str));
    let states = ["current", "deleted"]
        .map(|token| MemoryEntryState::parse(token).map(MemoryEntryState::as_str));

    assert_eq!(
        (
            types.clone(),
            types
                .iter()
                .all(|token| MemoryEntryType::parse(token).is_some()),
            origins,
            states,
            MemoryEntryType::parse("fiction_assertion"),
            MemoryEntryOrigin::parse("capture"),
            MemoryEntryState::parse("superseded"),
        ),
        (
            vec![
                "writing_preference",
                "foreshadowing",
                "character",
                "relationship",
                "major_event",
                "other"
            ],
            true,
            [
                Some("author_request"),
                Some("conversation"),
                Some("chapter")
            ],
            [Some("current"), Some("deleted")],
            None,
            None,
            None,
        )
    );
}

/// A Run pins the Memory revision only when its captured settings permit use, and the revision 0
/// has no entry.
#[test]
fn a_run_pins_the_memory_revision_only_when_use_is_enabled() {
    assert_eq!(
        [(true, 4), (false, 4), (true, 0), (false, 0)]
            .map(|(use_enabled, revision)| { memory_revision_pin(use_enabled, revision) }),
        [Some(4), None, None, None]
    );
}
