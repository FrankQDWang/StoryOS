use serde_json::{Value, json};

use super::{MemoryOperationCandidate, PassFacts, PassMemoryEntry, intake, settle_operations_call};
use crate::{
    ConsolidationOutcome, CurrentMemoryEntry, IntakeRefusal, MemoryEntryOrigin, MemoryEntryType,
    MemorySourceReference, MemoryWriteAction, PassObservation,
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
    }
}

fn create(entry_type: &str, title: &str, text: &str, observations: Value) -> Value {
    json!({
        "action": "create", "entry": null, "type": entry_type, "title": title,
        "description": "述。", "text": text, "observations": observations, "reason": null
    })
}

fn update(entry: u64, title: &str) -> Value {
    json!({
        "action": "update", "entry": entry, "type": null, "title": title, "description": "新述。",
        "text": "新文。", "observations": [1], "reason": null
    })
}

fn delete(entry: u64) -> Value {
    json!({
        "action": "delete", "entry": entry, "type": null, "title": null, "description": null,
        "text": null, "observations": [], "reason": "不再成立。"
    })
}

/// The decision of each operation: the action, the number, the origin, and the count of the
/// sources of an admitted operation, or the label of a refusal.
fn decided(candidates: &[MemoryOperationCandidate], facts: &PassFacts<'_>) -> Vec<String> {
    settle_operations_call(candidates, facts)
        .decisions
        .iter()
        .map(|decision| match decision {
            Ok(admitted) => format!(
                "{:?} {} {} {}",
                admitted.version.action,
                admitted.version.number,
                admitted.origin.as_str(),
                admitted.sources.len()
            ),
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
        (None, &json!(["create", "update", "delete"]))
    );
    let mut partial = delete(1);
    partial.as_object_mut().unwrap().remove("reason");
    let mut extra = delete(1);
    extra["state"] = json!("deleted");
    let mut action = delete(1);
    action["action"] = json!("merge");
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

/// A delete needs a reason, an update needs its three values and no type, and a delete of an
/// entry keeps its origin. An operation names its entry also when the Host refuses it, and an
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
    };
    let mut no_reason = delete(4);
    no_reason["reason"] = json!(" ");
    let mut typed = update(5, "灯");
    typed["type"] = json!("other");
    let mut partial = update(6, "灯");
    partial["text"] = Value::Null;
    let candidates = operations(json!([
        no_reason,
        typed,
        partial,
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
