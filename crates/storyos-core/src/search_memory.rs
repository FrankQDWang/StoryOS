//! The Host Tool `search_memory` (ADR 0094 section 4): the Agent finds the current Memory Entries
//! of the Project that hold an exact term in the title, the description, or the text. The search
//! uses the one exact match of the author search of the manuscript, and no embedding and no
//! index. The result has the form of the Memory Index with no text of an entry, in the bounds of
//! one Context item. The call has no external effect and gives the Agent no target.

use serde_json::json;

use crate::read_chapter::bounded_result;
use crate::{
    CurrentMemoryEntry, TOOL_RESULT_TEXT_MAX_BYTES, ToolOperation, ToolResultText, ToolSpec,
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
    // A memory search result has no head line. The cost of an empty text is 0.
    bounded_result(&parts, |_| String::new(), more, call_id)
}

#[cfg(test)]
#[path = "search_memory_tests.rs"]
mod tests;
