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

/// Produce complete fake-destination changes for the exact declared targets.
pub fn produce_fake_prose_changes(
    chapter_id: &str,
    targets: &[(String, String)],
) -> Vec<ProseChangeCandidate> {
    targets
        .iter()
        .enumerate()
        .map(|(index, (block_id, revision_id))| ProseChangeCandidate {
            chapter_id: chapter_id.to_owned(),
            manuscript_block_id: block_id.clone(),
            base_authoritative_revision_id: revision_id.clone(),
            candidate_text: if index == 0 {
                crate::PROSE_CHANGE_TEXT
            } else {
                crate::SECOND_PROSE_CHANGE_TEXT
            }
            .to_owned(),
            explanation: if index == 0 {
                "Preserve the narrator voice in the first passage."
            } else {
                "Keep the second passage consistent with the narrator voice."
            }
            .to_owned(),
        })
        .collect()
}

/// Validate the whole declared result before any candidate can open.
pub fn prose_changes_match_targets(
    chapter_id: &str,
    targets: &[(String, String)],
    candidates: &[ProseChangeCandidate],
) -> bool {
    let declared: std::collections::BTreeMap<_, _> =
        targets.iter().map(|(block, base)| (block, base)).collect();
    let actual: std::collections::BTreeSet<_> = candidates
        .iter()
        .map(|candidate| &candidate.manuscript_block_id)
        .collect();
    !candidates.is_empty()
        && targets.len() <= crate::assemble_context::CONTEXT_ITEM_TOKEN_LIMIT as usize + 1
        && declared.len() == targets.len()
        && actual.len() == targets.len()
        && candidates.len() == targets.len()
        && candidates.iter().all(|candidate| {
            candidate.chapter_id == chapter_id
                && declared
                    .get(&candidate.manuscript_block_id)
                    .is_some_and(|base| **base == candidate.base_authoritative_revision_id)
                && !candidate.candidate_text.is_empty()
                && !candidate.explanation.trim().is_empty()
                && crate::assemble_context::count_context_item_tokens(&candidate.candidate_text)
                    <= crate::assemble_context::CONTEXT_ITEM_TOKEN_LIMIT
                && crate::assemble_context::count_context_item_tokens(&candidate.explanation)
                    <= crate::assemble_context::CONTEXT_ITEM_TOKEN_LIMIT
        })
}
