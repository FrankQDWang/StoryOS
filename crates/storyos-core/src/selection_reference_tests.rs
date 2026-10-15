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
