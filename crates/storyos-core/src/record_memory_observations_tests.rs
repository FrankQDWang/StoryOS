use serde_json::json;

use super::*;
use crate::RECORD_MEMORY_OBSERVATIONS_TOOL_SPEC as SPEC;
use ObservationRefusal::{Invalid, TooLong, TooMany};

fn item(entry_type: &str, title: &str, text: &str) -> ObservationCandidate {
    ObservationCandidate {
        entry_type: entry_type.to_owned(),
        title: title.to_owned(),
        text: text.to_owned(),
    }
}

#[test]
fn the_intake_takes_one_list_and_refuses_another_shape_and_u0000() {
    let given = json!({"observations": [{"type": "character", "title": "林舟", "text": "船夫。"}]});

    assert_eq!(
        SPEC.intake(&given),
        Ok(ToolIntake::RecordMemoryObservations(vec![item(
            "character",
            "林舟",
            "船夫。"
        )]))
    );
    assert_eq!(
        SPEC.intake(&json!({"observations": []})),
        Ok(ToolIntake::RecordMemoryObservations(Vec::new()))
    );
    for malformed in [
        json!({}),
        json!({"observations": [{"type": "character", "title": "林舟"}]}),
        json!({"observations": [], "reason": "no"}),
    ] {
        assert_eq!(SPEC.intake(&malformed), Err(IntakeRefusal::Malformed));
    }
    let unstorable = json!({"observations": [{"type": "other", "title": "a", "text": "b\u{0}"}]});
    assert_eq!(SPEC.intake(&unstorable), Err(IntakeRefusal::UnstorableText));
}

/// One row for each refusal of the ticket table, in the order of the checks.
#[test]
fn one_refused_item_refuses_the_list_with_its_position() {
    let valid = item("major_event", "沉船", "船在雨夜沉了。");
    let long_title = "长".repeat(MEMORY_TITLE_MAX_CHARS + 1);
    let long_text = "字".repeat(MEMORY_TEXT_MAX_CHARS + 1);
    let cases = [
        (vec![valid.clone(); 6], ObservationRefusal::TooMany),
        (
            vec![valid.clone(), item("place", "港口", "雨。")],
            ObservationRefusal::Invalid { position: 2 },
        ),
        (
            vec![item("other", " ", "雨。")],
            ObservationRefusal::Invalid { position: 1 },
        ),
        (
            vec![item("other", "雨", "\n")],
            ObservationRefusal::Invalid { position: 1 },
        ),
        (
            vec![item("other", "雨\n港", "雨。")],
            ObservationRefusal::Invalid { position: 1 },
        ),
        (
            vec![
                valid.clone(),
                valid.clone(),
                item("other", &long_title, "雨。"),
            ],
            ObservationRefusal::TooLong { position: 3 },
        ),
        (
            vec![item("relationship", "林舟与阿青", &long_text)],
            ObservationRefusal::TooLong { position: 1 },
        ),
    ];

    for (list, refusal) in cases {
        assert_eq!(validate_observations(&list), Err(refusal));
    }
    let labels =
        [TooMany, Invalid { position: 1 }, TooLong { position: 1 }].map(ObservationRefusal::label);
    let labels = labels.join(" ");
    assert_eq!(
        labels,
        "too_many_observations observation_invalid observation_too_long"
    );
    let text = TooLong { position: 3 }.result().text;
    assert!(text.contains("the observation 3 is too long"));
}

#[test]
fn a_valid_list_keeps_its_order_and_a_character_has_the_longer_text() {
    let character_text = "人".repeat(MEMORY_CHARACTER_TEXT_MAX_CHARS);
    let list = vec![
        item("character", " 林舟 ", &character_text),
        item("foreshadowing", "铜铃", "铜铃会再响。"),
    ];

    assert_eq!(
        validate_observations(&list),
        Ok(vec![
            MemoryObservation {
                entry_type: MemoryEntryType::Character,
                title: "林舟".to_owned(),
                text: character_text,
            },
            MemoryObservation {
                entry_type: MemoryEntryType::Foreshadowing,
                title: "铜铃".to_owned(),
                text: "铜铃会再响。".to_owned(),
            },
        ])
    );
    assert_eq!(validate_observations(&[]), Ok(Vec::new()));
    assert_eq!(
        validate_observations(&vec![list[1].clone(); 5])
            .unwrap()
            .len(),
        5
    );
}
