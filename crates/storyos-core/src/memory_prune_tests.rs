use serde_json::{Value, json};

use super::{memory_entry_unused, utc_milliseconds};
use crate::{
    ContextSourceClass, CurrentMemoryEntry, MemoryConsolidationAssembly, MemoryDeleteMark,
    MemoryEntryOrigin, MemoryEntryType, PassMemoryEntry, assemble_memory_consolidation_context,
};

const NOW: &str = "2026-11-17T12:00:00.000Z";
/// Exactly 90 days before `NOW`, over the end of a month.
const LIMIT: &str = "2026-08-19T12:00:00.000Z";
/// One millisecond after `LIMIT`.
const INSIDE: &str = "2026-08-19T12:00:00.001Z";

/// The store form gives the milliseconds since 1970-01-01, also in a leap year. Another text
/// gives none.
#[test]
fn the_store_time_form_gives_milliseconds() {
    assert_eq!(
        [
            "1970-01-01T00:00:00.000Z",
            "2024-03-01T00:00:00.001Z",
            "2026-11-17T12:00:00.000Z",
            "2026-11-17 12:00:00.000Z",
            "2026-13-17T12:00:00.000Z",
            "2026-11-17T12:00:00Z",
        ]
        .map(utc_milliseconds),
        [
            Some(0),
            Some(1_709_251_200_001),
            Some(1_794_916_800_000),
            None,
            None,
            None
        ]
    );
}

/// A background entry gets the mark at the limit of 90 days for its change and its read, and
/// not one millisecond inside it. An entry with no read needs only the old change. An entry of
/// the author never gets the mark, and a time in another form gives no mark.
#[test]
fn the_mark_unused_starts_at_the_limit() {
    let chapter = MemoryEntryOrigin::Chapter;
    assert_eq!(
        [
            memory_entry_unused(chapter, Some(LIMIT), LIMIT, NOW),
            memory_entry_unused(chapter, None, LIMIT, NOW),
            memory_entry_unused(MemoryEntryOrigin::Conversation, None, LIMIT, NOW),
            memory_entry_unused(chapter, Some(INSIDE), LIMIT, NOW),
            memory_entry_unused(chapter, Some(LIMIT), INSIDE, NOW),
            memory_entry_unused(chapter, None, INSIDE, NOW),
            memory_entry_unused(MemoryEntryOrigin::AuthorRequest, None, LIMIT, NOW),
            memory_entry_unused(chapter, None, "2026-08-19", NOW),
        ],
        [true, true, true, false, false, false, false, false]
    );
}

/// The input of a pass gives the key `unused` only on the line of a marked entry, and it ends
/// with the `memory_delete_marks` item. A pass with no mark has no such item.
#[test]
fn the_input_of_a_pass_has_the_mark_unused_and_the_delete_marks() {
    let entry = |number: u64| {
        let entry = PassMemoryEntry {
            entry: CurrentMemoryEntry {
                memory_entry_id: format!("entry-{number}"),
                number,
                version: 1,
                entry_type: MemoryEntryType::Other,
                title: format!("条{number}"),
                description: "述。".to_owned(),
                text: "文。".to_owned(),
            },
            origin: MemoryEntryOrigin::Chapter,
        };
        (entry, LIMIT.to_owned())
    };
    let entries = [entry(1), entry(2)];
    let marks = [MemoryDeleteMark {
        entry_type: MemoryEntryType::Other,
        title: "旧灯".to_owned(),
        description: "述。".to_owned(),
    }];
    let assemble = |marks: &[MemoryDeleteMark]| {
        let record = assemble_memory_consolidation_context(&MemoryConsolidationAssembly {
            operation_requirement_id: "requirement".to_owned(),
            run_id: "run".to_owned(),
            owner_user_id: "owner".to_owned(),
            project_id: "project".to_owned(),
            destination_identity: "destination".to_owned(),
            memory_revision: 1,
            entries: &entries,
            unused: &[2],
            observations: &[],
            marks,
        });
        let items: Vec<(ContextSourceClass, Vec<Value>)> = record
            .selected
            .iter()
            .map(|item| {
                let lines = item.content.lines().map(serde_json::from_str);
                (item.source_class, lines.collect::<Result<_, _>>().unwrap())
            })
            .collect();
        items
    };
    let line = |number: u64| {
        json!({
            "id": number, "type": "other", "title": format!("条{number}"), "description": "述。",
            "text": "文。", "origin": "chapter", "changed_at": LIMIT
        })
    };
    let mut unused = line(2);
    unused["unused"] = json!(true);

    assert_eq!(
        (assemble(&marks), assemble(&[]).len()),
        (
            vec![
                (ContextSourceClass::MemoryEntries, vec![line(1), unused]),
                (
                    ContextSourceClass::MemoryDeleteMarks,
                    vec![json!({ "type": "other", "title": "旧灯", "description": "述。" })]
                ),
            ],
            1
        )
    );
}
