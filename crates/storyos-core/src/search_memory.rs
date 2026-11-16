//! The Host Tool `search_memory` (ADR 0094 section 4): the Agent finds the current Memory Entries
//! of the Project that hold an exact term in the title, the description, or the text. The search
//! uses the one exact match of the author search of the manuscript, and no embedding and no
//! index. The result has the form of the Memory Index with no text of an entry, in the bounds of
//! one Context item. The call has no external effect and gives the Agent no target.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::read_chapter::{kept_prefix, text_cost};
use crate::{
    CurrentMemoryEntry, IntakeRefusal, TOOL_RESULT_TEXT_MAX_BYTES, ToolIntake, ToolOperation,
    ToolResultText, ToolSpec,
};

pub const SEARCH_MEMORY_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.search-memory.v1",
    operation: ToolOperation::SearchMemory,
    intake_contract: &[],
    envelope: None,
    // The Context item of a result is at or below the item limit (ADR 0052).
    result_text_max_bytes: TOOL_RESULT_TEXT_MAX_BYTES,
};

pub(crate) const DESCRIPTION: &str = "Finds the entries of the Memory of this Project that hold \
an exact word or phrase in the title, the description, or the text. The result lists each entry \
that matches, with no text. Then read an entry with read_memory_entry.";

const NO_MATCH: &str = "No current entry of the Memory has this exact text, and this is not a \
failure.";

/// The strict JSON Schema of the arguments.
pub(crate) fn parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "The exact text to find, with the same letter case. It is not a \
                                question."
            }
        },
        "required": ["query"],
        "additionalProperties": false
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    query: String,
}

/// The typed intake: the query rule of `web_search`. An empty query, a query above 512
/// characters, and a query with a control character do not fit.
pub(crate) fn intake(arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
    let Arguments { query } =
        Arguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
    match crate::tool_spec::query_fits(&query) {
        true => Ok(ToolIntake::SearchMemory { query }),
        false => Err(IntakeRefusal::Query),
    }
}

/// The result of a search of `query` in the current entries `entries`, for the native call
/// `call_id`. Each entry whose title, description, or text holds `query` as an exact sequence of
/// characters is one line, in the order of the Memory Index: each type that has a match starts a
/// group with the line `{"type": type}`, and each entry is the line `{"description", "id",
/// "matched", "title"}`, in the order of the numbers. `matched` names the fields that hold the
/// term. The result keeps each entry while the `tool_result` item of the call, with its boundary
/// and its escape, fits in the item limit. The first entry that does not fit ends the result with
/// one line. A search with no match gives one sentence.
pub fn search_memory(entries: &[CurrentMemoryEntry], query: &str, call_id: &str) -> ToolResultText {
    let mut hits: Vec<(&CurrentMemoryEntry, Vec<&str>)> = entries
        .iter()
        .filter_map(|entry| {
            let fields = [
                ("title", &entry.title),
                ("description", &entry.description),
                ("text", &entry.text),
            ];
            let matched: Vec<&str> = (fields.into_iter())
                .filter(|(_, value)| crate::exact_matches(value, query).next().is_some())
                .map(|(name, _)| name)
                .collect();
            (!matched.is_empty()).then_some((entry, matched))
        })
        .collect();
    if hits.is_empty() {
        return crate::host_tool_result(NO_MATCH);
    }
    hits.sort_by_key(|(entry, _)| (entry.entry_type, entry.number));
    // Each part is the line of one entry, after the line of its type when it starts a group.
    let parts: Vec<String> = (hits.iter().enumerate())
        .map(|(index, (entry, matched))| {
            let mut part = String::new();
            if index > 0 {
                part.push('\n');
            }
            if index == 0 || hits[index - 1].0.entry_type != entry.entry_type {
                part.push_str(&crate::canonical_json(
                    &json!({ "type": entry.entry_type.as_str() }),
                ));
                part.push('\n');
            }
            part.push_str(&crate::canonical_json(&json!({
                "id": entry.number,
                "title": entry.title,
                "description": entry.description,
                "matched": matched,
            })));
            part
        })
        .collect();
    let count = parts.len();
    let more = |kept: usize| {
        format!(
            "\n[This result does not have the other {} entries with a match. Search for a more \
             exact term to find them.]",
            count - kept
        )
    };
    let costs: Vec<u64> = parts.iter().map(|part| text_cost(part)).collect();
    let kept = kept_prefix(
        &costs,
        |kept| match kept < count {
            true => text_cost(&more(kept)),
            false => 0,
        },
        call_id,
    );
    let tail = match kept < count {
        true => more(kept),
        false => String::new(),
    };
    crate::host_tool_result(&format!("{}{tail}", parts[..kept].concat()))
}

#[cfg(test)]
#[path = "search_memory_tests.rs"]
mod tests;
