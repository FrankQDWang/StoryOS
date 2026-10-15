use super::*;
use storyos_application::{DestinationRequest, RequestAttempt};
use storyos_core::{AdmittedSelection, TextPoint};

#[test]
fn the_model_reads_the_exact_text_and_range_of_each_reference() {
    let DestinationRequest::Create(request) = crate::tests::create(
        crate::tests::route("http://127.0.0.1:9"),
        RequestAttempt::New,
    ) else {
        unreachable!("create() makes a Create request")
    };
    let chapter = request.declared_targets[0].clone();
    let request = CreateRequest {
        selection_references: vec![AdmittedSelection {
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
        }],
        ..request
    };

    assert_eq!(
        request_text(&request),
        format!(
            "Author request:\nTighten this paragraph.\n\nWorking text:\nThe rain fell hard.\n\n\
             Author references:\n[reference 1] Chapter text of chapter_id {chapter_id}, from \
             block_id {block} character 4 to block_id {block} character 13 (UTF-16 code units):\n\
             rain fell\n\nTarget blocks:\n[block_id: {block}]\nThe rain fell hard on the old roof.",
            chapter_id = chapter.chapter_id,
            block = chapter.block_id,
        )
    );
}
