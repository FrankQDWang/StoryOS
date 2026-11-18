use serde_json::{Value, json};

use super::{MemoryOperationCandidate, PassFacts, PassMemoryEntry, intake, settle_operations_call};
use crate::{
    ConsolidationOutcome, CurrentMemoryEntry, IntakeRefusal, MEMORY_DESCRIPTION_MAX_CHARS,
    MemoryConflictFlag, MemoryDeleteMark, MemoryEntryOrigin, MemoryEntryState, MemoryEntryType,
    MemorySourceReference, MemoryWriteAction, PassConflict, PassObservation,
};

fn entry(number: u64, origin: MemoryEntryOrigin, title: &str) -> PassMemoryEntry {
    PassMemoryEntry {
        entry: CurrentMemoryEntry {
            memory_entry_id: format!("entry-{number}"),
            number,
            version: 1,
            entry_type: MemoryEntryType::Other,
            title: title.to_owned(),
            description: "述。".to_owned(),
            text: "文。".to_owned(),
            flagged: false,
        },
        origin,
    }
}

fn operations(items: Value) -> Vec<MemoryOperationCandidate> {
    intake(&json!({ "operations": items })).expect("the intake admits the operations")
}

/// An observation of the input from `source`: a Chapter, or else a Conversation.
fn observation(source: &str) -> PassObservation {
    PassObservation {
        entry_type: MemoryEntryType::Other,
        title: "观察".to_owned(),
        text: "文。".to_owned(),
        source: match source {
            "chapter" => MemorySourceReference::Chapter {
                chapter_id: "chapter".to_owned(),
            },
            _ => MemorySourceReference::Conversation {
                conversation_id: source.to_owned(),
                run_id: None,
                tool_call_id: None,
            },
        },
        refused: None,
    }
}

fn create(entry_type: &str, title: &str, text: &str, observations: Value) -> Value {
    json!({
        "action": "create", "entry": null, "entries": null, "type": entry_type, "title": title,
        "description": "述。", "text": text, "observations": observations, "reason": null
    })
}

fn update(entry: u64, title: &str) -> Value {
    json!({
        "action": "update", "entry": entry, "entries": null, "type": null, "title": title,
        "description": "新述。", "text": "新文。", "observations": [1], "reason": null
    })
}

fn merge(entries: Value, title: &str) -> Value {
    json!({
        "action": "merge", "entry": null, "entries": entries, "type": null, "title": title,
        "description": "合述。", "text": "合文。", "observations": [1], "reason": null
    })
}

fn delete(entry: u64) -> Value {
    json!({
        "action": "delete", "entry": entry, "entries": null, "type": null, "title": null,
        "description": null, "text": null, "observations": [], "reason": "不再成立。"
    })
}

fn flag(entry: u64, reason: &str, observations: Value) -> Value {
    json!({
        "action": "flag_conflict", "entry": entry, "entries": null, "type": null, "title": null,
        "description": null, "text": null, "observations": observations, "reason": reason
    })
}

/// The decision of each operation: the action, the number, the origin, and the count of the
/// sources of an admitted operation, and the numbers that a merge deleted, or the label of a
/// refusal.
fn decided(candidates: &[MemoryOperationCandidate], facts: &PassFacts<'_>) -> Vec<String> {
    settle_operations_call(candidates, facts)
        .decisions
        .iter()
        .map(|decision| match decision {
            Ok(admitted) => {
                let merged: Vec<String> = admitted
                    .merged
                    .iter()
                    .map(|version| format!(" -{}", version.number))
                    .collect();
                format!(
                    "{:?} {} {} {}{}",
                    admitted.version.action,
                    admitted.version.number,
                    admitted.origin.as_str(),
                    admitted.sources.len(),
                    merged.concat()
                )
            }
            Err(refusal) => refusal.label().to_owned(),
        })
        .collect()
}

/// The intake needs each key of each operation and no other key, and it refuses an unknown
/// action and U+0000. It admits an empty list and a null value. The schema has no enum of the
/// types, as the intake admits each type text and the validation refuses an unknown type.
#[test]
fn the_intake_needs_each_key_and_refuses_an_unstorable_text() {
    let item = &super::parameters()["properties"]["operations"]["items"]["properties"];
    assert_eq!(
        (item["type"].get("enum"), &item["action"]["enum"]),
        (
            None,
            &json!(["create", "update", "merge", "delete", "flag_conflict"])
        )
    );
    let mut partial = delete(1);
    partial.as_object_mut().unwrap().remove("reason");
    let mut extra = delete(1);
    extra["state"] = json!("deleted");
    let mut action = delete(1);
    action["action"] = json!("rename");
    let unstorable = create("other", "灯\0", "文。", json!([1]));
    assert_eq!(
        [
            json!([partial]),
            json!([extra]),
            json!([action]),
            json!([unstorable]),
            json!({}),
            json!([]),
            json!([delete(1)]),
        ]
        .map(|items| intake(&json!({ "operations": items })).map(|items| items.len())),
        [
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::UnstorableText),
            Err(IntakeRefusal::Malformed),
            Ok(0),
            Ok(1),
        ]
    );
}

/// Each refusal of the ticket, in list order. Each admitted operation counts for the operations
/// after it, and a refused operation does not stop them. The four last refusals are the write rule
/// of S7-D1a2.
#[test]
fn each_operation_is_decided_in_list_order_with_each_refusal() {
    let mut entries: Vec<PassMemoryEntry> = (2..=299)
        .map(|number| {
            entry(
                number,
                MemoryEntryOrigin::Conversation,
                &format!("条{number}"),
            )
        })
        .collect();
    entries.push(entry(1, MemoryEntryOrigin::AuthorRequest, "阿青"));
    let observations = [observation("chapter"), observation("conversation")];
    let facts = PassFacts {
        pinned_revision: 7,
        memory_revision: 7,
        entries: &entries,
        next_entry_number: 300,
        observations: &observations,
        marks: &[],
        conflicts: &[],
    };
    let candidates = operations(json!([
        create("foreshadowing", "灯", "灯每夜亮一次。", json!([1, 2, 1])),
        update(1, "阿青"),
        update(999, "无"),
        delete(300),
        create("other", "桥", "文。", json!([3])),
        create("other", "桥", "文。", json!([])),
        create("unknown", "桥", "文。", json!([2])),
        create("other", "桥", &"长".repeat(601), json!([2])),
        create("foreshadowing", "灯", "文。", json!([2])),
        create("other", "桥", "文。", json!([2])),
        update(2, "条二"),
    ]));

    assert_eq!(
        decided(&candidates, &facts),
        [
            "Create 300 chapter 2",
            "author_entry",
            "memory_entry_not_found",
            "entry_named_twice",
            "observation_unavailable",
            "create_without_source",
            "memory_entry_invalid",
            "memory_entry_too_long",
            "memory_entry_exists",
            "memory_index_full",
            "Change 2 conversation 1",
        ]
    );
}

/// A delete needs a reason, an update needs one or more of its three values and no type, and a
/// delete of an entry keeps its origin. An operation names its entry also when the Host refuses it, and an
/// observation number below 1 is not in the input. An empty list applies nothing.
#[test]
fn the_shape_of_each_action_and_an_empty_list() {
    let entries = [4, 5, 6, 7]
        .map(|number| entry(number, MemoryEntryOrigin::Chapter, &format!("灯{number}")));
    let facts = PassFacts {
        pinned_revision: 1,
        memory_revision: 1,
        entries: &entries,
        next_entry_number: 8,
        observations: &[observation("conversation")],
        marks: &[],
        conflicts: &[],
    };
    let mut no_reason = delete(4);
    no_reason["reason"] = json!(" ");
    let mut typed = update(5, "灯");
    typed["type"] = json!("other");
    let mut blank = update(6, "灯");
    for key in ["title", "description", "text"] {
        blank[key] = Value::Null;
    }
    let candidates = operations(json!([
        no_reason,
        typed,
        blank,
        delete(7),
        update(4, "灯"),
        create("other", "桥", "文。", json!([-1])),
    ]));
    let empty = settle_operations_call(&[], &facts);

    assert_eq!(
        (
            decided(&candidates, &facts),
            empty.outcome,
            empty.decisions.len()
        ),
        (
            vec![
                "memory_entry_invalid".to_owned(),
                "memory_entry_invalid".to_owned(),
                "memory_entry_invalid".to_owned(),
                format!("{:?} 7 chapter 0", MemoryWriteAction::Delete),
                "entry_named_twice".to_owned(),
                "observation_unavailable".to_owned(),
            ],
            ConsolidationOutcome::NoOperation,
            0
        )
    );
}

/// The reason obeys the declared schema (#1696): a create or an update with a reason is
/// `memory_entry_invalid`, and a delete reason above the limit of a description is
/// `memory_entry_too_long`. A delete reason at the limit is admitted.
#[test]
fn the_reason_of_an_operation_obeys_its_declared_form() {
    let entries =
        [4, 5, 6].map(|number| entry(number, MemoryEntryOrigin::Chapter, &format!("灯{number}")));
    let facts = PassFacts {
        pinned_revision: 1,
        memory_revision: 1,
        entries: &entries,
        next_entry_number: 7,
        observations: &[observation("conversation")],
        marks: &[],
        conflicts: &[],
    };
    let mut created = create("other", "桥", "文。", json!([1]));
    created["reason"] = json!("理由。");
    let mut updated = update(4, "灯");
    updated["reason"] = json!("理由。");
    let mut long = delete(5);
    long["reason"] = json!("长".repeat(MEMORY_DESCRIPTION_MAX_CHARS + 1));
    let mut at_limit = delete(6);
    at_limit["reason"] = json!("长".repeat(MEMORY_DESCRIPTION_MAX_CHARS));
    let candidates = operations(json!([created, updated, long, at_limit]));

    assert_eq!(
        decided(&candidates, &facts),
        vec![
            "memory_entry_invalid".to_owned(),
            "memory_entry_invalid".to_owned(),
            "memory_entry_too_long".to_owned(),
            format!("{:?} 6 chapter 0", MemoryWriteAction::Delete),
        ]
    );
}

/// A flag of the pass counts for the Index bound of a later write of the same pass (#1696): a
/// create that fits after the entries of the input does not fit after a flag of an entry.
#[test]
fn a_flag_of_the_pass_counts_for_the_index_bound_of_a_later_create() {
    let observations = [observation("conversation")];
    let entries = |count: u64, tail: usize| -> Vec<PassMemoryEntry> {
        let long = |number: u64, length: usize| PassMemoryEntry {
            entry: CurrentMemoryEntry {
                description: "长".repeat(length),
                ..entry(number, MemoryEntryOrigin::Chapter, &format!("条目{number}")).entry
            },
            origin: MemoryEntryOrigin::Chapter,
        };
        let mut entries = vec![entry(1, MemoryEntryOrigin::AuthorRequest, "条1")];
        entries.extend((2..=count).map(|number| long(number, MEMORY_DESCRIPTION_MAX_CHARS)));
        entries.push(long(count + 1, tail));
        entries
    };
    let create = || create("other", "新条目", "文。", json!([1]));
    let fits = |count: u64, tail: usize| {
        let entries = entries(count, tail);
        let facts = pass(&entries, &observations, &[]);
        decided(&operations(json!([create()])), &facts)[0] != "memory_index_full"
    };
    // The most entries with a full description, and then the longest last description, that let
    // the create fit.
    let count = (2..299)
        .take_while(|&count| fits(count, MEMORY_DESCRIPTION_MAX_CHARS))
        .last()
        .expect("the create fits after few entries");
    let tail = (1..=MEMORY_DESCRIPTION_MAX_CHARS)
        .take_while(|&tail| fits(count + 1, tail))
        .last()
        .expect("the create fits with a short last description");
    let entries = entries(count + 1, tail);
    let candidates = operations(json!([flag(1, "不符。", json!([1])), create()]));

    assert_eq!(
        decided(&candidates, &pass(&entries, &observations, &[])),
        ["Change 1 author_request 0", "memory_index_full"]
    );
}

/// A null title, description, or text of an update keeps the current value of the entry, and a
/// null title of a merge keeps the title of its first entry. A merge with no text is refused
/// (#1685).
#[test]
fn a_null_value_of_an_update_or_a_merge_keeps_the_current_value() {
    let entries = [4, 5, 6, 7, 8, 9, 10]
        .map(|number| entry(number, MemoryEntryOrigin::Chapter, &format!("灯{number}")));
    let facts = PassFacts {
        pinned_revision: 1,
        memory_revision: 1,
        entries: &entries,
        next_entry_number: 11,
        observations: &[observation("chapter")],
        marks: &[],
        conflicts: &[],
    };
    let mut untitled = update(4, "灯");
    untitled["title"] = Value::Null;
    let mut text_only = update(5, "灯");
    text_only["title"] = Value::Null;
    text_only["description"] = Value::Null;
    let mut no_new_text = update(10, "新灯");
    no_new_text["text"] = Value::Null;
    let mut merged = merge(json!([6, 7]), "合");
    merged["title"] = Value::Null;
    let mut no_text = merge(json!([8, 9]), "合");
    no_text["title"] = Value::Null;
    no_text["text"] = Value::Null;
    let candidates = operations(json!([untitled, text_only, no_new_text, merged, no_text]));
    let values: Vec<String> = settle_operations_call(&candidates, &facts)
        .decisions
        .iter()
        .map(|decision| match decision {
            Ok(admitted) => format!(
                "{} {} {} {}",
                admitted.version.number,
                admitted.version.title,
                admitted.version.description,
                admitted.version.text
            ),
            Err(refusal) => refusal.label().to_owned(),
        })
        .collect();

    assert_eq!(
        values,
        [
            "4 灯4 新述。 新文。",
            "5 灯5 述。 新文。",
            "10 新灯 新述。 文。",
            "6 灯6 合述。 合文。",
            "memory_entry_invalid",
        ]
    );
}

/// A pass whose pinned Memory revision is not the revision now decides no operation. A list with
/// an admitted operation is applied.
#[test]
fn a_stale_pass_decides_no_operation() {
    let candidates = operations(json!([create("other", "桥", "文。", json!([1]))]));
    let settle = |memory_revision| {
        let settlement = settle_operations_call(
            &candidates,
            &PassFacts {
                pinned_revision: 3,
                memory_revision,
                entries: &[],
                next_entry_number: 1,
                observations: &[observation("conversation")],
                marks: &[],
                conflicts: &[],
            },
        );
        (settlement.outcome, settlement.decisions.len())
    };
    assert_eq!(
        [settle(4), settle(3)],
        [
            (ConsolidationOutcome::Stale, 0),
            (ConsolidationOutcome::Applied, 1)
        ]
    );
}

/// The facts of a pass at the pinned Memory revision 1, with `entries` and `marks`.
fn pass<'a>(
    entries: &'a [PassMemoryEntry],
    observations: &'a [PassObservation],
    marks: &'a [MemoryDeleteMark],
) -> PassFacts<'a> {
    PassFacts {
        pinned_revision: 1,
        memory_revision: 1,
        entries,
        next_entry_number: entries.iter().map(|e| e.entry.number).max().unwrap_or(0) + 1,
        observations,
        marks,
        conflicts: &[],
    }
}

/// A merge gives the result to its first entry and the state `deleted` to each other entry, each
/// in its next version. The result can have the title of a merged entry, and the merge makes room
/// in the full Memory Index for a later create of the same list.
#[test]
fn a_merge_changes_its_first_entry_and_deletes_the_others() {
    let entries: Vec<PassMemoryEntry> = (1..=300)
        .map(|number| entry(number, MemoryEntryOrigin::Chapter, &format!("条{number}")))
        .collect();
    let observations = [observation("conversation")];
    let facts = pass(&entries, &observations, &[]);
    let candidates = operations(json!([
        create("other", "桥", "文。", json!([1])),
        merge(json!([3, 2, 4]), "条2"),
        create("other", "桥", "文。", json!([1])),
    ]));
    let settlement = settle_operations_call(&candidates, &facts);
    let Ok(admitted) = &settlement.decisions[1] else {
        panic!("the merge is admitted");
    };
    let versions: Vec<_> = [&admitted.version]
        .into_iter()
        .chain(&admitted.merged)
        .map(|v| {
            (
                v.number,
                v.version,
                v.state,
                v.title.as_str(),
                v.text.as_str(),
            )
        })
        .collect();

    assert_eq!(
        (decided(&candidates, &facts), versions),
        (
            vec![
                "memory_index_full".to_owned(),
                "Change 3 chapter 1 -2 -4".to_owned(),
                "Create 301 conversation 1".to_owned(),
            ],
            vec![
                (3, 2, MemoryEntryState::Current, "条2", "合文。"),
                (2, 2, MemoryEntryState::Deleted, "条2", ""),
                (4, 2, MemoryEntryState::Deleted, "条4", ""),
            ]
        )
    );
}

/// A merge needs two or more different current entries of one type and no entry of the author.
/// A number that is not the number of an entry refuses the merge.
/// It names each of its entries for the operations after it, also when the Host refuses it.
#[test]
fn each_refusal_of_a_merge() {
    let mut entries: Vec<PassMemoryEntry> = (2..=13)
        .map(|number| {
            entry(
                number,
                MemoryEntryOrigin::Conversation,
                &format!("条{number}"),
            )
        })
        .collect();
    entries.push(entry(1, MemoryEntryOrigin::AuthorRequest, "阿青"));
    entries[0].entry.entry_type = MemoryEntryType::Character;
    let observations = [observation("conversation")];
    let facts = pass(&entries, &observations, &[]);
    let candidates = operations(json!([
        merge(json!([3, 1]), "合"),
        merge(json!([2, 4]), "合"),
        update(5, "条5"),
        merge(json!([5, 6]), "合"),
        merge(json!([7, 8]), "合"),
        delete(8),
        merge(json!([9]), "合"),
        merge(json!([10, 10]), "合"),
        merge(json!([11, 99]), "合"),
        merge(json!([12, -1, 13]), "合"),
    ]));

    assert_eq!(
        decided(&candidates, &facts),
        [
            "author_entry",
            "merge_type_mismatch",
            "Change 5 conversation 1",
            "entry_named_twice",
            "Change 7 conversation 1 -8",
            "entry_named_twice",
            "memory_entry_invalid",
            "memory_entry_invalid",
            "memory_entry_not_found",
            "memory_entry_invalid",
        ]
    );
}

/// A create, an update, and a merge that give an entry the type and the title of a delete mark
/// are refused, with no regard to white space at the two ends. The same title with another type
/// and a delete of an entry are admitted.
#[test]
fn a_delete_mark_refuses_its_type_and_title() {
    let entries = [2, 3, 4, 5]
        .map(|number| entry(number, MemoryEntryOrigin::Chapter, &format!("条{number}")));
    let observations = [observation("chapter")];
    let marks = [MemoryDeleteMark {
        entry_type: MemoryEntryType::Other,
        title: " 旧灯 ".to_owned(),
        description: "述。".to_owned(),
    }];
    let facts = pass(&entries, &observations, &marks);
    let candidates = operations(json!([
        create("other", " 旧灯", "文。", json!([1])),
        update(2, "旧灯 "),
        merge(json!([3, 4]), "旧灯"),
        create("foreshadowing", "旧灯", "文。", json!([1])),
        delete(5),
    ]));

    assert_eq!(
        decided(&candidates, &facts),
        [
            "deleted_by_author",
            "deleted_by_author",
            "deleted_by_author",
            "Create 6 chapter 1",
            "Delete 5 chapter 0",
        ]
    );
}

/// The entry of the author keeps its text: each operation kind that names it is refused, and a
/// create with its type and its title is refused by the write rule. A conflict flag is the one
/// kind that the Host admits, and its version has the values of the entry.
#[test]
fn no_operation_changes_an_entry_of_the_author() {
    let entries = [
        entry(1, MemoryEntryOrigin::AuthorRequest, "阿青"),
        entry(2, MemoryEntryOrigin::Chapter, "条2"),
    ];
    let observations = [observation("chapter")];
    let facts = pass(&entries, &observations, &[]);
    let kinds = [
        create("other", "阿青", "新文。", json!([1])),
        update(1, "阿青"),
        merge(json!([2, 1]), "阿青"),
        delete(1),
        flag(1, "与第三章不符。", json!([1])),
    ];
    let refusals: Vec<String> = kinds
        .iter()
        .flat_map(|kind| decided(&operations(json!([kind])), &facts))
        .collect();
    let flagged = settle_operations_call(&operations(json!([&kinds[4]])), &facts);
    let Ok(admitted) = &flagged.decisions[0] else {
        panic!("the flag is admitted");
    };
    let version = &admitted.version;

    assert_eq!(
        (
            refusals,
            [&version.title, &version.description, &version.text]
        ),
        (
            [
                "memory_entry_exists",
                "author_entry",
                "author_entry",
                "author_entry",
                "Change 1 author_request 0",
            ]
            .map(String::from)
            .to_vec(),
            [
                &entries[0].entry.title,
                &entries[0].entry.description,
                &entries[0].entry.text
            ]
        )
    );
}

/// A flag version has the values of the version before it, the trimmed reason, and the source of
/// the first observation. A flag needs a current entry of the author and one or more
/// observations, and no other value. Its reason is one line at most as long as a description, and
/// a line break at an end of the reason also refuses the flag. It names its
/// entry for the operations after it.
#[test]
fn a_conflict_flag_keeps_the_entry_and_each_refusal_of_a_flag() {
    let mut entries: Vec<PassMemoryEntry> = (1..=10)
        .map(|number| {
            entry(
                number,
                MemoryEntryOrigin::AuthorRequest,
                &format!("条{number}"),
            )
        })
        .collect();
    entries[1].origin = MemoryEntryOrigin::Chapter;
    entries[2].origin = MemoryEntryOrigin::Conversation;
    let observations = [observation("conversation"), observation("chapter")];
    let facts = pass(&entries, &observations, &[]);
    let mut titled = flag(8, "不符。", json!([1]));
    titled["title"] = json!("条8");
    let candidates = operations(json!([
        flag(1, " 与第三章不符。 ", json!([2, 1])),
        update(1, "条1"),
        flag(2, "不符。", json!([1])),
        flag(3, "不符。", json!([1])),
        flag(99, "不符。", json!([1])),
        flag(4, &"长".repeat(101), json!([1])),
        flag(5, &"长".repeat(100), json!([1])),
        flag(6, "不符。\n第二行", json!([1])),
        flag(9, "不符。\n", json!([1])),
        flag(10, "\r\n不符。", json!([1])),
        flag(7, "不符。", json!([])),
        titled,
    ]));
    let settlement = settle_operations_call(&candidates, &facts);
    let Ok(admitted) = &settlement.decisions[0] else {
        panic!("the first flag is admitted");
    };
    let mut expected = entries[0].entry.clone();
    expected.version = 2;

    assert_eq!(
        (
            decided(&candidates, &facts),
            (
                admitted.version.state,
                admitted.version.entry_type,
                [
                    &admitted.version.title,
                    &admitted.version.description,
                    &admitted.version.text
                ],
                admitted.version.version
            ),
            admitted.version.flag.clone()
        ),
        (
            [
                "Change 1 author_request 0",
                "entry_named_twice",
                "flag_needs_author_entry",
                "flag_needs_author_entry",
                "memory_entry_not_found",
                "memory_entry_too_long",
                "Change 5 author_request 0",
                "memory_entry_invalid",
                "memory_entry_invalid",
                "memory_entry_invalid",
                "memory_entry_invalid",
                "memory_entry_invalid",
            ]
            .map(String::from)
            .to_vec(),
            (
                MemoryEntryState::Current,
                expected.entry_type,
                [&expected.title, &expected.description, &expected.text],
                expected.version
            ),
            Some(MemoryConflictFlag {
                reason: "与第三章不符。".to_owned(),
                source: observations[1].source.clone(),
            })
        )
    );
}

/// A conflict of the input with the reason `reason`, from the Chapter of the tests.
fn host_conflict(entry: u64, reason: &str) -> PassConflict {
    PassConflict {
        entry,
        reason: reason.to_owned(),
        source: observation("chapter").source,
    }
}

/// The Host writes a flag for each conflict whose entry is a current entry of the author, after
/// the operations of the model, so its flag is the current flag. A conflict of a background entry
/// or of a number with no entry gives no flag. An empty list with a flag is `applied`, and a
/// stale pass writes no flag.
#[test]
fn the_host_flags_each_conflict_of_an_entry_of_the_author_after_the_operations() {
    let mut entries = [1, 2, 3].map(|n| entry(n, MemoryEntryOrigin::AuthorRequest, "条"));
    entries[1].origin = MemoryEntryOrigin::Chapter;
    let observations = [observation("conversation")];
    let conflicts = [
        host_conflict(1, "甲"),
        host_conflict(2, "乙"),
        host_conflict(99, "丙"),
        host_conflict(3, "丁"),
        host_conflict(3, "戊"),
    ];
    let facts = PassFacts {
        conflicts: &conflicts,
        ..pass(&entries, &observations, &[])
    };
    let flags = |candidates: &[MemoryOperationCandidate], facts: &PassFacts<'_>| {
        let settlement = settle_operations_call(candidates, facts);
        let flags: Vec<(u64, u64, String)> = (settlement.flags.iter())
            .map(|f| {
                (
                    f.version.number,
                    f.version.version,
                    f.version.flag.clone().unwrap().reason,
                )
            })
            .collect();
        (settlement.outcome, flags)
    };
    let model = operations(json!([flag(1, "模型", json!([1]))]));
    let stale = PassFacts {
        memory_revision: 2,
        ..facts
    };
    let line = |number, version, reason: &str| (number, version, reason.to_owned());

    assert_eq!(
        [
            flags(&model, &facts),
            flags(&[], &facts),
            flags(&model, &stale)
        ],
        [
            (
                ConsolidationOutcome::Applied,
                vec![line(1, 3, "甲"), line(3, 2, "丁"), line(3, 3, "戊")]
            ),
            (
                ConsolidationOutcome::Applied,
                vec![line(1, 2, "甲"), line(3, 2, "丁"), line(3, 3, "戊")]
            ),
            (ConsolidationOutcome::Stale, vec![]),
        ]
    );
}
