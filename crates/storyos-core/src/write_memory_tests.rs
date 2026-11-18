use serde_json::{Value, json};

use super::{
    CurrentMemoryEntry, MEMORY_CHARACTER_TEXT_MAX_CHARS, MEMORY_DESCRIPTION_MAX_CHARS,
    MEMORY_TEXT_MAX_CHARS, MEMORY_TITLE_MAX_CHARS, MemoryVersionWrite, MemoryWrite,
    MemoryWriteRefusal, WRITE_MEMORY_TOOL_SPEC, decide_memory_write,
};
use crate::{IntakeRefusal, MemoryEntryType, ToolIntake};

fn entry(number: u64, entry_type: MemoryEntryType, title: &str) -> CurrentMemoryEntry {
    CurrentMemoryEntry {
        memory_entry_id: format!("entry-{number}"),
        number,
        version: 2,
        entry_type,
        title: title.to_owned(),
        description: "一句话。".to_owned(),
        text: "正文。".to_owned(),
        flagged: false,
    }
}

/// The intake of the JSON arguments `value`, with null for each argument that it does not give.
fn write(value: Value) -> MemoryWrite {
    let mut arguments = json!({
        "action": "create", "entry": null, "type": null, "title": null, "description": null,
        "text": null,
    });
    arguments
        .as_object_mut()
        .unwrap()
        .extend(value.as_object().unwrap().clone());
    match WRITE_MEMORY_TOOL_SPEC.intake(&arguments) {
        Ok(ToolIntake::WriteMemory(write)) => write,
        other => panic!("the arguments must fit the intake: {other:?}"),
    }
}

fn create(entry_type: &str, title: &str, text: &str) -> MemoryWrite {
    write(json!({"type": entry_type, "title": title, "description": "一句话。", "text": text}))
}

fn decide(write: &MemoryWrite) -> Result<MemoryVersionWrite, MemoryWriteRefusal> {
    let entries = [
        entry(1, MemoryEntryType::Character, "林舟"),
        entry(2, MemoryEntryType::Foreshadowing, "铜铃"),
    ];
    decide_memory_write(write, &entries, 7)
}

/// ADR 0094 section 5: `create` writes version 1 with the next number, `change` the next version
/// with the given values and the kept values, also when each value is equal, and `delete` the
/// next version with the state `deleted`, the kept title and description, and no text.
#[test]
fn each_action_gives_its_version() {
    let change = write(json!({"action": "change", "entry": 1, "text": "沉默的船夫。"}));
    let same = write(json!({"action": "change", "entry": 2, "title": " 铜铃 "}));
    let delete = write(json!({"action": "delete", "entry": 2}));
    let version = |result: Result<MemoryVersionWrite, MemoryWriteRefusal>| {
        let version = result.unwrap();
        let id = version.memory_entry_id.unwrap_or_else(|| "new".to_owned());
        let (state, kind) = (version.state.as_str(), version.entry_type.as_str());
        let (number, version_number) = (version.number, version.version);
        format!(
            "{id} {number} {version_number} {state} {kind} {}|{}",
            version.title, version.text
        )
    };

    assert_eq!(
        [
            version(decide(&create("relationship", " 林舟与阿青 ", "兄妹。"))),
            version(decide(&change)),
            version(decide(&same)),
            version(decide(&delete)),
        ],
        [
            "new 7 1 current relationship 林舟与阿青|兄妹。",
            "entry-1 1 3 current character 林舟|沉默的船夫。",
            "entry-2 2 3 current foreshadowing 铜铃|正文。",
            "entry-2 2 3 deleted foreshadowing 铜铃|",
        ]
    );
}

/// The refusals of S7-D1a2 in their order: the first check that fails decides the refusal.
#[test]
fn each_refusal_of_the_table_has_its_cause_and_its_label() {
    let title = "长".repeat(MEMORY_TITLE_MAX_CHARS + 1);
    let text = |limit: usize| "字".repeat(limit + 1);
    // A `create` of the type `other` with the title `t`, the description `d`, and `value`.
    let other = |value: Value| {
        let mut base = json!({"type": "other", "title": "t", "description": "d"});
        base.as_object_mut()
            .unwrap()
            .extend(value.as_object().unwrap().clone());
        write(base)
    };
    let cases = [
        (
            "memory_entry_invalid",
            vec![
                other(json!({})),
                write(json!({"action": "change", "entry": 1})),
                write(json!({"action": "delete", "entry": 1, "title": "林舟"})),
                other(json!({"entry": 3, "text": "x"})),
                create("other", "两\n行", "x"),
                create("other", "  ", "x"),
                create("other", "标题", " \n "),
                create("other", "标题", "铃\u{7}声"),
                create("other", "标题", "铃\r声"),
                other(json!({"title": title, "description": "\n", "text": "x"})),
            ],
        ),
        (
            "memory_entry_too_long",
            vec![
                create("other", &title, "x"),
                other(json!({"description": "长".repeat(101), "text": "x"})),
                create("other", "标题", &text(MEMORY_TEXT_MAX_CHARS)),
                create("character", "阿青", &text(MEMORY_CHARACTER_TEXT_MAX_CHARS)),
                write(json!({"action": "change", "entry": 9, "title": title})),
            ],
        ),
        (
            "memory_entry_not_found",
            vec![
                write(json!({"action": "change", "entry": 9, "title": "新"})),
                write(json!({"action": "delete", "entry": -1})),
            ],
        ),
        (
            "memory_entry_exists",
            vec![
                create("character", " 林舟 ", "x"),
                write(
                    json!({"action": "change", "entry": 2, "type": "character", "title": "林舟"}),
                ),
            ],
        ),
    ];
    let (decided, expected): (Vec<_>, Vec<_>) = (cases.iter())
        .flat_map(|(label, writes)| writes.iter().map(move |case| (case, *label)))
        .map(|(case, label)| {
            (
                decide(case).map(|_| ()).map_err(MemoryWriteRefusal::label),
                Err(label),
            )
        })
        .unzip();

    assert_eq!(decided, expected);
}

/// A text at each limit is admitted, and the title and the description at their limits too. The
/// type `character` has the longer text limit, and a change of the type checks the kept text. A
/// text is Markdown, so it can have a line feed and a tab.
#[test]
fn a_value_at_its_limit_is_admitted() {
    let title = "长".repeat(MEMORY_TITLE_MAX_CHARS);
    let description = "长".repeat(MEMORY_DESCRIPTION_MAX_CHARS);
    let at_limit = write(
        json!({"type": "other", "title": title, "description": description, "text": "字".repeat(MEMORY_TEXT_MAX_CHARS)}),
    );
    let character = create(
        "character",
        "阿青",
        &"字".repeat(MEMORY_CHARACTER_TEXT_MAX_CHARS),
    );
    let entries = [CurrentMemoryEntry {
        text: "字".repeat(MEMORY_TEXT_MAX_CHARS + 1),
        ..entry(1, MemoryEntryType::Character, "林舟")
    }];
    let retyped = write(json!({"action": "change", "entry": 1, "type": "other"}));

    assert_eq!(
        (
            decide(&at_limit).is_ok(),
            decide(&character).is_ok(),
            decide(&create("other", "标题", "第一行\n\t第二行")).is_ok(),
            decide_memory_write(&retyped, &entries, 2),
        ),
        (true, true, true, Err(MemoryWriteRefusal::TooLong))
    );
}

/// The Host limits of the Memory Index: a create after 300 current entries is refused and removes
/// no entry, and a change can keep the 300 entries. The result says that Memory is full.
#[test]
fn a_write_above_the_index_limits_is_refused_with_memory_index_full() {
    let entries: Vec<_> = (1..=300)
        .map(|number| {
            entry(
                number,
                MemoryEntryType::MajorEvent,
                &format!("事件{number}"),
            )
        })
        .collect();
    let change = write(json!({"action": "change", "entry": 300, "description": "新的一句话。"}));
    let long: Vec<_> = (1..=2)
        .map(|number| CurrentMemoryEntry {
            description: "长".repeat(MEMORY_DESCRIPTION_MAX_CHARS),
            ..entry(number, MemoryEntryType::Other, &format!("{number}"))
        })
        .collect();
    let refusal = MemoryWriteRefusal::IndexFull.result().text;

    assert_eq!(
        (
            decide_memory_write(&create("other", "第301件", "x"), &entries, 301),
            decide_memory_write(&change, &entries, 301).map(|version| version.version),
            decide_memory_write(&create("other", "3", "x"), &long, 3).map(|version| version.number),
            refusal.contains("Memory is full"),
        ),
        (Err(MemoryWriteRefusal::IndexFull), Ok(3), Ok(3), true)
    );
}

/// The Index bound counts the conflict flag of each other entry (#1696): a set of entries that
/// fits with no flag and does not fit with its flags refuses the write with `memory_index_full`.
#[test]
fn the_index_bound_of_a_write_counts_the_conflict_flags() {
    let others = |count: u64, flagged: bool| -> Vec<CurrentMemoryEntry> {
        (1..=count)
            .map(|number| CurrentMemoryEntry {
                description: "长".repeat(MEMORY_DESCRIPTION_MAX_CHARS),
                flagged,
                ..entry(number, MemoryEntryType::Other, &format!("条目{number}"))
            })
            .collect()
    };
    let new = create("other", "新条目", "x");
    let decide =
        |count: u64, flagged: bool| decide_memory_write(&new, &others(count, flagged), count + 1);
    // The largest count of other entries that lets the create fit with no flag.
    let fits = (1..300)
        .take_while(|&count| decide(count, false).is_ok())
        .last()
        .expect("one entry fits");

    assert_eq!(
        (decide(fits, false).is_ok(), decide(fits, true)),
        (true, Err(MemoryWriteRefusal::IndexFull))
    );
}

/// The schema is strict: each key is necessary, also for a null value, and an unknown type does
/// not fit. A value with U+0000 does not fit, because the ToolCall record cannot hold it. The canonical arguments keep each null.
#[test]
fn the_intake_is_strict_and_the_arguments_keep_each_null() {
    let refused = |value: Value| WRITE_MEMORY_TOOL_SPEC.intake(&value).err();
    let full = |extra: Value| {
        let mut base = json!({"action": "create", "entry": null, "type": "other", "title": "t", "description": "d", "text": "x"});
        base.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        base
    };
    let parameters = super::parameters();

    assert_eq!(
        (
            refused(full(json!({"type": "fiction_assertion"}))),
            refused(full(json!({"text": "a\u{0}b"}))),
            refused(full(json!({"extra": 1}))),
            refused(full(json!({"action": "merge"}))),
            refused(json!({"action": "delete", "entry": 2})),
            write(json!({"action": "delete", "entry": 2})).arguments(),
            parameters["required"].as_array().map(Vec::len),
            parameters["additionalProperties"].clone(),
            MemoryWriteRefusal::Exists(4).result().text,
        ),
        (
            Some(IntakeRefusal::Malformed),
            Some(IntakeRefusal::UnstorableText),
            Some(IntakeRefusal::Malformed),
            Some(IntakeRefusal::Malformed),
            Some(IntakeRefusal::Malformed),
            json!({"action": "delete", "entry": 2, "type": null, "title": null, "description": null, "text": null}),
            Some(6),
            json!(false),
            "The Host refused this call because the entry 4 already has this type and this \
             title: change that entry, so Memory did not change."
                .to_owned(),
        )
    );
}
