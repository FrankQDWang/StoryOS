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
    targets: &[(String, String, String)],
    author_message: &str,
) -> Vec<ProseChangeCandidate> {
    let mut candidates: Vec<_> = targets
        .iter()
        .enumerate()
        .map(
            |(index, (chapter_id, block_id, revision_id))| ProseChangeCandidate {
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
            },
        )
        .collect();
    if let Some(first) = candidates.first_mut() {
        if author_message.ends_with("SCRIPT:malformed_locations") {
            first.explanation.clear();
        } else if author_message.ends_with("SCRIPT:undeclared_location") {
            first.manuscript_block_id = first.chapter_id.clone();
        } else if author_message.ends_with("SCRIPT:stale_location_base") {
            first.base_authoritative_revision_id = first.manuscript_block_id.clone();
        } else if author_message.ends_with("SCRIPT:oversized_explanation") {
            first.explanation =
                "a".repeat(crate::assemble_context::CONTEXT_ITEM_TOKEN_LIMIT as usize + 1);
        }
    }
    if author_message.ends_with("SCRIPT:reverse_locations") {
        candidates.reverse();
    }
    candidates
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

/// Revise the exact fresh candidate through the fake destination's bounded result contract.
pub fn produce_fake_candidate_revision(
    targets: &[(String, String, String)],
    author_message: &str,
    candidate: &str,
) -> Vec<ProseChangeCandidate> {
    let mut changes = produce_fake_prose_changes(targets, author_message);
    for change in &mut changes {
        change.candidate_text = if author_message.contains("calmer") {
            candidate.replace("Guard the narrator voice", "Keep the narrator calm")
        } else {
            format!("{candidate} Keep the voice consistent.")
        };
        change.explanation =
            "Revise the selected candidate under the new author instruction.".to_owned();
    }
    changes
}

/// Recognize an explicit candidate revision request in the finite fake profile.
pub fn is_fake_candidate_revision_request(author_message: &str) -> bool {
    let message = author_message.trim().to_ascii_lowercase();
    let message = message.trim_start_matches("please ");
    [
        "make ", "revise ", "rewrite ", "tighten ", "change ", "shorten ", "expand ",
    ]
    .iter()
    .any(|verb| message.starts_with(verb))
}
