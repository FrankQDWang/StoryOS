//! The delete marks of a capture (ADR 0094 section 8): one `memory_delete_marks` Context item with
//! the type, the title, and the description of each entry that the author deleted. The marks are
//! examples of content that the author does not want, and they are data.

use crate::assemble_context::{
    CONTEXT_ITEM_TOKEN_LIMIT, ConsideredSource, ProjectionMode, SelectedProjection,
    count_context_item_tokens,
};
use crate::{ContextSourceClass, CurrentPassageAssemblyRecord, MemoryEntryType, canonical_json};

/// The cause kinds of a delete by the author: a request in a Conversation (`write_memory`) and the
/// delete control of the settings page. The mark of an entry is its last version when that
/// version has the state `deleted` and one of these causes.
pub const MEMORY_DELETE_MARK_CAUSES: [&str; 2] = ["author_request", "author_delete"];

/// The last version of one entry that the author deleted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryDeleteMark {
    pub entry_type: MemoryEntryType,
    pub title: String,
    pub description: String,
    /// The deleted entry was the part of the author of a character file (ADR 0101 section 3). Its
    /// mark does not stop the part of the background work of that character.
    pub author_part: bool,
}

/// The text of the `memory_delete_marks` item of `marks`, which come newest first, and its
/// projection mode. Each mark is the line `{"description", "title", "type"}`, and the mark of the
/// part of the author of a character file also has `"author_part": true`. When the marks are
/// above the Context item limit (ADR 0052), the text holds the newest marks that fit and ends
/// with the line `{"not_listed": count}`. Then the mode is `deterministic_excerpt`.
pub fn memory_delete_marks_text(marks: &[MemoryDeleteMark]) -> (String, ProjectionMode) {
    let lines: Vec<String> = marks
        .iter()
        .map(|mark| {
            let mut line = serde_json::json!({
                "description": mark.description,
                "title": mark.title,
                "type": mark.entry_type.as_str(),
            });
            if mark.author_part {
                line["author_part"] = serde_json::Value::Bool(true);
            }
            canonical_json(&line)
        })
        .collect();
    let complete = lines.join("\n");
    if count_context_item_tokens(&complete) <= CONTEXT_ITEM_TOKEN_LIMIT {
        return (complete, ProjectionMode::ExactRequired);
    }
    let excerpt = |listed: usize| {
        let count_line = canonical_json(&serde_json::json!({ "not_listed": lines.len() - listed }));
        match listed {
            0 => count_line,
            _ => format!("{}\n{count_line}", lines[..listed].join("\n")),
        }
    };
    // The longest prefix that fits. A longer prefix never counts fewer tokens.
    let (mut fits, mut above) = (0, lines.len());
    while above - fits > 1 {
        let middle = fits + (above - fits) / 2;
        match count_context_item_tokens(&excerpt(middle)) <= CONTEXT_ITEM_TOKEN_LIMIT {
            true => fits = middle,
            false => above = middle,
        }
    }
    (excerpt(fits), ProjectionMode::DeterministicExcerpt)
}

/// Adds the `memory_delete_marks` item of `marks` after the items of before. A Project with no
/// mark adds no item.
pub fn assemble_memory_delete_marks(
    record: &mut CurrentPassageAssemblyRecord,
    marks: &[MemoryDeleteMark],
) {
    if marks.is_empty() {
        return;
    }
    let (text, projection_mode) = memory_delete_marks_text(marks);
    let token_count = count_context_item_tokens(&text);
    let source_version = format!("sha256:{}", crate::hex_sha256(text.as_bytes()));
    record.considered.push(ConsideredSource {
        source_class: ContextSourceClass::MemoryDeleteMarks,
        source_version: source_version.clone(),
        token_count,
        eligible: true,
    });
    record.selected.push(SelectedProjection {
        source_class: ContextSourceClass::MemoryDeleteMarks,
        source_version,
        projection_mode,
        token_count,
        content: text,
    });
}

#[cfg(test)]
#[path = "memory_delete_marks_tests.rs"]
mod tests;
