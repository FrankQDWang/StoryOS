use std::ops::Range;

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
    if let RevisionComparisonAccess::WrongScope = access {
        return None;
    }
    let base: Vec<char> = source.base_text.chars().collect();
    let candidate: Vec<char> = source.candidate_text.chars().collect();
    let edits = normalize_fragmented_matches(&base, &candidate, myers_edits(&base, &candidate));
    Some(RevisionComparison {
        base_revision_id: source.base_revision_id.clone(),
        candidate_revision_id: source.candidate_revision_id.clone(),
        operation_id: source.operation_id.clone(),
        spans: replacement_spans(&base, &candidate, &edits),
    })
}

/// One run of scalars, given as index ranges into the base and candidate scalars.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Edit {
    Equal {
        base: Range<usize>,
        candidate: Range<usize>,
    },
    Delete(Range<usize>),
    Insert(Range<usize>),
}

impl Edit {
    fn is_empty(&self) -> bool {
        match self {
            Edit::Equal { base, .. } => base.is_empty(),
            Edit::Delete(run) | Edit::Insert(run) => run.is_empty(),
        }
    }
}

const TRACE_BYTE_CEILING: usize = 1024 * 1024;

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
        let snake = x
            .saturating_sub(previous_x)
            .min(y.saturating_sub(previous_y));
        prepend_step(
            &mut edits,
            Edit::Equal {
                base: x - snake..x,
                candidate: y - snake..y,
            },
        );
        x -= snake;
        y -= snake;
        if x == previous_x && y == 0 || x != previous_x && x == 0 {
            return coarse_replacement(base, candidate);
        }
        if x == previous_x {
            y -= 1;
            prepend_step(&mut edits, Edit::Insert(y..y + 1));
        } else {
            x -= 1;
            prepend_step(&mut edits, Edit::Delete(x..x + 1));
        }
    }
    let snake = x.min(y);
    prepend_step(
        &mut edits,
        Edit::Equal {
            base: x - snake..x,
            candidate: y - snake..y,
        },
    );
    prepend_step(&mut edits, Edit::Delete(0..x - snake));
    prepend_step(&mut edits, Edit::Insert(0..y - snake));
    edits.reverse();
    edits
}

/// Record a step that ends where the last recorded step starts, because backtracking walks backward.
fn prepend_step(edits: &mut Vec<Edit>, step: Edit) {
    if step.is_empty() {
        return;
    }
    match (edits.last_mut(), &step) {
        (
            Some(Edit::Equal { base, candidate }),
            Edit::Equal {
                base: step_base,
                candidate: step_candidate,
            },
        ) => {
            base.start = step_base.start;
            candidate.start = step_candidate.start;
        }
        (Some(Edit::Delete(run)), Edit::Delete(step_run))
        | (Some(Edit::Insert(run)), Edit::Insert(step_run)) => run.start = step_run.start,
        _ => edits.push(step),
    }
}

fn coarse_replacement(base: &[char], candidate: &[char]) -> Vec<Edit> {
    vec![
        Edit::Delete(0..base.len()),
        Edit::Insert(0..candidate.len()),
    ]
}

fn normalize_fragmented_matches(base: &[char], candidate: &[char], edits: Vec<Edit>) -> Vec<Edit> {
    let mut edits = coalesce(edits);
    let mut guard = edits.len().saturating_add(1);
    while guard > 0 {
        guard -= 1;
        if !absorb_one_fragment(base, candidate, &mut edits) {
            break;
        }
        edits = coalesce(edits);
    }
    merge_change_groups(base, candidate, edits)
}

fn absorb_one_fragment(base: &[char], candidate: &[char], edits: &mut Vec<Edit>) -> bool {
    let mut equality = None;
    let mut equality_len = 0_u32;
    let mut insertions_before = 0_u32;
    let mut deletions_before = 0_u32;
    let mut insertions_after = 0_u32;
    let mut deletions_after = 0_u32;
    for (index, edit) in edits.iter().enumerate() {
        match edit {
            Edit::Equal {
                base: base_run,
                candidate: candidate_run,
            } => {
                equality = Some((index, base_run.clone(), candidate_run.clone()));
                equality_len = utf16_len(&base[base_run.clone()]);
                insertions_before = insertions_after;
                deletions_before = deletions_after;
                insertions_after = 0;
                deletions_after = 0;
            }
            Edit::Insert(run) => insertions_after += utf16_len(&candidate[run.clone()]),
            Edit::Delete(run) => deletions_after += utf16_len(&base[run.clone()]),
        }
        let fragmented = equality.is_some()
            && equality_len <= insertions_before.max(deletions_before)
            && equality_len <= insertions_after.max(deletions_after);
        if fragmented {
            let Some((index, base_run, candidate_run)) = equality else {
                return false;
            };
            edits.splice(
                index..=index,
                [Edit::Delete(base_run), Edit::Insert(candidate_run)],
            );
            return true;
        }
    }
    false
}

fn merge_change_groups(base: &[char], candidate: &[char], edits: Vec<Edit>) -> Vec<Edit> {
    let mut edits = coalesce(edits).into_iter().peekable();
    let mut merged = Vec::new();
    let mut base_at = 0;
    let mut candidate_at = 0;
    while let Some(edit) = edits.next() {
        let (mut deleted, mut inserted) = match edit {
            Edit::Equal {
                base: base_run,
                candidate: candidate_run,
            } => {
                base_at = base_run.end;
                candidate_at = candidate_run.end;
                merged.push(Edit::Equal {
                    base: base_run,
                    candidate: candidate_run,
                });
                continue;
            }
            Edit::Delete(run) => (run, candidate_at..candidate_at),
            Edit::Insert(run) => (base_at..base_at, run),
        };
        while let Some(next) = edits.next_if(|next| !matches!(next, Edit::Equal { .. })) {
            match next {
                Edit::Delete(run) => deleted.end = run.end,
                Edit::Insert(run) => inserted.end = run.end,
                Edit::Equal { .. } => {}
            }
        }
        base_at = deleted.end;
        candidate_at = inserted.end;
        merged.extend(split_shared_edges(base, candidate, deleted, inserted));
    }
    coalesce(merged)
}

fn split_shared_edges(
    base: &[char],
    candidate: &[char],
    deleted: Range<usize>,
    inserted: Range<usize>,
) -> [Edit; 4] {
    let deleted_scalars = &base[deleted.clone()];
    let inserted_scalars = &candidate[inserted.clone()];
    let prefix = deleted_scalars
        .iter()
        .zip(inserted_scalars)
        .take_while(|(deleted, inserted)| deleted == inserted)
        .count();
    let suffix = deleted_scalars[prefix..]
        .iter()
        .rev()
        .zip(inserted_scalars[prefix..].iter().rev())
        .take_while(|(deleted, inserted)| deleted == inserted)
        .count();
    [
        Edit::Equal {
            base: deleted.start..deleted.start + prefix,
            candidate: inserted.start..inserted.start + prefix,
        },
        Edit::Delete(deleted.start + prefix..deleted.end - suffix),
        Edit::Insert(inserted.start + prefix..inserted.end - suffix),
        Edit::Equal {
            base: deleted.end - suffix..deleted.end,
            candidate: inserted.end - suffix..inserted.end,
        },
    ]
}

/// Drop empty runs and join adjacent runs of one kind. Adjacent runs of one kind are contiguous.
fn coalesce(mut edits: Vec<Edit>) -> Vec<Edit> {
    edits.retain(|edit| !edit.is_empty());
    edits.dedup_by(|next, current| match (current, next) {
        (
            Edit::Equal { base, candidate },
            Edit::Equal {
                base: next_base,
                candidate: next_candidate,
            },
        ) => {
            base.end = next_base.end;
            candidate.end = next_candidate.end;
            true
        }
        (Edit::Delete(run), Edit::Delete(next_run))
        | (Edit::Insert(run), Edit::Insert(next_run)) => {
            run.end = next_run.end;
            true
        }
        _ => false,
    });
    edits
}

fn replacement_spans(base: &[char], candidate: &[char], edits: &[Edit]) -> Vec<ReplacementSpan> {
    let mut spans = Vec::new();
    let mut base_at = 0_u32;
    let mut candidate_at = 0_u32;
    let mut index = 0;
    while index < edits.len() {
        match &edits[index] {
            Edit::Equal { base: run, .. } => {
                let len = utf16_len(&base[run.clone()]);
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
                        Edit::Delete(run) => {
                            let scalars = &base[run.clone()];
                            base_text.extend(scalars);
                            base_at += utf16_len(scalars);
                            index += 1;
                        }
                        Edit::Insert(run) => {
                            let scalars = &candidate[run.clone()];
                            candidate_text.extend(scalars);
                            candidate_at += utf16_len(scalars);
                            index += 1;
                        }
                        Edit::Equal { .. } => break,
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

fn utf16_len(scalars: &[char]) -> u32 {
    u32::try_from(
        scalars
            .iter()
            .map(|scalar| scalar.len_utf16())
            .sum::<usize>(),
    )
    .unwrap_or(u32::MAX)
}

#[cfg(test)]
#[path = "revision_comparison_tests.rs"]
mod tests;
