//! The Core part of the function `apply_memory_operations` (ADR 0094 sections 6 and 8): the output
//! of a consolidation pass. Its one call gives the typed list of the operations of the pass. One
//! Core function validates each operation in list order against the entries and the observations
//! of the pass, and gives the value that the store applies in the transaction that settles the
//! call.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    ConsolidationOutcome, CurrentMemoryEntry, IntakeRefusal, MemoryEntryOrigin, MemoryEntryType,
    MemorySourceReference, MemoryVersionWrite, MemoryWrite, MemoryWriteAction, MemoryWriteRefusal,
    PassObservation, ToolResultText,
};

/// The keys of an operation. The strict schema requires each one, also when its value is null.
const KEYS: [&str; 8] = [
    "action",
    "entry",
    "type",
    "title",
    "description",
    "text",
    "observations",
    "reason",
];

/// The strict JSON Schema of the arguments. The schema names the types in the description and
/// not as an enum, so an operation with an unknown type is refused alone (`memory_entry_invalid`)
/// and does not refuse the call.
pub fn parameters() -> Value {
    let types: Vec<&str> = MemoryEntryType::ALL.iter().map(|t| t.as_str()).collect();
    let types = format!(
        "The type of a new entry: one of {}. Null for update and delete.",
        types.join(", ")
    );
    let value =
        |description: &str| json!({ "type": ["string", "null"], "description": description });
    json!({
        "type": "object",
        "properties": {
            "operations": {
                "type": "array",
                "description": "The operations in the order to apply, or an empty list.",
                "items": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["create", "update", "delete"] },
                        "entry": {
                            "type": ["integer", "null"],
                            "description": "The number of the entry to update or delete. Null \
                                            for create."
                        },
                        "type": value(&types),
                        "title": value("One line, at most 30 characters. Null for delete."),
                        "description": value("One line, at most 100 characters. Null for delete."),
                        "text": value(
                            "Markdown, at most 600 characters, or 1500 for a character. Null for \
                             delete."
                        ),
                        "observations": {
                            "type": "array",
                            "items": { "type": "integer" },
                            "description": "The numbers of the observations that the operation \
                                            uses. Create needs one or more."
                        },
                        "reason": value("Why the entry is deleted, in one line. Null otherwise.")
                    },
                    "required": KEYS,
                    "additionalProperties": false
                }
            }
        },
        "required": ["operations"],
        "additionalProperties": false
    })
}

/// The action of one operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryOperationAction {
    Create,
    Update,
    Delete,
}

/// One operation as the model gave it. The type is the given text: the validation refuses a type
/// that is not one of the six types.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MemoryOperationCandidate {
    pub action: MemoryOperationAction,
    pub entry: Option<i64>,
    #[serde(rename = "type")]
    pub entry_type: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub text: Option<String>,
    pub observations: Vec<i64>,
    pub reason: Option<String>,
}

impl MemoryOperationCandidate {
    /// The operation as the ToolCall record and the pass row keep it.
    pub fn arguments(&self) -> Value {
        json!({
            "action": self.action,
            "entry": self.entry,
            "type": self.entry_type,
            "title": self.title,
            "description": self.description,
            "text": self.text,
            "observations": self.observations,
            "reason": self.reason,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    operations: Vec<MemoryOperationCandidate>,
}

/// The typed intake of the arguments with the strict schema of `parameters`. Each operation has
/// each key. The ToolCall record cannot hold U+0000, so a value with it does not fit.
pub fn intake(arguments: &Value) -> Result<Vec<MemoryOperationCandidate>, IntakeRefusal> {
    let complete = arguments["operations"].as_array().is_some_and(|items| {
        items.iter().all(|item| {
            item.as_object()
                .is_some_and(|item| KEYS.iter().all(|key| item.contains_key(*key)))
        })
    });
    if !complete {
        return Err(IntakeRefusal::Malformed);
    }
    let Arguments { operations } =
        Arguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
    let unstorable = operations.iter().any(|item| {
        [
            &item.entry_type,
            &item.title,
            &item.description,
            &item.text,
            &item.reason,
        ]
        .iter()
        .any(|value| value.as_ref().is_some_and(|value| value.contains('\0')))
    });
    match unstorable {
        true => Err(IntakeRefusal::UnstorableText),
        false => Ok(operations),
    }
}

/// One current entry of the pass, at the Memory revision that the pass pinned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassMemoryEntry {
    pub entry: CurrentMemoryEntry,
    pub origin: MemoryEntryOrigin,
}

/// Why the Host refused one operation. A refused operation does not stop the other operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryOperationRefusal {
    /// An update or a delete names an entry whose origin is the request of the author.
    AuthorEntry,
    /// A second operation of the list names the same entry. An update or a delete names its entry
    /// also when the Host refuses it.
    EntryNamedTwice,
    /// An operation names an observation that is not in the input of the pass.
    ObservationUnavailable,
    /// A create names no observation.
    CreateWithoutSource,
    /// The write rule of S7-D1a2.
    Write(MemoryWriteRefusal),
}

impl MemoryOperationRefusal {
    /// The stable label of the refusal, for the pass row.
    pub fn label(self) -> &'static str {
        match self {
            Self::AuthorEntry => "author_entry",
            Self::EntryNamedTwice => "entry_named_twice",
            Self::ObservationUnavailable => "observation_unavailable",
            Self::CreateWithoutSource => "create_without_source",
            Self::Write(refusal) => refusal.label(),
        }
    }
}

/// One operation that Core admitted: the version to write, the origin of the entry, and the
/// sources of the observations that it uses, each one time. The store adds these sources to the
/// sources of the earlier version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedOperation {
    pub version: MemoryVersionWrite,
    pub origin: MemoryEntryOrigin,
    pub sources: Vec<MemorySourceReference>,
}

/// The write of `candidate`, or the refusal of its shape. Create needs a type, update needs no
/// type, and delete needs a reason and no values.
fn memory_write(candidate: &MemoryOperationCandidate) -> Result<MemoryWrite, MemoryWriteRefusal> {
    let action = match candidate.action {
        MemoryOperationAction::Create => MemoryWriteAction::Create,
        MemoryOperationAction::Update => MemoryWriteAction::Change,
        MemoryOperationAction::Delete => MemoryWriteAction::Delete,
    };
    let given = [&candidate.title, &candidate.description, &candidate.text];
    let shape = match candidate.action {
        MemoryOperationAction::Create => true,
        MemoryOperationAction::Update => {
            candidate.entry_type.is_none() && given.iter().all(|value| value.is_some())
        }
        MemoryOperationAction::Delete => {
            candidate.entry_type.is_none()
                && candidate.reason.as_deref().is_some_and(|reason| {
                    !reason.trim().is_empty() && !reason.chars().any(char::is_control)
                })
        }
    };
    let entry_type = match candidate.entry_type.as_deref() {
        Some(token) => Some(MemoryEntryType::parse(token).ok_or(MemoryWriteRefusal::Invalid)?),
        None => None,
    };
    match shape {
        true => Ok(MemoryWrite {
            action,
            entry: candidate.entry,
            entry_type,
            title: candidate.title.clone(),
            description: candidate.description.clone(),
            text: candidate.text.clone(),
        }),
        false => Err(MemoryWriteRefusal::Invalid),
    }
}

/// Validates each operation of one call in list order (ADR 0039, ADR 0094 sections 6 and 8).
/// `entries` are the current entries at the pinned Memory revision, and `observations` are the
/// observations of the input, from number 1. Each admitted operation counts for the
/// operations after it, and a refused operation does not stop them.
pub fn decide_memory_operations(
    candidates: &[MemoryOperationCandidate],
    entries: &[PassMemoryEntry],
    next_entry_number: u64,
    observations: &[PassObservation],
) -> Vec<Result<AdmittedOperation, MemoryOperationRefusal>> {
    let mut current = entries.to_vec();
    let mut next_entry_number = next_entry_number;
    let mut named = Vec::new();
    let mut decisions = Vec::new();
    for candidate in candidates {
        let decision = decide_one(candidate, &current, next_entry_number, observations, &named);
        // An update or a delete names its entry also when the Host refuses it.
        match (&decision, candidate.action) {
            (Ok(admitted), MemoryOperationAction::Create) => named.push(admitted.version.number),
            (Err(_), MemoryOperationAction::Create) => {}
            (_, MemoryOperationAction::Update | MemoryOperationAction::Delete) => {
                named.extend(
                    candidate
                        .entry
                        .and_then(|number| u64::try_from(number).ok()),
                );
            }
        }
        if let Ok(admitted) = &decision {
            let number = admitted.version.number;
            current.retain(|entry| entry.entry.number != number);
            if admitted.version.action == MemoryWriteAction::Create {
                next_entry_number += 1;
            }
            if admitted.version.action != MemoryWriteAction::Delete {
                current.push(PassMemoryEntry {
                    entry: CurrentMemoryEntry {
                        memory_entry_id: admitted
                            .version
                            .memory_entry_id
                            .clone()
                            .unwrap_or_default(),
                        number,
                        version: admitted.version.version,
                        entry_type: admitted.version.entry_type,
                        title: admitted.version.title.clone(),
                        description: admitted.version.description.clone(),
                        text: admitted.version.text.clone(),
                    },
                    origin: admitted.origin,
                });
            }
        }
        decisions.push(decision);
    }
    decisions
}

/// The checks of one operation. The checks of the list come before the write rule of S7-D1a2.
fn decide_one(
    candidate: &MemoryOperationCandidate,
    current: &[PassMemoryEntry],
    next_entry_number: u64,
    observations: &[PassObservation],
    named: &[u64],
) -> Result<AdmittedOperation, MemoryOperationRefusal> {
    let create = candidate.action == MemoryOperationAction::Create;
    let origin_of = |number: &i64| {
        usize::try_from(*number)
            .ok()
            .and_then(|number| number.checked_sub(1))
            .and_then(|index| observations.get(index))
    };
    let first_origin = candidate
        .observations
        .first()
        .and_then(origin_of)
        .map(PassObservation::origin);
    if !candidate
        .observations
        .iter()
        .all(|number| origin_of(number).is_some())
    {
        return Err(MemoryOperationRefusal::ObservationUnavailable);
    }
    let target = candidate
        .entry
        .and_then(|number| u64::try_from(number).ok());
    if !create && target.is_some_and(|number| named.contains(&number)) {
        return Err(MemoryOperationRefusal::EntryNamedTwice);
    }
    let named_entry = target.and_then(|number| current.iter().find(|e| e.entry.number == number));
    let origin = match (create, named_entry) {
        (true, _) => first_origin.ok_or(MemoryOperationRefusal::CreateWithoutSource)?,
        (false, None) => {
            return Err(MemoryOperationRefusal::Write(MemoryWriteRefusal::NotFound));
        }
        (false, Some(entry)) if entry.origin == MemoryEntryOrigin::AuthorRequest => {
            return Err(MemoryOperationRefusal::AuthorEntry);
        }
        (false, Some(entry)) => entry.origin,
    };
    let write = memory_write(candidate).map_err(MemoryOperationRefusal::Write)?;
    let entries: Vec<CurrentMemoryEntry> = current.iter().map(|e| e.entry.clone()).collect();
    let version = crate::decide_memory_write(&write, &entries, next_entry_number)
        .map_err(MemoryOperationRefusal::Write)?;
    let mut sources = Vec::new();
    for source in candidate.observations.iter().filter_map(origin_of) {
        if !sources.contains(&source.source) {
            sources.push(source.source.clone());
        }
    }
    Ok(AdmittedOperation {
        version,
        origin,
        sources,
    })
}

/// The facts of the settlement of one pass, which the store loads after the lock of the Memory
/// head.
#[derive(Clone, Copy, Debug)]
pub struct PassFacts<'a> {
    /// The Memory revision that the pass pinned at its start.
    pub pinned_revision: u64,
    /// The Memory revision of the Project now.
    pub memory_revision: u64,
    /// The current entries of the Project now.
    pub entries: &'a [PassMemoryEntry],
    pub next_entry_number: u64,
    /// The observations of the input, from number 1.
    pub observations: &'a [PassObservation],
}

/// The settlement of the one `apply_memory_operations` call of a pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassSettlement {
    pub outcome: ConsolidationOutcome,
    /// The decision of each operation in list order. A stale pass decides no operation.
    pub decisions: Vec<Result<AdmittedOperation, MemoryOperationRefusal>>,
    /// The result of the call. The pass ends with it, so no request reads it.
    pub result: ToolResultText,
}

/// Settles the call of a pass (ADR 0039, ADR 0094 section 6). When the Memory revision now is not
/// the pinned revision, the input of the pass is older than the entries, and the pass applies
/// nothing. Otherwise Core decides each operation, and the outcome is `applied` when one or more
/// operations applied.
pub fn settle_operations_call(
    candidates: &[MemoryOperationCandidate],
    facts: &PassFacts<'_>,
) -> PassSettlement {
    if facts.memory_revision != facts.pinned_revision {
        return PassSettlement {
            outcome: ConsolidationOutcome::Stale,
            decisions: Vec::new(),
            result: crate::host_tool_result(
                "The Memory changed after the start of this pass, so the Host applied no \
                 operation, and the pass is complete.",
            ),
        };
    }
    let decisions = decide_memory_operations(
        candidates,
        facts.entries,
        facts.next_entry_number,
        facts.observations,
    );
    let applied = decisions.iter().filter(|decision| decision.is_ok()).count();
    let refused = decisions.len() - applied;
    PassSettlement {
        outcome: match applied {
            0 => ConsolidationOutcome::NoOperation,
            _ => ConsolidationOutcome::Applied,
        },
        decisions,
        result: crate::host_tool_result(&format!(
            "The Host applied {applied} operations and refused {refused}, and the pass is \
             complete."
        )),
    }
}

#[cfg(test)]
#[path = "apply_memory_operations_tests.rs"]
mod tests;
