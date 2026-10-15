use super::*;
use storyos_application::{DestinationRequest, RequestAttempt};
use storyos_core::{AdmittedSelection, DeclaredTargetKind, TextPoint};

#[test]
fn the_model_reads_the_exact_references_and_the_pending_candidate_target() {
    let DestinationRequest::Create(request) = crate::tests::create(
        crate::tests::route("http://127.0.0.1:9"),
        RequestAttempt::New,
    ) else {
        unreachable!("create() makes a Create request")
    };
    let chapter = request.declared_targets[0].clone();
    let request = CreateRequest {
        declared_targets: vec![
            chapter.clone(),
            storyos_application::DeclaredTarget {
                block_id: "block-2".to_owned(),
                kind: DeclaredTargetKind::PendingCandidate,
                block_text: "The wind stopped.".to_owned(),
                ..chapter.clone()
            },
        ],
        selection_references: vec![
            AdmittedSelection {
                reference: SelectionReference::ChapterText {
                    chapter_id: chapter.chapter_id.clone(),
                    base_authoritative_revision_id: chapter.base_revision_id.clone(),
                    start: TextPoint {
                        manuscript_block_id: chapter.block_id.clone(),
                        offset: 4,
                    },
                    end: TextPoint {
                        manuscript_block_id: chapter.block_id.clone(),
                        offset: 13,
                    },
                },
                selected_text: Some("rain fell".to_owned()),
                manuscript_block_ids: vec![chapter.block_id.clone()],
                candidate_text: None,
            },
            AdmittedSelection {
                reference: SelectionReference::ProposalCandidateText {
                    chapter_id: chapter.chapter_id.clone(),
                    proposal_id: "proposal-1".to_owned(),
                    operation_id: "operation-1".to_owned(),
                    revision_id: "revision-9".to_owned(),
                    start_offset: 4,
                    end_offset: 8,
                },
                selected_text: Some("wind".to_owned()),
                manuscript_block_ids: vec!["block-2".to_owned()],
                candidate_text: Some("The wind rose.".to_owned()),
            },
        ],
        ..request
    };

    assert_eq!(
        request_text(&request),
        format!(
            "Author request:\nTighten this paragraph.\n\nWorking text:\nThe rain fell hard.\n\n\
             Author references:\n[reference 1] Chapter text of chapter_id {chapter_id}, from \
             block_id {block} character 4 to block_id {block} character 13 (UTF-16 code units):\n\
             rain fell\n\n[reference 2] Pending Proposal candidate text of block_id block-2, from \
             character 4 to character 8 (UTF-16 code units):\nwind\n\n\
             Target blocks:\n[block_id: {block}]\nThe rain fell hard on the old roof.\n\n\
             [block_id: block-2] (pending Proposal candidate)\nThe wind stopped.",
            chapter_id = chapter.chapter_id,
            block = chapter.block_id,
        )
    );
}
