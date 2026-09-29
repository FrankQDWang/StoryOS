//! Derive an optional comparison from one exact base revision and one candidate revision.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevisionComparisonAccess {
    SameScope,
    WrongScope,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactRevisionTexts {
    pub base_revision_id: String,
    pub candidate_revision_id: String,
    pub operation_id: String,
    pub base_text: String,
    pub candidate_text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplacementSpan {
    pub base_from: u32,
    pub base_to: u32,
    pub candidate_from: u32,
    pub candidate_to: u32,
    pub base_text: String,
    pub candidate_text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionComparison {
    pub base_revision_id: String,
    pub candidate_revision_id: String,
    pub operation_id: String,
    pub spans: Vec<ReplacementSpan>,
}

/// Return the UTF-16 slice `from..to`, or `None` when the range is not in `text`.
pub fn utf16_slice(text: &str, from: u32, to: u32) -> Option<&str> {
    if from > to {
        return None;
    }
    let start = crate::utf16_offset_to_byte(text, from)?;
    let end = if from == to {
        start
    } else {
        start + crate::utf16_offset_to_byte(&text[start..], to - from)?
    };
    text.get(start..end)
}

/// Inspect one comparison. A wrong Scope returns no text and no spans.
pub fn inspect_revision_comparison(
    access: RevisionComparisonAccess,
    source: &ExactRevisionTexts,
) -> Option<RevisionComparison> {
    match access {
        RevisionComparisonAccess::WrongScope => None,
        RevisionComparisonAccess::SameScope => Some(derive_revision_comparison(source)),
    }
}

fn derive_revision_comparison(source: &ExactRevisionTexts) -> RevisionComparison {
    let edits =
        normalize_fragmented_matches(&diff_texts(&source.base_text, &source.candidate_text));
    RevisionComparison {
        base_revision_id: source.base_revision_id.clone(),
        candidate_revision_id: source.candidate_revision_id.clone(),
        operation_id: source.operation_id.clone(),
        spans: replacement_spans(&edits),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Edit {
    Equal(String),
    Delete(String),
    Insert(String),
}

/// One comparison trace stays inside the existing public JSON string ceiling.
const TRACE_BYTE_CEILING: usize = 1024 * 1024;

fn diff_texts(base: &str, candidate: &str) -> Vec<Edit> {
    let base_chars: Vec<char> = base.chars().collect();
    let candidate_chars: Vec<char> = candidate.chars().collect();
    coalesce(&myers_edits(&base_chars, &candidate_chars))
}

fn myers_edits(base: &[char], candidate: &[char]) -> Vec<Edit> {
    if base.is_empty() && candidate.is_empty() {
        return Vec::new();
    }
    let max = base.len() + candidate.len();
    let mut trace = Vec::new();
    let mut used = 0_usize;
    for distance in 0..=max {
        let width = 2 * distance + 1;
        let row_bytes = width.saturating_mul(size_of::<usize>());
        if used.saturating_add(row_bytes) > TRACE_BYTE_CEILING {
            return coarse_replacement(base, candidate);
        }
        used += row_bytes;
        let mut row = vec![0_usize; width];
        let mut done = false;
        let mut diagonal = -(distance as isize);
        while diagonal <= distance as isize {
            let mut x = next_x(&trace, distance, diagonal);
            let y_signed = (x as isize) - diagonal;
            if y_signed < 0 {
                return coarse_replacement(base, candidate);
            }
            let mut y = y_signed as usize;
            while x < base.len() && y < candidate.len() && base[x] == candidate[y] {
                x += 1;
                y += 1;
            }
            row[(diagonal + distance as isize) as usize] = x;
            if x >= base.len() && y >= candidate.len() {
                done = true;
                break;
            }
            diagonal += 2;
        }
        trace.push(row);
        if done {
            return backtrack(base, candidate, &trace);
        }
    }
    coarse_replacement(base, candidate)
}

fn next_x(trace: &[Vec<usize>], distance: usize, diagonal: isize) -> usize {
    if distance == 0 {
        return 0;
    }
    let previous = &trace[distance - 1];
    let previous_distance = distance - 1;
    let read = |diagonal: isize| previous[(diagonal + previous_distance as isize) as usize];
    if followed_insert(distance, diagonal, &read) {
        read(diagonal + 1)
    } else {
        read(diagonal - 1) + 1
    }
}

fn followed_insert(distance: usize, diagonal: isize, read: &impl Fn(isize) -> usize) -> bool {
    diagonal == -(distance as isize)
        || (diagonal != distance as isize && read(diagonal - 1) < read(diagonal + 1))
}

fn backtrack(base: &[char], candidate: &[char], trace: &[Vec<usize>]) -> Vec<Edit> {
    let mut x = base.len();
    let mut y = candidate.len();
    let mut edits = Vec::new();
    for distance in (1..trace.len()).rev() {
        let diagonal = x as isize - y as isize;
        let previous = &trace[distance - 1];
        let previous_distance = distance - 1;
        let read = |diagonal: isize| previous[(diagonal + previous_distance as isize) as usize];
        let previous_diagonal = if followed_insert(distance, diagonal, &read) {
            diagonal + 1
        } else {
            diagonal - 1
        };
        let previous_x = read(previous_diagonal);
        let previous_y_signed = previous_x as isize - previous_diagonal;
        if previous_y_signed < 0 {
            return coarse_replacement(base, candidate);
        }
        let previous_y = previous_y_signed as usize;
        while x > previous_x && y > previous_y {
            x -= 1;
            y -= 1;
            edits.push(Edit::Equal(base[x].to_string()));
        }
        if x == previous_x && y == 0 || x != previous_x && x == 0 {
            return coarse_replacement(base, candidate);
        }
        if x == previous_x {
            y -= 1;
            edits.push(Edit::Insert(candidate[y].to_string()));
        } else {
            x -= 1;
            edits.push(Edit::Delete(base[x].to_string()));
        }
    }
    while x > 0 && y > 0 {
        x -= 1;
        y -= 1;
        edits.push(Edit::Equal(base[x].to_string()));
    }
    while x > 0 {
        x -= 1;
        edits.push(Edit::Delete(base[x].to_string()));
    }
    while y > 0 {
        y -= 1;
        edits.push(Edit::Insert(candidate[y].to_string()));
    }
    edits.reverse();
    edits
}

fn coarse_replacement(base: &[char], candidate: &[char]) -> Vec<Edit> {
    let mut edits = Vec::new();
    if !base.is_empty() {
        edits.push(Edit::Delete(base.iter().collect()));
    }
    if !candidate.is_empty() {
        edits.push(Edit::Insert(candidate.iter().collect()));
    }
    edits
}

fn normalize_fragmented_matches(edits: &[Edit]) -> Vec<Edit> {
    let mut edits = coalesce(edits);
    let mut guard = edits.len().saturating_add(1);
    while guard > 0 {
        guard -= 1;
        if !absorb_one_fragment(&mut edits) {
            break;
        }
        edits = coalesce(&edits);
    }
    merge_change_groups(&edits)
}

fn absorb_one_fragment(edits: &mut Vec<Edit>) -> bool {
    let mut equality_index = None;
    let mut equality = String::new();
    let mut insertions_before = 0_u32;
    let mut deletions_before = 0_u32;
    let mut insertions_after = 0_u32;
    let mut deletions_after = 0_u32;
    for (index, edit) in edits.iter().enumerate() {
        match edit {
            Edit::Equal(text) => {
                equality_index = Some(index);
                equality = text.clone();
                insertions_before = insertions_after;
                deletions_before = deletions_after;
                insertions_after = 0;
                deletions_after = 0;
            }
            Edit::Insert(text) => insertions_after += utf16_len(text),
            Edit::Delete(text) => deletions_after += utf16_len(text),
        }
        let fragmented = equality_index.is_some()
            && utf16_len(&equality) <= insertions_before.max(deletions_before)
            && utf16_len(&equality) <= insertions_after.max(deletions_after);
        if fragmented {
            let Some(index) = equality_index else {
                return false;
            };
            edits.splice(
                index..=index,
                [Edit::Delete(equality.clone()), Edit::Insert(equality)],
            );
            return true;
        }
    }
    false
}

fn merge_change_groups(edits: &[Edit]) -> Vec<Edit> {
    let edits = coalesce(edits);
    let mut merged = Vec::new();
    let mut index = 0;
    while index < edits.len() {
        if matches!(edits[index], Edit::Equal(_)) {
            merged.push(edits[index].clone());
            index += 1;
            continue;
        }
        let mut deleted = String::new();
        let mut inserted = String::new();
        while index < edits.len() && !matches!(edits[index], Edit::Equal(_)) {
            match &edits[index] {
                Edit::Delete(text) => deleted.push_str(text),
                Edit::Insert(text) => inserted.push_str(text),
                Edit::Equal(_) => {}
            }
            index += 1;
        }
        let (prefix, deleted, inserted, suffix) = split_shared_edges(&deleted, &inserted);
        if !prefix.is_empty() {
            merged.push(Edit::Equal(prefix));
        }
        if !deleted.is_empty() {
            merged.push(Edit::Delete(deleted));
        }
        if !inserted.is_empty() {
            merged.push(Edit::Insert(inserted));
        }
        if !suffix.is_empty() {
            merged.push(Edit::Equal(suffix));
        }
    }
    coalesce(&merged)
}

fn split_shared_edges(deleted: &str, inserted: &str) -> (String, String, String, String) {
    let mut deleted: Vec<char> = deleted.chars().collect();
    let mut inserted: Vec<char> = inserted.chars().collect();
    let mut prefix = String::new();
    while !deleted.is_empty() && deleted.first() == inserted.first() {
        prefix.push(deleted.remove(0));
        inserted.remove(0);
    }
    let mut suffix = Vec::new();
    while !deleted.is_empty() && deleted.last() == inserted.last() {
        suffix.push(deleted.pop().expect("suffix char"));
        inserted.pop();
    }
    suffix.reverse();
    (
        prefix,
        deleted.into_iter().collect(),
        inserted.into_iter().collect(),
        suffix.into_iter().collect(),
    )
}

fn coalesce(edits: &[Edit]) -> Vec<Edit> {
    let mut coalesced = Vec::new();
    for edit in edits {
        let text = match edit {
            Edit::Equal(text) | Edit::Delete(text) | Edit::Insert(text) => text,
        };
        if text.is_empty() {
            continue;
        }
        match (coalesced.last_mut(), edit) {
            (Some(Edit::Equal(current)), Edit::Equal(text))
            | (Some(Edit::Delete(current)), Edit::Delete(text))
            | (Some(Edit::Insert(current)), Edit::Insert(text)) => current.push_str(text),
            _ => coalesced.push(edit.clone()),
        }
    }
    coalesced
}

fn replacement_spans(edits: &[Edit]) -> Vec<ReplacementSpan> {
    let mut spans = Vec::new();
    let mut base_at = 0_u32;
    let mut candidate_at = 0_u32;
    let mut index = 0;
    while index < edits.len() {
        match &edits[index] {
            Edit::Equal(text) => {
                let len = utf16_len(text);
                base_at += len;
                candidate_at += len;
                index += 1;
            }
            Edit::Delete(_) | Edit::Insert(_) => {
                let base_from = base_at;
                let candidate_from = candidate_at;
                let mut base_text = String::new();
                let mut candidate_text = String::new();
                while index < edits.len() {
                    match &edits[index] {
                        Edit::Delete(text) => {
                            base_text.push_str(text);
                            base_at += utf16_len(text);
                            index += 1;
                        }
                        Edit::Insert(text) => {
                            candidate_text.push_str(text);
                            candidate_at += utf16_len(text);
                            index += 1;
                        }
                        Edit::Equal(_) => break,
                    }
                }
                spans.push(ReplacementSpan {
                    base_from,
                    base_to: base_at,
                    candidate_from,
                    candidate_to: candidate_at,
                    base_text,
                    candidate_text,
                });
            }
        }
    }
    spans
}

fn utf16_len(text: &str) -> u32 {
    u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX)
}

#[cfg(test)]
#[path = "revision_comparison_tests.rs"]
mod tests;
