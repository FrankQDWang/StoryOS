//! The Chapter list of each Run (ADR 0095 section 1): one `chapter_list` Context item with the
//! number, the length, and the title of each live Chapter of the Project, in manuscript order.

use crate::assemble_context::{
    CONTEXT_ITEM_TOKEN_LIMIT, ConsideredSource, ProjectionMode, SelectedProjection,
    count_context_item_tokens,
};
use crate::{ContextSourceClass, CurrentPassageAssemblyRecord, canonical_json, hex_sha256};

/// One live Chapter of the Project, in manuscript order: the order of the Volumes, and then the
/// order of the Chapters in each Volume.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChapterListEntry {
    /// The position of the Volume in manuscript order.
    pub volume_order: u64,
    pub volume_title: String,
    pub chapter_id: String,
    pub title: String,
    /// The character count of the authoritative head text, with the count of the manuscript
    /// statistics.
    pub length: u64,
}

/// The text of the `chapter_list` item and its projection mode. Each Volume is one line
/// `{"volume": title}` before its Chapters. Each Chapter is one line `[number, length, title]`,
/// and the line of the Current Chapter `current_chapter_id` has `"current"` as a fourth element.
/// The number is the position in manuscript order, from 1. A list above the Context item limit
/// is a `deterministic_excerpt` of whole lines: the Current Chapter, then the Chapters before it
/// from the nearest, then the Chapters after it from the nearest, until the first line that does
/// not fit. Its last line gives the Chapter count and each range of numbers that it does not
/// list.
pub fn chapter_list_text(
    entries: &[ChapterListEntry],
    current_chapter_id: &str,
) -> (String, ProjectionMode) {
    let lines = Lines {
        entries,
        current: entries
            .iter()
            .position(|entry| entry.chapter_id == current_chapter_id),
    };
    if entries.is_empty() {
        return (String::new(), ProjectionMode::ExactRequired);
    }
    let complete = lines.render(0, entries.len() - 1, None);
    if count_context_item_tokens(&complete) <= CONTEXT_ITEM_TOKEN_LIMIT {
        return (complete, ProjectionMode::ExactRequired);
    }
    (lines.excerpt(), ProjectionMode::DeterministicExcerpt)
}

struct Lines<'a> {
    entries: &'a [ChapterListEntry],
    current: Option<usize>,
}

impl Lines<'_> {
    fn volume_line(&self, index: usize) -> String {
        canonical_json(&serde_json::json!({ "volume": self.entries[index].volume_title }))
    }

    fn chapter_line(&self, index: usize) -> String {
        let entry = &self.entries[index];
        let mut line = serde_json::json!([index + 1, entry.length, entry.title]);
        if self.current == Some(index) {
            line.as_array_mut()
                .expect("a Chapter line is an array")
                .push(serde_json::json!("current"));
        }
        canonical_json(&line)
    }

    fn new_volume(&self, index: usize, neighbor: usize) -> bool {
        self.entries[index].volume_order != self.entries[neighbor].volume_order
    }

    /// The last line of an excerpt of the Chapters `first..=last` (0-based).
    fn count_line(&self, first: usize, last: usize) -> String {
        let count = self.entries.len();
        let not_listed: Vec<[usize; 2]> = [(1, first), (last + 2, count)]
            .into_iter()
            .filter(|(from, to)| from <= to)
            .map(|(from, to)| [from, to])
            .collect();
        canonical_json(&serde_json::json!({
            "chapter_count": count,
            "not_listed": not_listed,
        }))
    }

    /// The text of the Chapters `first..=last` (0-based), with `count_line` as the last line
    /// when it is given.
    fn render(&self, first: usize, last: usize, count_line: Option<String>) -> String {
        let mut lines = Vec::new();
        for index in first..=last {
            if index == first || self.new_volume(index, index - 1) {
                lines.push(self.volume_line(index));
            }
            lines.push(self.chapter_line(index));
        }
        lines.extend(count_line);
        lines.join("\n")
    }

    /// The excerpt of whole lines. The kept Chapters are always one range that holds the Current
    /// Chapter, so the size of each step adds one Chapter line, and one Volume line when that
    /// Chapter starts a Volume at the edge of the range.
    fn excerpt(&self) -> String {
        let start = self.current.unwrap_or(0);
        let before = (0..start).rev();
        let after = start + 1..self.entries.len();
        let tokens = |line: String| count_context_item_tokens(&line) + 1;
        let size = |first, last, lines: u64| {
            lines + count_context_item_tokens(&self.count_line(first, last))
        };
        let mut kept: Option<(usize, usize, u64)> = None;
        for index in std::iter::once(start).chain(before).chain(after) {
            let (first, last, lines) = match kept {
                None => (
                    index,
                    index,
                    tokens(self.volume_line(index)) + tokens(self.chapter_line(index)),
                ),
                Some((first, last, lines)) => {
                    let neighbor = if index < first { first } else { last };
                    let volume = if self.new_volume(index, neighbor) {
                        tokens(self.volume_line(index))
                    } else {
                        0
                    };
                    (
                        first.min(index),
                        last.max(index),
                        lines + volume + tokens(self.chapter_line(index)),
                    )
                }
            };
            if size(first, last, lines) > CONTEXT_ITEM_TOKEN_LIMIT {
                break;
            }
            kept = Some((first, last, lines));
        }
        match kept {
            Some((first, last, _)) => self.render(first, last, Some(self.count_line(first, last))),
            None => self.count_line(self.entries.len(), self.entries.len()),
        }
    }
}

/// Adds the `chapter_list` item of `entries` to `record`, after the items of before. The Current
/// Chapter is the Chapter of the Operation Requirement. A Project with no live Chapter adds
/// nothing.
pub fn assemble_chapter_list(
    record: &mut CurrentPassageAssemblyRecord,
    entries: &[ChapterListEntry],
) {
    if entries.is_empty() {
        return;
    }
    let (text, projection_mode) =
        chapter_list_text(entries, &record.operation_requirement.chapter_id);
    let token_count = count_context_item_tokens(&text);
    let source_version = format!("sha256:{}", hex_sha256(text.as_bytes()));
    record.considered.push(ConsideredSource {
        source_class: ContextSourceClass::ChapterList,
        source_version: source_version.clone(),
        token_count,
        eligible: true,
    });
    record.selected.push(SelectedProjection {
        source_class: ContextSourceClass::ChapterList,
        source_version,
        projection_mode,
        token_count,
        content: text,
    });
}

#[cfg(test)]
#[path = "chapter_list_tests.rs"]
mod tests;
