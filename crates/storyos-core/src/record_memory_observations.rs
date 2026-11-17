//! The Host Tool `record_memory_observations` (ADR 0094 sections 5 and 8): the output Tool of a
//! capture. Its one call gives the typed list of the Memory Observations of the capture, and the
//! settled call ends the capture. An empty list is a correct result. One Core function validates
//! the list, and the store writes the observations in the transaction that settles the call.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    IntakeRefusal, MEMORY_CHARACTER_TEXT_MAX_CHARS, MEMORY_TEXT_MAX_CHARS, MEMORY_TITLE_MAX_CHARS,
    MemoryEntryType, ToolIntake, ToolOperation, ToolResultText, ToolSpec,
};

pub const RECORD_MEMORY_OBSERVATIONS_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.record-memory-observations.v1",
    operation: ToolOperation::RecordMemoryObservations,
    intake_contract: &[],
    envelope: None,
    result_text_max_bytes: 512,
};

/// The Host limit of the number of observations of one capture. It is an initial value, and S7-D7
/// can change it.
pub const CAPTURE_OBSERVATIONS_MAX: usize = 5;

pub(crate) const DESCRIPTION: &str = "Records the Memory Observations of this capture and ends \
the capture. Call it one time, when you have read the input. An observation is a fact that a \
later Chapter needs and that a search of the manuscript cannot find. It is a writing preference \
of the author, a foreshadowing, a character, a relationship, a major event, or another fact. Give \
at most 5 observations. Give an empty list when the input has no such fact.";

/// The strict JSON Schema of the arguments.
pub(crate) fn parameters() -> Value {
    let types: Vec<&str> = MemoryEntryType::ALL.iter().map(|t| t.as_str()).collect();
    json!({
        "type": "object",
        "properties": {
            "observations": {
                "type": "array",
                "description": "At most 5 observations, or an empty list.",
                "items": {
                    "type": "object",
                    "properties": {
                        "type": { "type": "string", "enum": types },
                        "title": {
                            "type": "string",
                            "description": "The subject in one line, at most 30 characters."
                        },
                        "text": {
                            "type": "string",
                            "description": "Markdown, at most 600 characters, or 1500 for a \
                                            character."
                        }
                    },
                    "required": ["type", "title", "text"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["observations"],
        "additionalProperties": false
    })
}

/// One item of the list as the model gave it. The type is the given text: the validation refuses
/// a type that is not one of the six types.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ObservationCandidate {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub title: String,
    pub text: String,
}

/// One validated Memory Observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryObservation {
    pub entry_type: MemoryEntryType,
    pub title: String,
    pub text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    observations: Vec<ObservationCandidate>,
}

/// The typed intake. The ToolCall record cannot hold U+0000, so a value with it does not fit.
pub(crate) fn intake(arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
    let Arguments { observations } =
        Arguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
    let unstorable = observations.iter().any(|item| {
        [&item.entry_type, &item.title, &item.text]
            .iter()
            .any(|value| value.contains('\0'))
    });
    match unstorable {
        true => Err(IntakeRefusal::UnstorableText),
        false => Ok(ToolIntake::RecordMemoryObservations(observations)),
    }
}

/// The arguments of a validated call, as the ToolCall record keeps them.
pub(crate) fn arguments(observations: &[ObservationCandidate]) -> Value {
    let items: Vec<Value> = observations
        .iter()
        .map(|item| json!({ "type": item.entry_type, "title": item.title, "text": item.text }))
        .collect();
    json!({ "observations": items })
}

/// Why the Host refused one `record_memory_observations` call. The position of an item starts
/// at 1. A refused call records no observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationRefusal {
    TooMany,
    Invalid { position: usize },
    TooLong { position: usize },
}

impl ObservationRefusal {
    /// The stable label of the refusal, for the `refusal` of the ToolCall row.
    pub fn label(self) -> &'static str {
        match self {
            Self::TooMany => "too_many_observations",
            Self::Invalid { .. } => "observation_invalid",
            Self::TooLong { .. } => "observation_too_long",
        }
    }

    /// The result of the refused call: the reason and the position of the item.
    pub fn result(self) -> ToolResultText {
        let reason = match self {
            Self::TooMany => {
                format!("the list has more than {CAPTURE_OBSERVATIONS_MAX} observations")
            }
            Self::Invalid { position } => format!(
                "the observation {position} has a type that is not one of the six types, or a \
                 title or a text that is empty, or a title that is not one line"
            ),
            Self::TooLong { position } => format!(
                "the observation {position} is too long: a title has at most \
                 {MEMORY_TITLE_MAX_CHARS} characters, and a text at most {MEMORY_TEXT_MAX_CHARS}, \
                 or {MEMORY_CHARACTER_TEXT_MAX_CHARS} for a character"
            ),
        };
        crate::host_tool_result(&format!(
            "The Host refused this call because {reason}, so it recorded no observation. \
             Correct the list and call record_memory_observations again."
        ))
    }
}

/// Validates the list of one call with the Host limits (ADR 0094 section 8). The count comes
/// first, and then each item in list order. One refused item refuses the call.
pub fn validate_observations(
    candidates: &[ObservationCandidate],
) -> Result<Vec<MemoryObservation>, ObservationRefusal> {
    if candidates.len() > CAPTURE_OBSERVATIONS_MAX {
        return Err(ObservationRefusal::TooMany);
    }
    let mut observations = Vec::new();
    for (position, item) in (1..).zip(candidates) {
        let title = item.title.trim();
        let line = !title.is_empty() && !title.chars().any(char::is_control);
        let text = !item.text.trim().is_empty()
            && !item
                .text
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t');
        let Some(entry_type) = MemoryEntryType::parse(&item.entry_type).filter(|_| line && text)
        else {
            return Err(ObservationRefusal::Invalid { position });
        };
        let text_limit = match entry_type {
            MemoryEntryType::Character => MEMORY_CHARACTER_TEXT_MAX_CHARS,
            MemoryEntryType::WritingPreference
            | MemoryEntryType::Foreshadowing
            | MemoryEntryType::Relationship
            | MemoryEntryType::MajorEvent
            | MemoryEntryType::Other => MEMORY_TEXT_MAX_CHARS,
        };
        if title.chars().count() > MEMORY_TITLE_MAX_CHARS || item.text.chars().count() > text_limit
        {
            return Err(ObservationRefusal::TooLong { position });
        }
        observations.push(MemoryObservation {
            entry_type,
            title: title.to_owned(),
            text: item.text.clone(),
        });
    }
    Ok(observations)
}

#[cfg(test)]
#[path = "record_memory_observations_tests.rs"]
mod tests;
