//! The Host Tool `write_memory` (ADR 0094 section 5, writer 1): on the request of the author, the
//! Agent creates, changes, or deletes one Memory Entry. One function decides each write from the
//! arguments and from the current entries that the store loads, and the store writes the version
//! in the transaction that settles the ToolCall.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    IntakeRefusal, MemoryEntryState, MemoryEntryType, MemoryIndexEntry, ProjectionMode, ToolIntake,
    ToolOperation, ToolResultText, ToolSpec, memory_index_text,
};

pub const WRITE_MEMORY_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.write-memory.v1",
    operation: ToolOperation::WriteMemory,
    intake_contract: &[],
    envelope: None,
    result_text_max_bytes: 1024,
};

/// The Host limit of the text of an entry (initial value, S7-D7 can change it).
pub const MEMORY_TEXT_MAX_CHARS: usize = 600;
/// The Host limit of the text of an entry of the type `character` (initial value).
pub const MEMORY_CHARACTER_TEXT_MAX_CHARS: usize = 1500;
/// The Host limit of the title of an entry (initial value).
pub const MEMORY_TITLE_MAX_CHARS: usize = 30;
/// The Host limit of the description of an entry (initial value).
pub const MEMORY_DESCRIPTION_MAX_CHARS: usize = 100;

pub(crate) const DESCRIPTION: &str = "Creates, changes, or deletes one entry of the Memory of \
this Project. Call it only when the author asks you to remember something, to change what you \
remember, or to forget something. An entry is a short synthesis or an intent, not a copy of a \
passage. The title names the subject of the entry and holds no fact: for a character, the title is \
only the name of the character. A fact goes into the description and the text. For a change or a \
delete, give the id of an entry of the section Memory index.";

/// The strict JSON Schema of the arguments. Each field is required, and a field that the action
/// does not use takes null.
pub(crate) fn parameters() -> Value {
    let value =
        |description: &str| json!({ "type": ["string", "null"], "description": description });
    let types: Vec<Value> = MemoryEntryType::ALL
        .iter()
        .map(|kind| json!(kind.as_str()))
        .chain([Value::Null])
        .collect();
    json!({
        "type": "object",
        "properties": {
            "action": { "type": "string", "enum": ["create", "change", "delete"] },
            "entry": {
                "type": ["integer", "null"],
                "description": "The id of an entry of the section Memory index. Null for create."
            },
            "type": {
                "type": ["string", "null"],
                "enum": types,
                "description": "The type of the entry. Null keeps the type of the entry."
            },
            "title": value("One line, at most 30 characters. Null keeps the title."),
            "description": value("One line, at most 100 characters. Null keeps the description."),
            "text": value(
                "Markdown, at most 600 characters, or 1500 for a character. Null keeps the text."
            )
        },
        "required": KEYS,
        "additionalProperties": false
    })
}

/// The action of one `write_memory` call.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryWriteAction {
    Create,
    Change,
    Delete,
}

impl MemoryWriteAction {
    /// The action that wrote one stored version: a deleted state is a delete, the first version of
    /// an entry is a create, and each other version is a change.
    pub fn of_version(state: MemoryEntryState, first_version: bool) -> Self {
        match (state, first_version) {
            (MemoryEntryState::Deleted, _) => Self::Delete,
            (MemoryEntryState::Current, true) => Self::Create,
            (MemoryEntryState::Current, false) => Self::Change,
        }
    }
}

/// The validated arguments of one `write_memory` call. A null value is `None`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryWrite {
    pub action: MemoryWriteAction,
    pub entry: Option<i64>,
    pub entry_type: Option<MemoryEntryType>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub text: Option<String>,
}

impl MemoryWrite {
    pub(crate) fn arguments(&self) -> Value {
        json!({
            "action": self.action,
            "entry": self.entry,
            "type": self.entry_type.map(MemoryEntryType::as_str),
            "title": self.title,
            "description": self.description,
            "text": self.text,
        })
    }
}

/// The keys of the arguments. The strict schema requires each one, also when its value is null.
const KEYS: [&str; 6] = ["action", "entry", "type", "title", "description", "text"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    action: MemoryWriteAction,
    entry: Option<i64>,
    #[serde(rename = "type")]
    entry_type: Option<String>,
    title: Option<String>,
    description: Option<String>,
    text: Option<String>,
}

/// The typed intake. The ToolCall record cannot hold U+0000, so a value with it does not fit.
pub(crate) fn intake(arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
    let object = arguments.as_object().ok_or(IntakeRefusal::Malformed)?;
    if !KEYS.iter().all(|key| object.contains_key(*key)) {
        return Err(IntakeRefusal::Malformed);
    }
    let arguments = Arguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
    let entry_type = match arguments.entry_type.as_deref() {
        Some(token) => Some(MemoryEntryType::parse(token).ok_or(IntakeRefusal::Malformed)?),
        None => None,
    };
    let values = [&arguments.title, &arguments.description, &arguments.text];
    if values
        .iter()
        .any(|value| value.as_ref().is_some_and(|value| value.contains('\0')))
    {
        return Err(IntakeRefusal::UnstorableText);
    }
    Ok(ToolIntake::WriteMemory(MemoryWrite {
        action: arguments.action,
        entry: arguments.entry,
        entry_type,
        title: arguments.title,
        description: arguments.description,
        text: arguments.text,
    }))
}

/// One current Memory Entry of the Project at the time of the call: its last version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentMemoryEntry {
    pub memory_entry_id: String,
    pub number: u64,
    pub version: u64,
    pub entry_type: MemoryEntryType,
    pub title: String,
    pub description: String,
    pub text: String,
}

/// The version that one admitted write adds. `memory_entry_id` is `None` for a new entry. Only a
/// flag version (ADR 0094 section 7) has a `flag`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryVersionWrite {
    pub action: MemoryWriteAction,
    pub memory_entry_id: Option<String>,
    pub number: u64,
    pub version: u64,
    pub state: MemoryEntryState,
    pub entry_type: MemoryEntryType,
    pub title: String,
    pub description: String,
    pub text: String,
    pub flag: Option<crate::MemoryConflictFlag>,
}

impl MemoryVersionWrite {
    /// The result of the settled call: one sentence with the number and the title of the entry.
    pub fn result(&self) -> ToolResultText {
        let verb = match self.action {
            MemoryWriteAction::Create => "created",
            MemoryWriteAction::Change => "changed",
            MemoryWriteAction::Delete => "deleted",
        };
        crate::host_tool_result(&format!(
            "The Host {verb} the Memory entry {} with the title {}.",
            self.number,
            json!(self.title)
        ))
    }
}

/// Why the Host refused one `write_memory` call. A refused call writes no row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryWriteRefusal {
    Invalid,
    TooLong,
    NotFound,
    /// Another current entry has the type and the title. It has this number.
    Exists(u64),
    IndexFull,
}

impl MemoryWriteRefusal {
    /// The stable label of the refusal, for the `refusal` of the ToolCall row.
    pub fn label(self) -> &'static str {
        match self {
            Self::Invalid => "memory_entry_invalid",
            Self::TooLong => "memory_entry_too_long",
            Self::NotFound => "memory_entry_not_found",
            Self::Exists(_) => "memory_entry_exists",
            Self::IndexFull => "memory_index_full",
        }
    }

    /// The result of the refused call: the reason in one sentence.
    pub fn result(self) -> ToolResultText {
        let reason = match self {
            Self::Invalid => "the values do not fit the action: create needs the type, the \
                title, the description, and the text, change needs one or more of them, delete \
                needs none, and a title and a description are one line that is not empty"
                .to_owned(),
            Self::TooLong => format!(
                "a value is too long: a title has at most {MEMORY_TITLE_MAX_CHARS} characters, a \
                 description at most {MEMORY_DESCRIPTION_MAX_CHARS}, and a text at most \
                 {MEMORY_TEXT_MAX_CHARS}, or {MEMORY_CHARACTER_TEXT_MAX_CHARS} for a character"
            ),
            Self::NotFound => {
                "no current entry has this id: use an id of the section Memory index".to_owned()
            }
            Self::Exists(number) => format!(
                "the entry {number} already has this type and this title: change that entry"
            ),
            Self::IndexFull => "Memory is full, so tell the author that Memory is full".to_owned(),
        };
        crate::host_tool_result(&format!(
            "The Host refused this call because {reason}, so Memory did not change."
        ))
    }
}

/// Decides one write from `write` and the current entries of the Project, and returns the version
/// to write or the refusal. The checks are in the order of the refusals of S7-D1a2.
pub fn decide_memory_write(
    write: &MemoryWrite,
    entries: &[CurrentMemoryEntry],
    next_entry_number: u64,
) -> Result<MemoryVersionWrite, MemoryWriteRefusal> {
    let given = [&write.title, &write.description, &write.text];
    let shape = match write.action {
        MemoryWriteAction::Create => {
            write.entry.is_none() && write.entry_type.is_some() && given.iter().all(|v| v.is_some())
        }
        MemoryWriteAction::Change => {
            write.entry.is_some()
                && (write.entry_type.is_some() || given.iter().any(|value| value.is_some()))
        }
        MemoryWriteAction::Delete => {
            write.entry.is_some()
                && write.entry_type.is_none()
                && given.iter().all(|value| value.is_none())
        }
    };
    let line = |value: &Option<String>| {
        value
            .as_deref()
            .is_none_or(|value| !value.trim().is_empty() && !value.chars().any(char::is_control))
    };
    let text = write.text.as_deref().is_none_or(|text| {
        !text.trim().is_empty()
            && !text
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
    });
    if !(shape && line(&write.title) && line(&write.description) && text) {
        return Err(MemoryWriteRefusal::Invalid);
    }
    let named = write.entry.and_then(|number| {
        entries
            .iter()
            .find(|entry| i64::try_from(entry.number).is_ok_and(|current| current == number))
    });
    let entry_type = write.entry_type.or(named.map(|entry| entry.entry_type));
    let title = write.title.as_deref().map(str::trim);
    let description = write.description.as_deref().map(str::trim);
    // A change of the type checks the text that the entry keeps.
    let checked_text = write
        .text
        .as_deref()
        .or(write.entry_type.and(named).map(|entry| entry.text.as_str()));
    let text_limit = match entry_type {
        Some(MemoryEntryType::Character) | None => MEMORY_CHARACTER_TEXT_MAX_CHARS,
        Some(_) => MEMORY_TEXT_MAX_CHARS,
    };
    let longer =
        |value: Option<&str>, limit: usize| value.is_some_and(|v| v.chars().count() > limit);
    if longer(title, MEMORY_TITLE_MAX_CHARS)
        || longer(description, MEMORY_DESCRIPTION_MAX_CHARS)
        || longer(checked_text, text_limit)
    {
        return Err(MemoryWriteRefusal::TooLong);
    }
    let version = match (write.action, named) {
        (MemoryWriteAction::Create, _) => MemoryVersionWrite {
            action: write.action,
            memory_entry_id: None,
            number: next_entry_number,
            version: 1,
            state: MemoryEntryState::Current,
            entry_type: write.entry_type.unwrap_or(MemoryEntryType::Other),
            title: title.unwrap_or_default().to_owned(),
            description: description.unwrap_or_default().to_owned(),
            text: write.text.clone().unwrap_or_default(),
            flag: None,
        },
        (_, None) => return Err(MemoryWriteRefusal::NotFound),
        (action, Some(entry)) => MemoryVersionWrite {
            action,
            memory_entry_id: Some(entry.memory_entry_id.clone()),
            number: entry.number,
            version: entry.version + 1,
            state: match action {
                MemoryWriteAction::Delete => MemoryEntryState::Deleted,
                MemoryWriteAction::Create | MemoryWriteAction::Change => MemoryEntryState::Current,
            },
            entry_type: entry_type.unwrap_or(entry.entry_type),
            title: title.unwrap_or(&entry.title).to_owned(),
            description: description.unwrap_or(&entry.description).to_owned(),
            text: match action {
                MemoryWriteAction::Delete => String::new(),
                MemoryWriteAction::Create | MemoryWriteAction::Change => {
                    write.text.clone().unwrap_or_else(|| entry.text.clone())
                }
            },
            flag: None,
        },
    };
    if version.state == MemoryEntryState::Deleted {
        return Ok(version);
    }
    let others = entries
        .iter()
        .filter(|entry| entry.number != version.number);
    if (write.action == MemoryWriteAction::Create
        || write.title.is_some()
        || write.entry_type.is_some())
        && let Some(other) = others.clone().find(|entry| {
            entry.entry_type == version.entry_type && entry.title.trim() == version.title
        })
    {
        return Err(MemoryWriteRefusal::Exists(other.number));
    }
    let index: Vec<MemoryIndexEntry> = others
        .map(|entry| MemoryIndexEntry {
            number: entry.number,
            entry_type: entry.entry_type,
            title: entry.title.clone(),
            description: entry.description.clone(),
        })
        .chain([MemoryIndexEntry {
            number: version.number,
            entry_type: version.entry_type,
            title: version.title.clone(),
            description: version.description.clone(),
        }])
        .collect();
    match memory_index_text(&index).1 {
        ProjectionMode::ExactRequired => Ok(version),
        _ => Err(MemoryWriteRefusal::IndexFull),
    }
}

#[cfg(test)]
#[path = "write_memory_tests.rs"]
mod tests;
