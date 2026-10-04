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

/// Validate the whole declared result before any candidate can open.
pub fn prose_changes_match_targets(
    targets: &[(String, String, String)],
    candidates: &[ProseChangeCandidate],
) -> bool {
    let declared: std::collections::BTreeMap<_, _> = targets
        .iter()
        .map(|(chapter, block, base)| ((chapter, block), base))
        .collect();
    let actual: std::collections::BTreeSet<_> = candidates
        .iter()
        .map(|candidate| (&candidate.chapter_id, &candidate.manuscript_block_id))
        .collect();
    !candidates.is_empty()
        && targets.len() <= crate::assemble_context::CONTEXT_ITEM_TOKEN_LIMIT as usize + 1
        && declared.len() == targets.len()
        && actual.len() == targets.len()
        && candidates.len() == targets.len()
        && candidates.iter().all(|candidate| {
            declared
                .get(&(&candidate.chapter_id, &candidate.manuscript_block_id))
                .is_some_and(|base| **base == candidate.base_authoritative_revision_id)
                && !candidate.candidate_text.is_empty()
                && !candidate.explanation.trim().is_empty()
                && crate::assemble_context::count_context_item_tokens(&candidate.candidate_text)
                    <= crate::assemble_context::CONTEXT_ITEM_TOKEN_LIMIT
                && crate::assemble_context::count_context_item_tokens(&candidate.explanation)
                    <= crate::assemble_context::CONTEXT_ITEM_TOKEN_LIMIT
        })
}
