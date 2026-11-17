//! The Core part of the function `apply_memory_operations` (ADR 0094 sections 6 and 8): the output
//! of a consolidation pass. Its one call gives the typed list of the operations of the pass. One
//! Core function validates each operation in list order against the entries and the observations
//! of the pass, and gives the value that the store applies in the transaction that settles the
//! call.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    ConsolidationOutcome, CurrentMemoryEntry, IntakeRefusal, MemoryDeleteMark, MemoryEntryOrigin,
    MemoryEntryType, MemorySourceReference, MemoryVersionWrite, MemoryWrite, MemoryWriteAction,
    MemoryWriteRefusal, PassObservation, ToolIntake, ToolOperation, ToolResultText, ToolSpec,
};

pub const APPLY_MEMORY_OPERATIONS_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.apply-memory-operations.v1",
    operation: ToolOperation::ApplyMemoryOperations,
    intake_contract: &[],
    envelope: None,
    result_text_max_bytes: 512,
};

pub(crate) const DESCRIPTION: &str = "Applies the operations of this consolidation pass to the \
Memory of the Project. Call it one time. Create an entry from one or more observations, update an \
entry with what the observations add, merge two or more entries of one type into the first of \
them, and delete an entry that is not true now. Each entry and each observation has its number \
in the input. Give an empty list when the Memory needs no \
change.";

/// The keys of an operation. The strict schema requires each one, also when its value is null.
const KEYS: [&str; 9] = [
    "action",
    "entry",
    "entries",
    "type",
    "title",
    "description",
    "text",
    "observations",
    "reason",
];

/// The strict JSON Schema of the arguments. The description of `type` names the types, and no enum
/// does. Thus the Host refuses only the operation with an unknown type (`memory_entry_invalid`).
pub fn parameters() -> Value {
    let types: Vec<&str> = MemoryEntryType::ALL.iter().map(|t| t.as_str()).collect();
    let types = format!(
        "The type of a new entry: one of {}. Null for update, merge, and delete.",
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
                        "action": {
                            "type": "string",
                            "enum": ["create", "update", "merge", "delete"]
                        },
                        "entry": {
                            "type": ["integer", "null"],
                            "description": "The number of the entry to update or delete. Null \
                                            for create and merge."
                        },
                        "entries": {
                            "type": ["array", "null"],
                            "items": { "type": "integer" },
                            "description": "The numbers of two or more entries to merge. The \
                                            first entry gets the result. Null for the other \
                                            actions."
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
    Merge,
    Delete,
}

/// One operation as the model gave it. The type is the given text: the validation refuses a type
/// that is not one of the six types.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MemoryOperationCandidate {
    pub action: MemoryOperationAction,
    pub entry: Option<i64>,
    pub entries: Option<Vec<i64>>,
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
            "entries": self.entries,
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

/// The typed intake of a call of the Tool.
pub(crate) fn tool_intake(arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
    intake(arguments).map(ToolIntake::ApplyMemoryOperations)
}

/// The arguments of a validated call, as the ToolCall record keeps them.
pub(crate) fn arguments(operations: &[MemoryOperationCandidate]) -> Value {
    let items: Vec<Value> = operations
        .iter()
        .map(MemoryOperationCandidate::arguments)
        .collect();
    json!({ "operations": items })
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
    /// The entries of a merge do not have the same type.
    MergeTypeMismatch,
    /// A create, an update, or a merge gives an entry the type and the title of an entry that the
    /// author deleted.
    DeletedByAuthor,
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
            Self::MergeTypeMismatch => "merge_type_mismatch",
            Self::DeletedByAuthor => "deleted_by_author",
            Self::Write(refusal) => refusal.label(),
        }
    }
}

/// One operation that Core admitted: the version to write, the origin of the entry, and the
/// sources of the observations that it uses, each one time. The store adds these sources to the
/// sources of the earlier version. A merge also gives the `deleted` version of each other entry
/// that it merged, and the store adds the sources of these entries to the result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedOperation {
    pub version: MemoryVersionWrite,
    pub origin: MemoryEntryOrigin,
    pub sources: Vec<MemorySourceReference>,
    pub merged: Vec<MemoryVersionWrite>,
}

/// The write of `candidate`, or the refusal of its shape. Create needs a type, update and merge
/// need no type, and delete needs a reason and no values. Only a merge has entries, and the write
/// of a merge is the change of its first entry.
fn memory_write(candidate: &MemoryOperationCandidate) -> Result<MemoryWrite, MemoryWriteRefusal> {
    let action = match candidate.action {
        MemoryOperationAction::Create => MemoryWriteAction::Create,
        MemoryOperationAction::Update | MemoryOperationAction::Merge => MemoryWriteAction::Change,
        MemoryOperationAction::Delete => MemoryWriteAction::Delete,
    };
    let given = [&candidate.title, &candidate.description, &candidate.text];
    let merge = candidate.action == MemoryOperationAction::Merge;
    let shape = candidate.entries.is_some() == merge
        && match candidate.action {
            MemoryOperationAction::Create => true,
            MemoryOperationAction::Update => {
                candidate.entry_type.is_none() && given.iter().all(|value| value.is_some())
            }
            MemoryOperationAction::Merge => {
                candidate.entry.is_none()
                    && candidate.entry_type.is_none()
                    && candidate.reason.is_none()
                    && given.iter().all(|value| value.is_some())
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
            entry: match merge {
                true => candidate
                    .entries
                    .as_ref()
                    .and_then(|entries| entries.first().copied()),
                false => candidate.entry,
            },
            entry_type,
            title: candidate.title.clone(),
            description: candidate.description.clone(),
            text: candidate.text.clone(),
        }),
        false => Err(MemoryWriteRefusal::Invalid),
    }
}

/// The numbers of the entries that `candidate` names: the entry of an update or a delete, and
/// each entry of a merge. A create names no entry.
fn named_entries(candidate: &MemoryOperationCandidate) -> Vec<u64> {
    let numbers = match candidate.action {
        MemoryOperationAction::Create => &[][..],
        MemoryOperationAction::Update | MemoryOperationAction::Delete => candidate.entry.as_slice(),
        MemoryOperationAction::Merge => candidate.entries.as_deref().unwrap_or_default(),
    };
    numbers
        .iter()
        .filter_map(|number| u64::try_from(*number).ok())
        .collect()
}

/// Validates each operation of one call in list order (ADR 0039, ADR 0094 sections 6 and 8).
/// `entries` are the current entries at the pinned Memory revision, `observations` are the
/// observations of the input, from number 1, and `marks` are the delete marks of the author. Each
/// admitted operation counts for the operations after it, and a refused operation does not stop
/// them.
pub fn decide_memory_operations(
    candidates: &[MemoryOperationCandidate],
    entries: &[PassMemoryEntry],
    next_entry_number: u64,
    observations: &[PassObservation],
    marks: &[MemoryDeleteMark],
) -> Vec<Result<AdmittedOperation, MemoryOperationRefusal>> {
    let mut current = entries.to_vec();
    let mut next_entry_number = next_entry_number;
    let mut named = Vec::new();
    let mut decisions = Vec::new();
    for candidate in candidates {
        let facts = (current.as_slice(), next_entry_number, observations, marks);
        let decision = decide_one(candidate, facts, &named);
        // An update, a merge, or a delete names its entries also when the Host refuses it.
        match (&decision, candidate.action) {
            (Ok(admitted), MemoryOperationAction::Create) => named.push(admitted.version.number),
            (Err(_), MemoryOperationAction::Create) => {}
            (_, _) => named.extend(named_entries(candidate)),
        }
        if let Ok(admitted) = &decision {
            let number = admitted.version.number;
            current.retain(|entry| {
                entry.entry.number != number
                    && !admitted
                        .merged
                        .iter()
                        .any(|m| m.number == entry.entry.number)
            });
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

/// The facts of one decision: the current entries, the next entry number, the observations of
/// the input, and the delete marks.
type DecisionFacts<'a> = (
    &'a [PassMemoryEntry],
    u64,
    &'a [PassObservation],
    &'a [MemoryDeleteMark],
);

/// The checks of one operation. The checks of the list come before the write rule of S7-D1a2, and
/// the check of the delete marks comes after it.
fn decide_one(
    candidate: &MemoryOperationCandidate,
    (current, next_entry_number, observations, marks): DecisionFacts<'_>,
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
    let targets = named_entries(candidate);
    if targets.iter().any(|number| named.contains(number)) {
        return Err(MemoryOperationRefusal::EntryNamedTwice);
    }
    // A merge has two or more entries, and each one time.
    let merge = candidate.action == MemoryOperationAction::Merge;
    let distinct = targets
        .iter()
        .enumerate()
        .all(|(index, number)| !targets[..index].contains(number));
    if merge && (targets.len() < 2 || !distinct) {
        return Err(MemoryOperationRefusal::Write(MemoryWriteRefusal::Invalid));
    }
    let found: Vec<&PassMemoryEntry> = targets
        .iter()
        .filter_map(|number| current.iter().find(|e| e.entry.number == *number))
        .collect();
    let origin = match (create, found.first()) {
        (true, _) => first_origin.ok_or(MemoryOperationRefusal::CreateWithoutSource)?,
        (false, None) => {
            return Err(MemoryOperationRefusal::Write(MemoryWriteRefusal::NotFound));
        }
        (false, Some(_)) if found.len() < targets.len() => {
            return Err(MemoryOperationRefusal::Write(MemoryWriteRefusal::NotFound));
        }
        (false, Some(_))
            if found
                .iter()
                .any(|entry| entry.origin == MemoryEntryOrigin::AuthorRequest) =>
        {
            return Err(MemoryOperationRefusal::AuthorEntry);
        }
        (false, Some(entry)) => entry.origin,
    };
    if found
        .iter()
        .any(|entry| entry.entry.entry_type != found[0].entry.entry_type)
    {
        return Err(MemoryOperationRefusal::MergeTypeMismatch);
    }
    let write = memory_write(candidate).map_err(MemoryOperationRefusal::Write)?;
    // The other entries of a merge are not in the checks of the result, so a merge makes room.
    let others = match merge {
        true => &targets[1..],
        false => &[][..],
    };
    let entries: Vec<CurrentMemoryEntry> = current
        .iter()
        .filter(|e| !others.contains(&e.entry.number))
        .map(|e| e.entry.clone())
        .collect();
    let version = crate::decide_memory_write(&write, &entries, next_entry_number)
        .map_err(MemoryOperationRefusal::Write)?;
    let marked = marks.iter().any(|mark| {
        mark.entry_type == version.entry_type && mark.title.trim() == version.title.trim()
    });
    if marked && version.action != MemoryWriteAction::Delete {
        return Err(MemoryOperationRefusal::DeletedByAuthor);
    }
    let mut merged = Vec::new();
    for entry in found.iter().filter(|e| others.contains(&e.entry.number)) {
        let delete = MemoryWrite {
            action: MemoryWriteAction::Delete,
            entry: i64::try_from(entry.entry.number).ok(),
            entry_type: None,
            title: None,
            description: None,
            text: None,
        };
        let entries = [entry.entry.clone()];
        merged.push(
            crate::decide_memory_write(&delete, &entries, next_entry_number)
                .map_err(MemoryOperationRefusal::Write)?,
        );
    }
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
        merged,
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
    /// The delete marks of the author now.
    pub marks: &'a [MemoryDeleteMark],
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

/// Settles the call of a pass (ADR 0039, ADR 0094 section 6). A pass whose pinned Memory revision
/// is not the revision now is stale, and it applies nothing. Otherwise Core decides each
/// operation. The outcome is `applied` when one or more operations applied.
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
        facts.marks,
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
