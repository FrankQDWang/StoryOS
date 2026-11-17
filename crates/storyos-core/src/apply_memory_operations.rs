//! The Core part of the function `apply_memory_operations` (ADR 0094 sections 6 and 8): the output
//! of a consolidation pass. Its one call gives the typed list of the operations of the pass. One
//! Core function validates each operation in list order against the entries and the observations
//! of the pass, and gives the value that the store applies in the transaction that settles the
//! call.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    ConsolidationOutcome, CurrentMemoryEntry, IntakeRefusal, MemoryDeleteMark, MemoryEntryOrigin,
    MemoryEntryType, MemorySourceReference, MemoryVersionWrite, MemoryWriteRefusal,
    PassObservation, ToolIntake, ToolOperation, ToolResultText, ToolSpec, decide_memory_operations,
};

pub const APPLY_MEMORY_OPERATIONS_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.apply-memory-operations.v1",
    operation: ToolOperation::ApplyMemoryOperations,
    intake_contract: &[],
    envelope: None,
    result_text_max_bytes: 512,
};

pub(crate) const DESCRIPTION: &str = "Applies the operations of this consolidation pass to the \
Memory of the Project. Call it one time. Create an entry from one or more observations. Update an \
entry with what the observations add. Merge two or more entries of one type into the first of \
them. Delete an entry that is not true now. Each entry and each observation has its number in \
the input. Give an empty list when the Memory needs no change.";

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
