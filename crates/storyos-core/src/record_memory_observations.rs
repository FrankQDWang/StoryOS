//! The Host Tool `record_memory_observations` (ADR 0094 sections 5, 7, and 8): the output Tool of
//! a capture. Its one call gives the typed list of the Memory Observations of the capture, and the
//! settled call ends the capture. An empty list is a correct result. An item is an observation of
//! a fact or a conflict with an entry of the author. One Core function validates the list, and the
//! store writes the items in the transaction that settles the call.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    IntakeRefusal, MEMORY_CHARACTER_TEXT_MAX_CHARS, MEMORY_DESCRIPTION_MAX_CHARS,
    MEMORY_TEXT_MAX_CHARS, MEMORY_TITLE_MAX_CHARS, MemoryEntryType, ToolCallState, ToolIntake,
    ToolOperation, ToolResultText, ToolSpec,
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

/// The keys of an item. The strict schema requires each one, also when its value is null.
const KEYS: [&str; 6] = ["kind", "type", "title", "text", "entry", "reason"];

/// The keys that an item of a kind must have. The intake reads a missing key that the kind does
/// not use as null: `entry` and `reason` of an observation, and `type`, `title`, and `text` of a
/// conflict (#1685). An item with no known kind must have each key.
fn required_keys(kind: Option<&Value>) -> &'static [&'static str] {
    match kind.and_then(Value::as_str) {
        Some("observation") => &KEYS[..4],
        Some("conflict") => &["kind", "entry", "reason"],
        _ => &KEYS,
    }
}

/// The strict JSON Schema of the arguments. A value that the kind of an item does not use is null.
pub(crate) fn parameters() -> Value {
    let mut types: Vec<Value> = MemoryEntryType::ALL
        .iter()
        .map(|t| json!(t.as_str()))
        .collect();
    types.push(Value::Null);
    json!({
        "type": "object",
        "properties": {
            "observations": {
                "type": "array",
                "description": "At most 5 observations, or an empty list.",
                "items": {
                    "type": "object",
                    "properties": {
                        "kind": {
                            "type": "string",
                            "enum": ["observation", "conflict"],
                            "description": "observation for a fact. conflict when the input \
                                            conflicts with an entry of the author."
                        },
                        "type": {
                            "type": ["string", "null"],
                            "enum": types,
                            "description": "Null for a conflict."
                        },
                        "title": {
                            "type": ["string", "null"],
                            "description": "The subject in one line, at most 30 characters. \
                                            Null for a conflict."
                        },
                        "text": {
                            "type": ["string", "null"],
                            "description": "Markdown, at most 600 characters, or 1500 for a \
                                            character. Null for a conflict."
                        },
                        "entry": {
                            "type": ["integer", "null"],
                            "description": "The id of the entry of the author in the section \
                                            Memory index. Null for an observation."
                        },
                        "reason": {
                            "type": ["string", "null"],
                            "description": "How the input conflicts with the entry, in one line \
                                            of at most 100 characters. Null for an observation."
                        }
                    },
                    "required": KEYS,
                    "additionalProperties": false
                }
            }
        },
        "required": ["observations"],
        "additionalProperties": false
    })
}

/// The kind of one item of the list.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationKind {
    Observation,
    Conflict,
}

/// One item of the list as the model gave it. The type is the given text: the validation refuses
/// a type that is not one of the six types.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ObservationCandidate {
    pub kind: ObservationKind,
    #[serde(rename = "type")]
    pub entry_type: Option<String>,
    pub title: Option<String>,
    pub text: Option<String>,
    pub entry: Option<i64>,
    pub reason: Option<String>,
}

/// One validated item: a Memory Observation of a fact, or a conflict with the current entry of
/// the author with the number `entry` (ADR 0094 section 7).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryObservation {
    Fact {
        entry_type: MemoryEntryType,
        title: String,
        text: String,
    },
    Conflict {
        entry: u64,
        reason: String,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    observations: Vec<ObservationCandidate>,
}

/// The typed intake. The ToolCall record cannot hold U+0000, so a value with it does not fit.
pub(crate) fn intake(arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
    let complete = arguments["observations"].as_array().is_some_and(|items| {
        items.iter().all(|item| {
            item.as_object().is_some_and(|item| {
                required_keys(item.get("kind"))
                    .iter()
                    .all(|key| item.contains_key(*key))
            })
        })
    });
    if !complete {
        return Err(IntakeRefusal::Malformed);
    }
    let Arguments { observations } =
        Arguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
    let unstorable = observations.iter().any(|item| {
        [&item.entry_type, &item.title, &item.text, &item.reason]
            .iter()
            .any(|value| value.as_ref().is_some_and(|value| value.contains('\0')))
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
        .map(|item| {
            json!({
                "kind": item.kind,
                "type": item.entry_type,
                "title": item.title,
                "text": item.text,
                "entry": item.entry,
                "reason": item.reason,
            })
        })
        .collect();
    json!({ "observations": items })
}

/// Why the Host refused one `record_memory_observations` call. The position of an item starts
/// at 1. A refused call records no observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationRefusal {
    TooMany,
    Invalid {
        position: usize,
    },
    TooLong {
        position: usize,
    },
    /// A conflict names a number that is not a current entry of the author.
    ConflictEntryUnavailable {
        position: usize,
    },
    /// The reason of a conflict is longer than the limit of a description.
    ReasonTooLong {
        position: usize,
    },
}

impl ObservationRefusal {
    /// The stable label of the refusal, for the `refusal` of the ToolCall row.
    pub fn label(self) -> &'static str {
        match self {
            Self::TooMany => "too_many_observations",
            Self::Invalid { .. } => "observation_invalid",
            Self::TooLong { .. } => "observation_too_long",
            Self::ConflictEntryUnavailable { .. } => "conflict_entry_unavailable",
            Self::ReasonTooLong { .. } => "memory_entry_too_long",
        }
    }

    /// The result of the refused call: the reason and the position of the item.
    pub fn result(self) -> ToolResultText {
        let reason = match self {
            Self::TooMany => {
                format!("the list has more than {CAPTURE_OBSERVATIONS_MAX} observations")
            }
            Self::Invalid { position } => format!(
                "the observation {position} is not valid. Its type must be one of the six types. \
                 Its title and its text must not be empty, and its title must be one line. A \
                 conflict has only an entry and a reason of one line"
            ),
            Self::ConflictEntryUnavailable { position } => format!(
                "the conflict {position} names no current entry of the author. An entry of the \
                 author has the origin author_request in the result of read_memory_entry"
            ),
            Self::ReasonTooLong { position } => format!(
                "the reason of the conflict {position} is too long. A reason has at most \
                 {MEMORY_DESCRIPTION_MAX_CHARS} characters"
            ),
            Self::TooLong { position } => format!(
                "the observation {position} is too long. A title has at most \
                 {MEMORY_TITLE_MAX_CHARS} characters. A text has at most {MEMORY_TEXT_MAX_CHARS} \
                 characters, or {MEMORY_CHARACTER_TEXT_MAX_CHARS} for a character"
            ),
        };
        crate::host_tool_result(&format!(
            "The Host refused this call because {reason}. It recorded no observation. Correct \
             the list and call record_memory_observations again."
        ))
    }
}

/// Validates the list of one call with the Host limits (ADR 0094 sections 7 and 8). The count
/// comes first, and then each item in list order. A conflict counts as one observation, and it
/// must name one of `author_entries`, the numbers of the current entries of the author. One
/// refused item refuses the call.
pub fn validate_observations(
    candidates: &[ObservationCandidate],
    author_entries: &[u64],
) -> Result<Vec<MemoryObservation>, ObservationRefusal> {
    if candidates.len() > CAPTURE_OBSERVATIONS_MAX {
        return Err(ObservationRefusal::TooMany);
    }
    let mut observations = Vec::new();
    for (position, item) in (1..).zip(candidates) {
        let fact = [&item.entry_type, &item.title, &item.text];
        let observation = item.kind == ObservationKind::Observation
            && fact.iter().all(|value| value.is_some())
            && item.entry.is_none()
            && item.reason.is_none();
        if item.kind == ObservationKind::Conflict {
            observations.push(crate::memory_conflict_flag::capture_conflict(
                item,
                position,
                author_entries,
            )?);
            continue;
        }
        let given = |value: &Option<String>| value.clone().unwrap_or_default();
        let (title, text) = (given(&item.title), given(&item.text));
        let title = title.trim();
        let line = !title.is_empty() && !title.chars().any(char::is_control);
        let text_valid = !text.trim().is_empty()
            && !text
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t');
        let Some(entry_type) = MemoryEntryType::parse(&given(&item.entry_type))
            .filter(|_| observation && line && text_valid)
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
        if title.chars().count() > MEMORY_TITLE_MAX_CHARS || text.chars().count() > text_limit {
            return Err(ObservationRefusal::TooLong { position });
        }
        observations.push(MemoryObservation::Fact {
            entry_type,
            title: title.to_owned(),
            text,
        });
    }
    Ok(observations)
}

/// The result of the settled call. The capture ends with it, so no request reads it.
pub fn observations_result(count: usize) -> ToolResultText {
    crate::host_tool_result(&match count {
        0 => "The Host recorded no observation, and the capture is complete.".to_owned(),
        count => format!("The Host recorded {count} observations, and the capture is complete."),
    })
}

/// The outcome of a settled capture (ADR 0094 section 5).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureOutcome {
    ObservationsRecorded,
    /// The settled list is empty. An empty list is a correct result.
    NoObservation,
    /// The last RunStep gave no output call that settled.
    StepLimit,
    /// The Run ended with no settled output call, for the reason of the Run.
    Failed,
}

impl CaptureOutcome {
    /// The persisted name of the outcome, for `memory_captures.outcome`.
    pub fn label(self) -> &'static str {
        match self {
            Self::ObservationsRecorded => "observations_recorded",
            Self::NoObservation => "no_observation",
            Self::StepLimit => "step_limit",
            Self::Failed => "failed",
        }
    }
}

/// The settlement of one `record_memory_observations` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObservationCallSettlement {
    /// The valid list settles the capture with `outcome` and stores each observation in list
    /// order. The settled call completes the Run, so no request follows it.
    Settled {
        outcome: CaptureOutcome,
        observations: Vec<MemoryObservation>,
        result: ToolResultText,
    },
    /// The refused list records no observation. The model can correct the list in the next
    /// RunStep.
    Refused {
        refusal: ObservationRefusal,
        result: ToolResultText,
    },
}

impl ObservationCallSettlement {
    /// The state of the ToolCall row of the call.
    pub fn call_state(&self) -> ToolCallState {
        match self {
            Self::Settled { .. } => ToolCallState::Settled,
            Self::Refused { .. } => ToolCallState::Refused,
        }
    }

    /// Whether the call completes the Run of the capture.
    pub fn completes_run(&self) -> bool {
        match self {
            Self::Settled { .. } => true,
            Self::Refused { .. } => false,
        }
    }
}

/// Settles one call: Core validates the list against the numbers of the current entries of the
/// author, and decides the outcome of the capture.
pub fn settle_observation_call(
    candidates: &[ObservationCandidate],
    author_entries: &[u64],
) -> ObservationCallSettlement {
    match validate_observations(candidates, author_entries) {
        Ok(observations) => ObservationCallSettlement::Settled {
            outcome: match observations.is_empty() {
                true => CaptureOutcome::NoObservation,
                false => CaptureOutcome::ObservationsRecorded,
            },
            result: observations_result(observations.len()),
            observations,
        },
        Err(refusal) => ObservationCallSettlement::Refused {
            refusal,
            result: refusal.result(),
        },
    }
}

#[cfg(test)]
#[path = "record_memory_observations_tests.rs"]
mod tests;
