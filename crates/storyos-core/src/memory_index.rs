//! The Memory Index of a Run (ADR 0094 section 4): one `memory_index` Context item with the type,
//! the title, and the description of each current Memory Entry at the pinned Memory revision.
//! The Index does not hold the text of an entry.

use crate::assemble_context::{
    ConsideredSource, ProjectionMode, SelectedProjection, count_context_item_tokens,
};
use crate::{ContextSourceClass, CurrentPassageAssemblyRecord, MemoryEntryType, canonical_json};

/// The Host limit of the entries that the Index lists (initial value, parent default 1).
pub const MEMORY_INDEX_MAX_ENTRIES: usize = 300;
/// The Host limit of the counted tokens of the Index text (initial value, ADR 0052).
pub const MEMORY_INDEX_TOKEN_LIMIT: u64 = 32_000;

/// One current Memory Entry at the pinned Memory revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryIndexEntry {
    /// The number of the entry in the Project. The model names an entry by this number.
    pub number: u64,
    pub entry_type: MemoryEntryType,
    pub title: String,
    pub description: String,
}

/// The text of the `memory_index` item and its projection mode. Each type that has an entry is
/// one group, in the order of the type list. A group starts with the line `{"type": type}`, and
/// each entry is the line `{"description", "id", "title"}`, in the order of the numbers. When the
/// entries are above a Host limit, the text lists the entries in order up to the limit and ends
/// with the line `{"not_listed": count}`. Then the mode is `deterministic_excerpt`.
pub fn memory_index_text(entries: &[MemoryIndexEntry]) -> (String, ProjectionMode) {
    let mut ordered: Vec<&MemoryIndexEntry> = entries.iter().collect();
    ordered.sort_by_key(|entry| (entry.entry_type, entry.number));
    let complete = render(&ordered);
    if ordered.len() <= MEMORY_INDEX_MAX_ENTRIES
        && count_context_item_tokens(&complete) <= MEMORY_INDEX_TOKEN_LIMIT
    {
        return (complete, ProjectionMode::ExactRequired);
    }
    let mut listed = ordered.len().min(MEMORY_INDEX_MAX_ENTRIES);
    loop {
        let text = excerpt(&ordered, listed);
        if count_context_item_tokens(&text) <= MEMORY_INDEX_TOKEN_LIMIT || listed == 0 {
            return (text, ProjectionMode::DeterministicExcerpt);
        }
        listed -= 1;
    }
}

fn render(entries: &[&MemoryIndexEntry]) -> String {
    let mut lines = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        if index == 0 || entries[index - 1].entry_type != entry.entry_type {
            lines.push(canonical_json(
                &serde_json::json!({ "type": entry.entry_type.as_str() }),
            ));
        }
        lines.push(canonical_json(&serde_json::json!({
            "id": entry.number,
            "title": entry.title,
            "description": entry.description,
        })));
    }
    lines.join("\n")
}

fn excerpt(entries: &[&MemoryIndexEntry], listed: usize) -> String {
    let count_line = canonical_json(&serde_json::json!({ "not_listed": entries.len() - listed }));
    match listed {
        0 => count_line,
        _ => format!("{}\n{count_line}", render(&entries[..listed])),
    }
}

/// Records the pin of the Memory revision of the Run in the Operation Requirement and adds the
/// `memory_index` item of `entries` after the items of before. A Run with no pin, or with no
/// current entry at the pinned revision, adds no item.
pub fn assemble_memory_index(
    record: &mut CurrentPassageAssemblyRecord,
    pin: Option<u64>,
    entries: &[MemoryIndexEntry],
) {
    record.operation_requirement.memory_revision = pin;
    if pin.is_none() || entries.is_empty() {
        return;
    }
    let (text, projection_mode) = memory_index_text(entries);
    let token_count = count_context_item_tokens(&text);
    let source_version = format!("sha256:{}", crate::hex_sha256(text.as_bytes()));
    record.considered.push(ConsideredSource {
        source_class: ContextSourceClass::MemoryIndex,
        source_version: source_version.clone(),
        token_count,
        eligible: true,
    });
    record.selected.push(SelectedProjection {
        source_class: ContextSourceClass::MemoryIndex,
        source_version,
        projection_mode,
        token_count,
        content: text,
    });
}

#[cfg(test)]
#[path = "memory_index_tests.rs"]
mod tests;
