use std::ops::Range;

use super::{Edit, coalesce, exact_edits, utf16_len};

/// One base range moved through an exact comparison of two texts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RangeMapping {
    /// The range is inside one unchanged run. The offsets are UTF-16 offsets in the candidate.
    Unchanged { from: u32, to: u32 },
    /// A change touches the inside of the range, or the range is not a scalar range of the base.
    Changed,
    /// The comparison reached its trace limit, so it has no exact runs.
    Coarse,
}

/// Move the UTF-16 range `from..to` of `base_text` into `candidate_text`.
///
/// The common prefix and suffix are unchanged runs, and Myers compares only the middle. An
/// insertion inside the range slides out to an edge when the candidate text stays the same.
pub fn map_unchanged_range(
    base_text: &str,
    candidate_text: &str,
    from: u32,
    to: u32,
) -> RangeMapping {
    let base: Vec<char> = base_text.chars().collect();
    let candidate: Vec<char> = candidate_text.chars().collect();
    let (Some(from), Some(to)) = (scalar_index(&base, from), scalar_index(&base, to)) else {
        return RangeMapping::Changed;
    };
    if from >= to {
        return RangeMapping::Changed;
    }
    let Some(edits) = edge_anchored_edits(&base, &candidate) else {
        return RangeMapping::Coarse;
    };
    match contained(&slide_out(edits, &candidate, from, to), from, to) {
        Some(start) => RangeMapping::Unchanged {
            from: utf16_len(&candidate[..start]),
            to: utf16_len(&candidate[..start + (to - from)]),
        },
        None => RangeMapping::Changed,
    }
}

fn scalar_index(scalars: &[char], utf16_offset: u32) -> Option<usize> {
    let mut at = 0_u32;
    for (index, scalar) in scalars.iter().enumerate() {
        if at == utf16_offset {
            return Some(index);
        }
        at += scalar.len_utf16() as u32;
    }
    (at == utf16_offset).then_some(scalars.len())
}

fn edge_anchored_edits(base: &[char], candidate: &[char]) -> Option<Vec<Edit>> {
    let prefix = base
        .iter()
        .zip(candidate)
        .take_while(|(left, right)| left == right)
        .count();
    let suffix = base[prefix..]
        .iter()
        .rev()
        .zip(candidate[prefix..].iter().rev())
        .take_while(|(left, right)| left == right)
        .count();
    let base_middle = prefix..base.len() - suffix;
    let candidate_middle = prefix..candidate.len() - suffix;
    let middle = if base_middle.is_empty() || candidate_middle.is_empty() {
        vec![
            Edit::Delete(0..base_middle.len()),
            Edit::Insert(0..candidate_middle.len()),
        ]
    } else {
        exact_edits(
            &base[base_middle.clone()],
            &candidate[candidate_middle.clone()],
        )?
    };
    let mut edits = vec![Edit::Equal {
        base: 0..prefix,
        candidate: 0..prefix,
    }];
    edits.extend(middle.into_iter().map(|edit| shifted(edit, prefix)));
    edits.push(Edit::Equal {
        base: base_middle.end..base.len(),
        candidate: candidate_middle.end..candidate.len(),
    });
    Some(coalesce(edits))
}

fn shifted(edit: Edit, by: usize) -> Edit {
    let shift = |run: Range<usize>| run.start + by..run.end + by;
    match edit {
        Edit::Equal { base, candidate } => Edit::Equal {
            base: shift(base),
            candidate: shift(candidate),
        },
        Edit::Delete(run) => Edit::Delete(shift(run)),
        Edit::Insert(run) => Edit::Insert(shift(run)),
    }
}

/// The candidate start of a range that lies inside one unchanged run.
fn contained(edits: &[Edit], from: usize, to: usize) -> Option<usize> {
    edits.iter().find_map(|edit| match edit {
        Edit::Equal { base, candidate } if base.start <= from && to <= base.end => {
            Some(from - base.start + candidate.start)
        }
        Edit::Equal { .. } | Edit::Delete(_) | Edit::Insert(_) => None,
    })
}

/// Slide each pure insertion inside the range to one of its edges. A slide moves the insertion only
/// over equal scalars, so the candidate text stays the same.
fn slide_out(mut edits: Vec<Edit>, candidate: &[char], from: usize, to: usize) -> Vec<Edit> {
    for index in 1..edits.len().saturating_sub(/*rhs*/ 1) {
        let (
            Edit::Equal {
                base: before,
                candidate: before_candidate,
            },
            Edit::Insert(inserted),
            Edit::Equal { base: after, .. },
        ) = (&edits[index - 1], &edits[index], &edits[index + 1])
        else {
            continue;
        };
        let at = before.end;
        if at <= from || to <= at {
            continue;
        }
        let left = at - from;
        let right = to - at;
        let shift = if left <= before_candidate.len()
            && (1..=left)
                .all(|step| candidate[inserted.start - step] == candidate[inserted.end - step])
        {
            -(left as isize)
        } else if right <= after.len()
            && (0..right)
                .all(|step| candidate[inserted.start + step] == candidate[inserted.end + step])
        {
            right as isize
        } else {
            continue;
        };
        let moved = |position: usize| position.saturating_add_signed(shift);
        if let Edit::Equal { base, candidate } = &mut edits[index - 1] {
            base.end = moved(base.end);
            candidate.end = moved(candidate.end);
        }
        if let Edit::Insert(run) = &mut edits[index] {
            *run = moved(run.start)..moved(run.end);
        }
        if let Edit::Equal { base, candidate } = &mut edits[index + 1] {
            base.start = moved(base.start);
            candidate.start = moved(candidate.start);
        }
    }
    coalesce(edits)
}

#[cfg(test)]
#[path = "revision_range_mapping_tests.rs"]
mod tests;
