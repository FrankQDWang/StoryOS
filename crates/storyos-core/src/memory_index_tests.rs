use super::{
    MEMORY_INDEX_MAX_ENTRIES, MEMORY_INDEX_TOKEN_LIMIT, MemoryIndexEntry, assemble_memory_index,
    memory_index_text,
};
use crate::assemble_context::count_context_item_tokens;
use crate::shared_tests::passage_assembly;
use crate::{
    ContextSourceClass, MemoryEntryType, ProjectionMode, assemble_current_passage_context,
    decode_assembly_record, encode_assembly_record,
};

fn entry(
    number: u64,
    entry_type: MemoryEntryType,
    title: &str,
    description: &str,
) -> MemoryIndexEntry {
    MemoryIndexEntry {
        number,
        entry_type,
        title: title.to_owned(),
        description: description.to_owned(),
        flagged: false,
        author_part: false,
        intent: None,
    }
}

fn record() -> crate::CurrentPassageAssemblyRecord {
    assemble_current_passage_context(&passage_assembly("Write the quarrel.", "Rain."))
}

/// ADR 0094 section 4: one group for each type that has an entry, in the order of the type
/// list, and the entries of a group in the order of their numbers. The text has no entry text.
#[test]
fn the_index_has_one_group_for_each_type_in_the_order_of_the_type_list() {
    let entries = [
        entry(7, MemoryEntryType::Other, "结局", "林舟最后离开渡口。"),
        entry(3, MemoryEntryType::Character, "林舟", "沉默的船夫。"),
        entry(
            1,
            MemoryEntryType::WritingPreference,
            "短句",
            "作者喜欢短句。",
        ),
        entry(2, MemoryEntryType::Character, "阿青", "林舟的妹妹。"),
    ];

    assert_eq!(
        memory_index_text(&entries),
        (
            [
                r#"{"type":"writing_preference"}"#,
                r#"{"description":"作者喜欢短句。","id":1,"title":"短句"}"#,
                r#"{"type":"character"}"#,
                r#"{"description":"林舟的妹妹。","id":2,"title":"阿青"}"#,
                r#"{"description":"沉默的船夫。","id":3,"title":"林舟"}"#,
                r#"{"type":"other"}"#,
                r#"{"description":"林舟最后离开渡口。","id":7,"title":"结局"}"#,
            ]
            .join("\n"),
            ProjectionMode::ExactRequired,
        )
    );
}

/// ADR 0094 section 7: the line of an entry with a conflict flag has one more field and no
/// reason. The line of an entry with no flag is the line of S7-D1a, so an Index with no flag has
/// the same text as before the flag.
#[test]
fn the_line_of_a_flagged_entry_has_one_more_field() {
    let mut entries = [
        entry(1, MemoryEntryType::Character, "阿青", "林舟的妹妹。"),
        entry(2, MemoryEntryType::Character, "林舟", "沉默的船夫。"),
    ];
    let (unflagged, _) = memory_index_text(&entries);
    entries[0].flagged = true;

    assert_eq!(
        (unflagged, memory_index_text(&entries)),
        (
            [
                r#"{"type":"character"}"#,
                r#"{"description":"林舟的妹妹。","id":1,"title":"阿青"}"#,
                r#"{"description":"沉默的船夫。","id":2,"title":"林舟"}"#,
            ]
            .join("\n"),
            (
                [
                    r#"{"type":"character"}"#,
                    r#"{"conflict_flag":true,"description":"林舟的妹妹。","id":1,"title":"阿青"}"#,
                    r#"{"description":"沉默的船夫。","id":2,"title":"林舟"}"#,
                ]
                .join("\n"),
                ProjectionMode::ExactRequired,
            )
        )
    );
}

/// The Host limit of 300 entries: 300 entries are complete, and 301 list the first 300 in
/// order and end with the count of the entries that the Index does not list.
#[test]
fn the_index_lists_at_most_300_entries() {
    let entries: Vec<_> = (1..=MEMORY_INDEX_MAX_ENTRIES as u64 + 1)
        .map(|number| entry(number, MemoryEntryType::MajorEvent, "事件", "一件事。"))
        .collect();
    let (complete, complete_mode) = memory_index_text(&entries[..MEMORY_INDEX_MAX_ENTRIES]);
    let (excerpt, excerpt_mode) = memory_index_text(&entries);
    let lines: Vec<&str> = excerpt.lines().collect();

    assert_eq!(
        (
            complete_mode,
            complete.lines().count(),
            excerpt_mode,
            lines.len(),
            lines[MEMORY_INDEX_MAX_ENTRIES],
            lines[MEMORY_INDEX_MAX_ENTRIES + 1],
        ),
        (
            ProjectionMode::ExactRequired,
            MEMORY_INDEX_MAX_ENTRIES + 1,
            ProjectionMode::DeterministicExcerpt,
            MEMORY_INDEX_MAX_ENTRIES + 2,
            r#"{"description":"一件事。","id":300,"title":"事件"}"#,
            r#"{"not_listed":1}"#,
        )
    );
}

/// The Host limit of 32,000 counted tokens: a text at the limit is complete, and one more token
/// lists the entries in order up to the limit with the count line. The limit counts the intents:
/// the same entries with no intent are complete.
#[test]
fn the_index_text_has_at_most_32000_counted_tokens() {
    let mut entries: Vec<_> = (10..110)
        .map(|number| entry(number, MemoryEntryType::Character, "人物", ""))
        .collect();
    // Each character of an intent is one counted token, so the intent of the last entry fills
    // the text up to the limit.
    entries[99].intent = Some(String::new());
    let empty = count_context_item_tokens(&memory_index_text(&entries).0);
    entries[99].intent = Some("雨".repeat((MEMORY_INDEX_TOKEN_LIMIT - empty) as usize));
    let (at_limit, at_limit_mode) = memory_index_text(&entries);
    entries[99].intent = entries[99].intent.take().map(|intent| intent + "雨");
    let (above, above_mode) = memory_index_text(&entries);
    entries[99].intent = None;
    let (_, no_intent_mode) = memory_index_text(&entries);

    assert_eq!(
        (
            no_intent_mode,
            count_context_item_tokens(&at_limit),
            at_limit_mode,
            above_mode,
            above.lines().count(),
            above.lines().last(),
            count_context_item_tokens(&above) <= MEMORY_INDEX_TOKEN_LIMIT,
        ),
        (
            ProjectionMode::ExactRequired,
            MEMORY_INDEX_TOKEN_LIMIT,
            ProjectionMode::ExactRequired,
            ProjectionMode::DeterministicExcerpt,
            101,
            Some(r#"{"not_listed":1}"#),
            true,
        )
    );
}

/// The pin goes into the Operation Requirement, and an item comes only with a pin and a current
/// entry. The record keeps the pin through its codec, so each later RunStep can copy it.
#[test]
fn the_record_keeps_the_pin_and_adds_an_item_only_for_a_pin_with_entries() {
    let entries = [entry(
        1,
        MemoryEntryType::Foreshadowing,
        "铜铃",
        "铃声预示船沉。",
    )];
    let outcome = |pin: Option<u64>, entries: &[MemoryIndexEntry]| {
        let mut record = record();
        let before = record.selected.len();
        assemble_memory_index(&mut record, pin, entries);
        let decoded =
            decode_assembly_record(&encode_assembly_record(&record)).expect("record decodes");
        (
            decoded.operation_requirement.memory_revision,
            record.selected[before..]
                .iter()
                .map(|item| {
                    (
                        item.source_class,
                        item.projection_mode,
                        item.content.clone(),
                    )
                })
                .collect::<Vec<_>>(),
            record
                .considered
                .iter()
                .filter(|source| source.source_class == ContextSourceClass::MemoryIndex)
                .count(),
        )
    };

    assert_eq!(
        [outcome(Some(5), &entries), outcome(Some(5), &[]), outcome(None, &entries)],
        [
            (
                Some(5),
                vec![(
                    ContextSourceClass::MemoryIndex,
                    ProjectionMode::ExactRequired,
                    "{\"type\":\"foreshadowing\"}\n{\"description\":\"铃声预示船沉。\",\"id\":1,\"title\":\"铜铃\"}"
                        .to_owned(),
                )],
                1,
            ),
            (Some(5), Vec::new(), 0),
            (None, Vec::new(), 0),
        ]
    );
}
