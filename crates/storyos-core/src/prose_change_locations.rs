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
/// A change of a pending candidate is the only change of its result (ADR 0062).
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
    let changes_candidate = candidates.iter().any(|candidate| {
        declared
            .get(&(&candidate.chapter_id, &candidate.manuscript_block_id))
            .is_some_and(|target| target.kind == DeclaredTargetKind::PendingCandidate)
    });
    !candidates.is_empty()
        && (!changes_candidate || candidates.len() == 1)
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
