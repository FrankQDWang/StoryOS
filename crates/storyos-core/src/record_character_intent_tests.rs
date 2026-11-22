use serde_json::json;

use super::{
    CharacterIntent, IntentRefusal, RECORD_CHARACTER_INTENT_TOOL_SPEC, validate_character_intent,
};
use crate::{IntakeRefusal, ToolIntake};

/// Each value above a limit of ADR 0101 section 4 is refused with its own label, and a value at
/// each limit is valid. Each limit counts the outer white space, and a question with an outer line
/// break is invalid.
#[test]
fn a_call_above_a_limit_is_refused_and_a_call_at_each_limit_is_valid() {
    let question = |count: usize| "问".repeat(count);
    let calls: [(String, Vec<String>); 8] = [
        ("想".repeat(151), vec![]),
        ("想去渡口。".to_owned(), vec![question(5); 4]),
        ("想去渡口。".to_owned(), vec![question(5), question(101)]),
        (" \n ".to_owned(), vec![]),
        (
            "想去渡口。".to_owned(),
            vec!["船在哪里？\n谁的船？".to_owned()],
        ),
        (format!(" {} ", "想".repeat(149)), vec![]),
        ("想去渡口。".to_owned(), vec![format!("{}\n", question(5))]),
        ("想".repeat(150), vec![question(100); 3]),
    ];
    let results = calls.map(|(intent, questions)| {
        validate_character_intent(&intent, &questions).map_err(IntentRefusal::label)
    });
    assert_eq!(
        results,
        [
            Err("intent_too_long"),
            Err("too_many_questions"),
            Err("question_too_long"),
            Err("intent_empty"),
            Err("question_invalid"),
            Err("intent_too_long"),
            Err("question_invalid"),
            Ok(CharacterIntent {
                intent: "想".repeat(150),
                questions: vec![question(100); 3],
            }),
        ]
    );
}

/// The result of a refused call tells the model the reason, the position, and the correction.
#[test]
fn the_result_of_a_refused_call_names_the_reason_and_the_correction() {
    assert_eq!(
        IntentRefusal::QuestionTooLong { position: 2 }.result(),
        crate::host_tool_result(
            "The Host refused this call because the question 2 is too long. A question has at most \
             100 characters. It recorded no intent. Correct the call and call \
             record_character_intent again."
        )
    );
}

/// The intake takes the two declared arguments, and it refuses another key and a text with
/// U+0000.
#[test]
fn the_intake_takes_the_intent_and_the_questions() {
    let intake = |arguments| RECORD_CHARACTER_INTENT_TOOL_SPEC.intake(&arguments);
    assert_eq!(
        [
            intake(json!({ "intent": "想去渡口。", "questions": ["船在哪里？"] })),
            intake(json!({ "intent": "想去渡口。" })),
            intake(json!({ "intent": "想去渡口。", "questions": [], "reason": "" })),
            intake(json!({ "intent": "想\u{0}", "questions": [] })),
        ],
        [
            Ok(ToolIntake::RecordCharacterIntent {
                intent: "想去渡口。".to_owned(),
                questions: vec!["船在哪里？".to_owned()],
            }),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::UnstorableText),
        ]
    );
}
