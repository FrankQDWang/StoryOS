use super::{
    MemoryEntryOrigin, MemoryEntryState, MemoryEntryType, MemorySourceReference,
    MemorySourceReferenceRead, memory_revision_pin, read_memory_source_references,
};

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

/// Memory contract section 10: a reference that names a record that the Project does not have
/// now, or that does not decode, is unavailable, and the read does not fail.
#[test]
fn a_reference_to_a_missing_record_reads_as_unavailable() {
    let references = serde_json::json!([
        {"kind": "conversation", "conversation_id": "conversation-a", "run_id": "run-a",
         "tool_call_id": "call-a"},
        {"kind": "chapter", "chapter_id": "chapter-a"},
        {"kind": "chapter", "chapter_id": "chapter-removed"},
        {"kind": "conversation", "conversation_id": "conversation-b"},
        {"kind": "fiction_assertion", "assertion_id": "assertion-a"},
    ]);
    let present = |reference: &MemorySourceReference| match reference {
        MemorySourceReference::Chapter { chapter_id } => chapter_id == "chapter-a",
        MemorySourceReference::Conversation { .. } => true,
    };

    assert_eq!(
        (
            read_memory_source_references(&references, present),
            read_memory_source_references(&serde_json::json!({"kind": "chapter"}), present),
        ),
        (
            vec![
                MemorySourceReferenceRead::Available(MemorySourceReference::Conversation {
                    conversation_id: "conversation-a".to_owned(),
                    run_id: Some("run-a".to_owned()),
                    tool_call_id: Some("call-a".to_owned()),
                }),
                MemorySourceReferenceRead::Available(MemorySourceReference::Chapter {
                    chapter_id: "chapter-a".to_owned(),
                }),
                MemorySourceReferenceRead::Unavailable,
                MemorySourceReferenceRead::Available(MemorySourceReference::Conversation {
                    conversation_id: "conversation-b".to_owned(),
                    run_id: None,
                    tool_call_id: None,
                }),
                MemorySourceReferenceRead::Unavailable,
            ],
            Vec::new(),
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
