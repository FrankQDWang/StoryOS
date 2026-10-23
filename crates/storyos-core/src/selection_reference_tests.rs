use super::*;

fn point(block: &str, offset: u32) -> TextPoint {
    TextPoint {
        manuscript_block_id: block.to_owned(),
        offset,
    }
}

#[test]
fn a_chapter_selection_keeps_the_exact_range_across_blocks() {
    let blocks = [
        ("b1", "雨夜。渡口"),
        ("b2", "船夫😀等着"),
        ("b3", "灯灭了。"),
    ]
    .map(|(id, text)| (id.to_owned(), text.to_owned()));

    assert_eq!(
        [
            select_chapter_text(&blocks, &point("b1", 3), &point("b3", 2)),
            select_chapter_text(&blocks, &point("b2", 2), &point("b2", 4)),
            select_chapter_text(&blocks, &point("b1", 5), &point("b2", 0)),
            select_chapter_text(&blocks, &point("b2", 0), &point("b1", 1)),
            select_chapter_text(&blocks, &point("b2", 3), &point("b2", 5)),
            select_chapter_text(&blocks, &point("b3", 0), &point("b3", 5)),
        ],
        [
            Some((
                "渡口\n船夫😀等着\n灯灭".to_owned(),
                vec!["b1".to_owned(), "b2".to_owned(), "b3".to_owned()]
            )),
            Some(("😀".to_owned(), vec!["b2".to_owned()])),
            Some(("\n".to_owned(), vec!["b1".to_owned(), "b2".to_owned()])),
            None,
            None,
            None,
        ]
    );
}

#[test]
fn a_candidate_selection_is_a_non_empty_utf16_range() {
    assert_eq!(
        [
            select_candidate_text("风停了。", 0, 2),
            select_candidate_text("风停了。", 2, 2),
            select_candidate_text("风停了。", 3, 9),
        ],
        [Some("风停".to_owned()), None, None]
    );
}

#[test]
fn a_revised_candidate_moves_each_reference_of_its_proposal_to_the_new_revision() {
    let candidate = |operation: &str, revision: &str, start_offset| AdmittedSelection {
        reference: SelectionReference::ProposalCandidateText {
            chapter_id: "chapter-1".to_owned(),
            proposal_id: "proposal-1".to_owned(),
            operation_id: operation.to_owned(),
            revision_id: revision.to_owned(),
            start_offset,
            end_offset: start_offset + 2,
        },
        selected_text: Some("雨夜".to_owned()),
        manuscript_block_ids: vec![format!("block-{operation}")],
        candidate_text: Some("雨夜。渡口".to_owned()),
    };
    let mut references = [
        candidate("operation-1", "revision-1", /*start_offset*/ 1),
        candidate("operation-2", "revision-1", /*start_offset*/ 3),
        candidate("operation-3", "revision-0", /*start_offset*/ 0),
    ];

    advance_candidate_references(
        references.iter_mut(),
        &crate::ProposalCandidateTarget {
            proposal_id: "proposal-1".to_owned(),
            operation_id: "operation-1".to_owned(),
            revision_id: "revision-1".to_owned(),
        },
        "revision-2",
        "风😀",
    );

    assert_eq!(
        references
            .iter()
            .map(|admitted| admitted.reference.clone())
            .collect::<Vec<_>>(),
        [
            ("operation-1", "revision-2", 0, 3),
            ("operation-2", "revision-2", 3, 5),
            ("operation-3", "revision-0", 0, 2),
        ]
        .map(|(operation, revision, start_offset, end_offset)| {
            SelectionReference::ProposalCandidateText {
                chapter_id: "chapter-1".to_owned(),
                proposal_id: "proposal-1".to_owned(),
                operation_id: operation.to_owned(),
                revision_id: revision.to_owned(),
                start_offset,
                end_offset,
            }
        })
    );
}

#[test]
fn a_revised_candidate_covers_its_new_text_after_a_joined_location_of_the_same_growth() {
    let mut references = [AdmittedSelection {
        reference: SelectionReference::ProposalCandidateText {
            chapter_id: "chapter-1".to_owned(),
            proposal_id: "proposal-1".to_owned(),
            operation_id: "operation-1".to_owned(),
            revision_id: "revision-1".to_owned(),
            start_offset: 1,
            end_offset: 3,
        },
        selected_text: Some("夜。".to_owned()),
        manuscript_block_ids: vec!["b1".to_owned()],
        candidate_text: Some("雨夜。渡口".to_owned()),
    }];
    let prior = |operation: &str| crate::ProposalCandidateTarget {
        proposal_id: "proposal-1".to_owned(),
        operation_id: operation.to_owned(),
        revision_id: "revision-1".to_owned(),
    };

    // A reply can list the joined block before the candidate.
    for operation in ["operation-2", "operation-1"] {
        advance_candidate_references(
            references.iter_mut(),
            &prior(operation),
            "revision-2",
            "风😀",
        );
    }

    assert_eq!(
        references[0].reference,
        SelectionReference::ProposalCandidateText {
            chapter_id: "chapter-1".to_owned(),
            proposal_id: "proposal-1".to_owned(),
            operation_id: "operation-1".to_owned(),
            revision_id: "revision-2".to_owned(),
            start_offset: 0,
            end_offset: 3,
        }
    );
}

#[test]
fn a_growth_moves_the_chapter_references_of_the_joined_blocks_to_their_candidates() {
    let candidate = |proposal: &str, operation: &str, block: &str, end_offset| AdmittedSelection {
        reference: SelectionReference::ProposalCandidateText {
            chapter_id: "chapter-1".to_owned(),
            proposal_id: proposal.to_owned(),
            operation_id: operation.to_owned(),
            revision_id: "revision-2".to_owned(),
            start_offset: 0,
            end_offset,
        },
        selected_text: Some("雨夜".to_owned()),
        manuscript_block_ids: vec![block.to_owned()],
        candidate_text: Some("雨夜".to_owned()),
    };
    let chapter = |blocks: &[&str]| AdmittedSelection {
        reference: SelectionReference::ChapterText {
            chapter_id: "chapter-1".to_owned(),
            base_authoritative_revision_id: "chapter-revision-1".to_owned(),
            start: point(blocks[0], /*offset*/ 0),
            end: point(blocks[blocks.len() - 1], /*offset*/ 2),
        },
        selected_text: Some("渡口".to_owned()),
        manuscript_block_ids: blocks.iter().map(|block| (*block).to_owned()).collect(),
        candidate_text: None,
    };
    let grown = |operation: &'static str, block: &'static str, text: &'static str| GrownCandidate {
        chapter_id: "chapter-1",
        manuscript_block_id: block,
        target: crate::ProposalCandidateTarget {
            proposal_id: "proposal-1".to_owned(),
            operation_id: operation.to_owned(),
            revision_id: "revision-2".to_owned(),
        },
        candidate_text: text,
    };
    let joined = |operation: &str, block: &str, text: &str| AdmittedSelection {
        selected_text: Some(text.to_owned()),
        candidate_text: Some(text.to_owned()),
        ..candidate(
            "proposal-1",
            operation,
            block,
            text.encode_utf16().count() as u32,
        )
    };
    let below = [
        grown("operation-1", "b1", "雨夜"),
        grown("operation-2", "b2", "船夫😀"),
    ];
    let above = [
        grown("operation-0", "b0", "风停了"),
        grown("operation-1", "b1", "雨夜"),
    ];
    let other = candidate("proposal-9", "operation-9", "b5", /*end_offset*/ 2);
    let narrow = AdmittedSelection {
        reference: SelectionReference::ChapterText {
            chapter_id: "chapter-1".to_owned(),
            base_authoritative_revision_id: "chapter-revision-1".to_owned(),
            start: point("b2", /*offset*/ 1),
            end: point("b2", /*offset*/ 2),
        },
        ..chapter(&["b2"])
    };

    assert_eq!(
        [
            reference_joined_candidates(
                vec![
                    candidate("proposal-1", "operation-1", "b1", /*end_offset*/ 2),
                    chapter(&["b2"])
                ],
                &below,
            ),
            reference_joined_candidates(
                vec![
                    chapter(&["b0", "b1"]),
                    candidate("proposal-1", "operation-1", "b1", /*end_offset*/ 2)
                ],
                &above,
            ),
            reference_joined_candidates(
                vec![
                    candidate("proposal-1", "operation-1", "b1", /*end_offset*/ 2),
                    chapter(&["b2", "b3"]),
                    other.clone()
                ],
                &below,
            ),
            reference_joined_candidates(
                vec![
                    candidate("proposal-1", "operation-1", "b1", /*end_offset*/ 2),
                    chapter(&["b2"]),
                    narrow
                ],
                &below,
            ),
        ],
        [
            vec![
                candidate("proposal-1", "operation-1", "b1", /*end_offset*/ 2),
                joined("operation-2", "b2", "船夫😀")
            ],
            vec![
                joined("operation-0", "b0", "风停了"),
                candidate("proposal-1", "operation-1", "b1", /*end_offset*/ 2)
            ],
            vec![
                candidate("proposal-1", "operation-1", "b1", /*end_offset*/ 2),
                chapter(&["b2", "b3"]),
                joined("operation-2", "b2", "船夫😀"),
                other,
            ],
            vec![
                candidate("proposal-1", "operation-1", "b1", /*end_offset*/ 2),
                joined("operation-2", "b2", "船夫😀")
            ],
        ]
    );
}

#[test]
fn a_steering_input_adds_only_the_references_that_the_run_does_not_have() {
    let chapter = |start| SelectionReference::ChapterText {
        chapter_id: "chapter-1".to_owned(),
        base_authoritative_revision_id: "revision-1".to_owned(),
        start: point("b1", start),
        end: point("b1", start + 2),
    };

    assert_eq!(
        merge_steering_references(
            [chapter(/*start*/ 0), chapter(/*start*/ 4)],
            &[chapter(/*start*/ 4), chapter(/*start*/ 2)]
        ),
        [
            chapter(/*start*/ 0),
            chapter(/*start*/ 4),
            chapter(/*start*/ 2)
        ]
    );
    assert_eq!(
        merge_steering_references([], &[chapter(/*start*/ 1)]),
        [chapter(/*start*/ 1)]
    );
}
