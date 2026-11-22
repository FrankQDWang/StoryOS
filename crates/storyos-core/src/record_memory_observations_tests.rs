use serde_json::json;

use super::*;
use crate::RECORD_MEMORY_OBSERVATIONS_TOOL_SPEC as SPEC;
use ObservationRefusal::{Invalid, TooLong, TooMany};

fn item(entry_type: &str, title: &str, text: &str) -> ObservationCandidate {
    ObservationCandidate {
        kind: ObservationKind::Observation,
        entry_type: Some(entry_type.to_owned()),
        title: Some(title.to_owned()),
        text: Some(text.to_owned()),
        entry: None,
        reason: None,
    }
}

fn conflict(entry: i64, reason: &str) -> ObservationCandidate {
    ObservationCandidate {
        kind: ObservationKind::Conflict,
        entry_type: None,
        title: None,
        text: None,
        entry: Some(entry),
        reason: Some(reason.to_owned()),
    }
}

/// The entries of the author of the tests.
const AUTHOR: [u64; 2] = [1, 4];

#[test]
fn the_intake_takes_one_list_and_refuses_another_shape_and_u0000() {
    let given = json!({"observations": [{
        "kind": "observation", "type": "character", "title": "林舟", "text": "船夫。",
        "entry": null, "reason": null
    }]});

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
    // A missing key that the kind does not use is null (#1685).
    let short = json!({"observations": [
        {"kind": "observation", "type": "character", "title": "林舟", "text": "船夫。"},
        {"kind": "conflict", "entry": 1, "reason": "不符。"}
    ]});
    assert_eq!(
        SPEC.intake(&short),
        Ok(ToolIntake::RecordMemoryObservations(vec![
            item("character", "林舟", "船夫。"),
            conflict(1, "不符。")
        ]))
    );
    for malformed in [
        json!({}),
        json!({"observations": [{"type": "character", "title": "林舟", "text": "船夫。",
            "entry": null, "reason": null}]}),
        json!({"observations": [{"kind": "observation", "type": "character", "title": "林舟"}]}),
        json!({"observations": [{"kind": "conflict", "entry": 1}]}),
        json!({"observations": [{
            "kind": "observation", "type": "character", "title": "林舟", "text": "船夫。",
            "entry": "一"
        }]}),
        json!({"observations": [], "reason": "no"}),
    ] {
        assert_eq!(SPEC.intake(&malformed), Err(IntakeRefusal::Malformed));
    }
    let unstorable = json!({"observations": [{
        "kind": "conflict", "type": null, "title": null, "text": null, "entry": 1,
        "reason": "b\u{0}"
    }]});
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
        assert_eq!(validate_observations(&list, &AUTHOR), Err(refusal));
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
        validate_observations(&list, &AUTHOR),
        Ok(vec![
            MemoryObservation::Fact {
                entry_type: MemoryEntryType::Character,
                title: "林舟".to_owned(),
                text: character_text,
            },
            MemoryObservation::Fact {
                entry_type: MemoryEntryType::Foreshadowing,
                title: "铜铃".to_owned(),
                text: "铜铃会再响。".to_owned(),
            },
        ])
    );
    assert_eq!(validate_observations(&[], &AUTHOR), Ok(Vec::new()));
    assert_eq!(
        validate_observations(&vec![list[1].clone(); 5], &AUTHOR)
            .unwrap()
            .len(),
        5
    );
}

/// ADR 0101 section 3: an observation of a character has a text of at most 4,000 characters, and
/// another type keeps the limit of 600 characters.
#[test]
fn an_observation_of_a_character_has_at_most_4000_characters() {
    let text = |count: usize| "字".repeat(count);
    let decided = [
        item("character", "林舟", &text(4000)),
        item("character", "林舟", &text(4001)),
        item("other", "桥", &text(600)),
        item("other", "桥", &text(601)),
    ]
    .map(|item| {
        validate_observations(&[item], &AUTHOR)
            .map(|_| ())
            .map_err(ObservationRefusal::label)
    });

    assert_eq!(
        decided,
        [
            Ok(()),
            Err("observation_too_long"),
            Ok(()),
            Err("observation_too_long")
        ]
    );
}

/// A valid list settles the capture with its outcome and completes the Run. A refused list
/// records no observation and does not complete the Run.
#[test]
fn a_settled_call_decides_the_outcome_of_the_capture_and_completes_the_run() {
    let one = item("foreshadowing", "铜铃", "铜铃会再响。");
    let facts = |list: &[ObservationCandidate]| {
        let settlement = settle_observation_call(list, &AUTHOR);
        let outcome = match &settlement {
            ObservationCallSettlement::Settled {
                outcome,
                observations,
                result,
            } => Ok((*outcome, observations.len(), result.text.clone())),
            ObservationCallSettlement::Refused { refusal, result } => {
                Err((*refusal, result.text.clone()))
            }
        };
        (settlement.call_state(), settlement.completes_run(), outcome)
    };

    assert_eq!(
        [
            facts(&[]),
            facts(std::slice::from_ref(&one)),
            facts(&vec![one; 6])
        ],
        [
            (
                ToolCallState::Settled,
                true,
                Ok((
                    CaptureOutcome::NoObservation,
                    0,
                    observations_result(0).text
                ))
            ),
            (
                ToolCallState::Settled,
                true,
                Ok((
                    CaptureOutcome::ObservationsRecorded,
                    1,
                    observations_result(1).text
                ))
            ),
            (
                ToolCallState::Refused,
                false,
                Err((TooMany, TooMany.result().text))
            ),
        ]
    );
    assert_eq!(
        [
            CaptureOutcome::ObservationsRecorded,
            CaptureOutcome::NoObservation
        ]
        .map(CaptureOutcome::label),
        ["observations_recorded", "no_observation"]
    );
}

/// A conflict counts as one observation of the limit and keeps its place in the list. It names a
/// current entry of the author and a reason of one line at most as long as a description, and it
/// has no other value. Each refusal gives the position of the item.
#[test]
fn a_conflict_item_names_an_entry_of_the_author_with_a_short_reason() {
    let fact = item("foreshadowing", "铜铃", "铜铃会再响。");
    let mut with_title = conflict(1, "不符。");
    with_title.title = Some("铃".to_owned());
    let mut fact_with_entry = fact.clone();
    fact_with_entry.entry = Some(1);
    let refused = [
        (
            vec![fact.clone(), conflict(2, "不符。")],
            "conflict_entry_unavailable",
        ),
        (vec![conflict(-1, "不符。")], "conflict_entry_unavailable"),
        (
            vec![conflict(1, &"长".repeat(101))],
            "memory_entry_too_long",
        ),
        (vec![conflict(1, "不符。\n")], "observation_invalid"),
        (vec![conflict(1, " ")], "observation_invalid"),
        (vec![with_title], "observation_invalid"),
        (vec![fact_with_entry], "observation_invalid"),
        (vec![conflict(1, "不符。"); 6], "too_many_observations"),
    ];
    let labels: Vec<(&str, &str)> = refused
        .iter()
        .map(|(list, label)| {
            let refusal = validate_observations(list, &AUTHOR).unwrap_err();
            (refusal.label(), *label)
        })
        .collect();
    let list = [conflict(4, &format!(" {} ", "长".repeat(100))), fact];

    assert_eq!(
        (
            labels.iter().map(|(given, _)| *given).collect::<Vec<_>>(),
            validate_observations(&list, &AUTHOR).map(|items| items[0].clone()),
            ObservationRefusal::ConflictEntryUnavailable { position: 2 }
                .result()
                .text
                .contains(
                    "the conflict 2 names no current entry of the author. An entry of the author \
                     has the origin author_request in the result of read_memory_entry"
                )
        ),
        (
            labels
                .iter()
                .map(|(_, expected)| *expected)
                .collect::<Vec<_>>(),
            Ok(MemoryObservation::Conflict {
                entry: 4,
                reason: "长".repeat(100)
            }),
            true
        )
    );
}
