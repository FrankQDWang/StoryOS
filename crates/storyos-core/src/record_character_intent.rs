//! The Host Tool `record_character_intent` (ADR 0101 section 4): the output Tool of an intent
//! pass. Its one call gives the intent of the character and at most three questions to the author.
//! The Host refuses a call above a limit, and the model gets the refusal as the result of the call,
//! so that the second RunStep can correct it. The settled call ends the pass.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{IntakeRefusal, ToolCallState, ToolIntake, ToolOperation, ToolResultText, ToolSpec};

pub(crate) const RECORD_CHARACTER_INTENT_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.record-character-intent.v1",
    operation: ToolOperation::RecordCharacterIntent,
    intake_contract: &[],
    envelope: None,
    result_text_max_bytes: 512,
};

/// The most characters of an intent (ADR 0101 section 4).
pub const CHARACTER_INTENT_MAX_CHARS: usize = 150;
/// The most questions of one intent pass.
pub const CHARACTER_QUESTIONS_MAX: usize = 3;
/// The most characters of one question.
pub const CHARACTER_QUESTION_MAX_CHARS: usize = 100;

pub(crate) const DESCRIPTION: &str = "Records the intent of the character and the questions to \
the author, and ends the pass. Call it one time.";

/// The strict JSON Schema of the arguments.
pub(crate) fn parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "intent": {
                "type": "string",
                "description": format!(
                    "What the character wants to do next, at most {CHARACTER_INTENT_MAX_CHARS} \
                     characters."
                )
            },
            "questions": {
                "type": "array",
                "description": format!(
                    "At most {CHARACTER_QUESTIONS_MAX} questions to the author, or an empty list."
                ),
                "items": {
                    "type": "string",
                    "description": format!(
                        "One sentence of at most {CHARACTER_QUESTION_MAX_CHARS} characters."
                    )
                }
            }
        },
        "required": ["intent", "questions"],
        "additionalProperties": false
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    intent: String,
    questions: Vec<String>,
}

/// The typed intake. The ToolCall record cannot hold U+0000, so a value with it does not fit.
pub(crate) fn intake(arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
    let Arguments { intent, questions } =
        Arguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
    match std::iter::once(&intent)
        .chain(&questions)
        .any(|text| text.contains('\0'))
    {
        true => Err(IntakeRefusal::UnstorableText),
        false => Ok(ToolIntake::RecordCharacterIntent { intent, questions }),
    }
}

/// Why the Host refused one `record_character_intent` call. The position of a question starts at
/// 1. A refused call records no intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntentRefusal {
    IntentEmpty,
    IntentTooLong,
    TooManyQuestions,
    /// The question is empty or has a line break.
    QuestionInvalid {
        position: usize,
    },
    QuestionTooLong {
        position: usize,
    },
}

impl IntentRefusal {
    /// The stable label of the refusal, for the `refusal` of the ToolCall row.
    pub fn label(self) -> &'static str {
        match self {
            Self::IntentEmpty => "intent_empty",
            Self::IntentTooLong => "intent_too_long",
            Self::TooManyQuestions => "too_many_questions",
            Self::QuestionInvalid { .. } => "question_invalid",
            Self::QuestionTooLong { .. } => "question_too_long",
        }
    }

    /// The result of the refused call: the reason and the position of the question.
    pub fn result(self) -> ToolResultText {
        let reason = match self {
            Self::IntentEmpty => "the intent is empty".to_owned(),
            Self::IntentTooLong => format!(
                "the intent is too long. An intent has at most {CHARACTER_INTENT_MAX_CHARS} \
                 characters"
            ),
            Self::TooManyQuestions => {
                format!("the list has more than {CHARACTER_QUESTIONS_MAX} questions")
            }
            Self::QuestionInvalid { position } => format!(
                "the question {position} is empty or has a line break. A question is one sentence"
            ),
            Self::QuestionTooLong { position } => format!(
                "the question {position} is too long. A question has at most \
                 {CHARACTER_QUESTION_MAX_CHARS} characters"
            ),
        };
        crate::host_tool_result(&format!(
            "The Host refused this call because {reason}. It recorded no intent. Correct the call \
             and call record_character_intent again."
        ))
    }
}

/// The validated result of one call: the intent and the questions as the call gave them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterIntent {
    pub intent: String,
    pub questions: Vec<String>,
}

/// Validates one call with the Host limits (ADR 0101 section 4): the intent first, then the count
/// of the questions, and then each question in list order. Each limit counts the characters of
/// the text as the call gave it, and the Host keeps that text. An intent of only white space is
/// empty, and a question of only white space or with a line break or another control character
/// anywhere is invalid.
pub(crate) fn validate_character_intent(
    intent: &str,
    questions: &[String],
) -> Result<CharacterIntent, IntentRefusal> {
    if intent.trim().is_empty() {
        return Err(IntentRefusal::IntentEmpty);
    }
    if intent.chars().count() > CHARACTER_INTENT_MAX_CHARS {
        return Err(IntentRefusal::IntentTooLong);
    }
    if questions.len() > CHARACTER_QUESTIONS_MAX {
        return Err(IntentRefusal::TooManyQuestions);
    }
    for (position, question) in (1..).zip(questions) {
        if question.trim().is_empty() || question.chars().any(breaks_line) {
            return Err(IntentRefusal::QuestionInvalid { position });
        }
        if question.chars().count() > CHARACTER_QUESTION_MAX_CHARS {
            return Err(IntentRefusal::QuestionTooLong { position });
        }
    }
    Ok(CharacterIntent {
        intent: intent.to_owned(),
        questions: questions.to_vec(),
    })
}

/// Whether `character` ends a line or is another control character: a question is one line.
fn breaks_line(character: char) -> bool {
    character.is_control() || matches!(character, '\u{2028}' | '\u{2029}')
}

/// The settlement of one `record_character_intent` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IntentCallSettlement {
    /// The valid call records the result. The settled call completes the Run, so no request
    /// follows it.
    Settled {
        intent: CharacterIntent,
        result: ToolResultText,
    },
    /// The refused call records nothing. The model can correct the call in the next RunStep.
    Refused {
        refusal: IntentRefusal,
        result: ToolResultText,
    },
}

impl IntentCallSettlement {
    /// The state of the ToolCall row of the call.
    pub fn call_state(&self) -> ToolCallState {
        match self {
            Self::Settled { .. } => ToolCallState::Settled,
            Self::Refused { .. } => ToolCallState::Refused,
        }
    }
}

/// The result text of a valid call whose result the Host recorded.
pub(crate) const INTENT_RECORDED: &str = "The Host recorded the intent, and the pass is complete.";

/// Settles one call with the Host limits.
pub fn settle_intent_call(intent: &str, questions: &[String]) -> IntentCallSettlement {
    match validate_character_intent(intent, questions) {
        Ok(intent) => IntentCallSettlement::Settled {
            intent,
            result: crate::host_tool_result(INTENT_RECORDED),
        },
        Err(refusal) => IntentCallSettlement::Refused {
            refusal,
            result: refusal.result(),
        },
    }
}

#[cfg(test)]
#[path = "record_character_intent_tests.rs"]
mod tests;
