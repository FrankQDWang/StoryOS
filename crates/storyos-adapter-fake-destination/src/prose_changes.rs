//! Fake-destination prose changes for the exact declared targets.

use storyos_core::{
    CONTEXT_ITEM_TOKEN_LIMIT, PROSE_CHANGE_TEXT, ProseChangeCandidate, SECOND_PROSE_CHANGE_TEXT,
};

/// Produce complete fake-destination changes for the exact declared targets.
pub(crate) fn produce_fake_prose_changes(
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
                    PROSE_CHANGE_TEXT
                } else {
                    SECOND_PROSE_CHANGE_TEXT
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
            first.explanation = "a".repeat(CONTEXT_ITEM_TOKEN_LIMIT as usize + 1);
        }
    }
    if author_message.ends_with("SCRIPT:reverse_locations") {
        candidates.reverse();
    }
    candidates
}

/// Revise the exact fresh candidate through the fake destination's bounded result contract.
pub(crate) fn produce_fake_candidate_revision(
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
pub(crate) fn is_fake_candidate_revision_request(author_message: &str) -> bool {
    let message = author_message.trim().to_ascii_lowercase();
    let message = message.trim_start_matches("please ");
    [
        "make ", "revise ", "rewrite ", "tighten ", "change ", "shorten ", "expand ",
    ]
    .iter()
    .any(|verb| message.starts_with(verb))
}
