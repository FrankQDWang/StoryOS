use serde::{Deserialize, Serialize};

/// One complete producer change bound to an admitted paragraph and base.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProseChangeCandidate {
    pub chapter_id: String,
    pub manuscript_block_id: String,
    pub base_authoritative_revision_id: String,
    pub candidate_text: String,
    pub explanation: String,
}

/// Whether a declared target is a Chapter block or the block of one pending candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclaredTargetKind {
    ChapterBlock,
    PendingCandidate,
    /// A Chapter block that can join the Proposal of the referenced candidate (ADR 0071).
    /// `distance` is the signed block count from the pending Operations of that Proposal.
    /// -1 is the block above them, and 1 is the block below them.
    JoinableBlock {
        distance: i32,
    },
}

/// One declared target location that a prose change must name with its exact base.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredLocation {
    pub chapter_id: String,
    pub block_id: String,
    pub base_revision_id: String,
    pub kind: DeclaredTargetKind,
}

/// Validate all of the result before any candidate can open (ADR 0061). Each change is for a
/// different declared target with its declared base, and one or more changes are necessary.
/// A change of a pending candidate is the only change of its result (ADR 0062). Or the result
/// grows the Proposal of that candidate over joinable blocks (ADR 0071).
pub fn prose_changes_match_targets(
    targets: &[DeclaredLocation],
    candidates: &[ProseChangeCandidate],
) -> bool {
    let declared: std::collections::BTreeMap<_, _> = targets
        .iter()
        .map(|target| ((&target.chapter_id, &target.block_id), target))
        .collect();
    let actual: std::collections::BTreeSet<_> = candidates
        .iter()
        .map(|candidate| (&candidate.chapter_id, &candidate.manuscript_block_id))
        .collect();
    let changed_kinds: Vec<_> = candidates
        .iter()
        .filter_map(|candidate| {
            declared
                .get(&(&candidate.chapter_id, &candidate.manuscript_block_id))
                .map(|target| target.kind)
        })
        .collect();
    !candidates.is_empty()
        && candidate_changes_are_one_revision_or_growth(&changed_kinds)
        && targets.len() <= crate::assemble_context::CONTEXT_ITEM_TOKEN_LIMIT as usize + 1
        && declared.len() == targets.len()
        && actual.len() == candidates.len()
        && candidates.iter().all(|candidate| {
            declared
                .get(&(&candidate.chapter_id, &candidate.manuscript_block_id))
                .is_some_and(|target| {
                    target.base_revision_id == candidate.base_authoritative_revision_id
                })
                && !candidate.candidate_text.is_empty()
                && !candidate.explanation.trim().is_empty()
                && crate::assemble_context::count_context_item_tokens(&candidate.candidate_text)
                    <= crate::assemble_context::CONTEXT_ITEM_TOKEN_LIMIT
                && crate::assemble_context::count_context_item_tokens(&candidate.explanation)
                    <= crate::assemble_context::CONTEXT_ITEM_TOKEN_LIMIT
        })
}

/// With no candidate change, any shape is valid here. With one, each other change is a joinable
/// block, and the changed blocks above and below the Proposal make one run with no gap.
fn candidate_changes_are_one_revision_or_growth(changed_kinds: &[DeclaredTargetKind]) -> bool {
    let mut candidate_changes = 0;
    let mut chapter_changes = 0;
    let mut distances = std::collections::BTreeSet::new();
    for kind in changed_kinds {
        match kind {
            DeclaredTargetKind::PendingCandidate => candidate_changes += 1,
            DeclaredTargetKind::ChapterBlock => chapter_changes += 1,
            DeclaredTargetKind::JoinableBlock { distance } => {
                if !distances.insert(*distance) {
                    return false;
                }
            }
        }
    }
    let above = distances.iter().filter(|distance| **distance < 0).count();
    let below = distances.iter().filter(|distance| **distance > 0).count();
    match candidate_changes {
        0 => true,
        1 => {
            chapter_changes == 0
                && !distances.contains(&0)
                && distances
                    .first()
                    .is_none_or(|first| *first >= -(above as i32))
                && distances.last().is_none_or(|last| *last <= below as i32)
        }
        _ => false,
    }
}
