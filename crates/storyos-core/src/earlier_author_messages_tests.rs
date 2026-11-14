use super::{
    EARLIER_AUTHOR_MESSAGES_TOKEN_LIMIT, EarlierAuthorMessage, EarlierAuthorMessageSource,
    is_earlier_author_message, keep_earlier_author_messages,
};
use crate::{ContextSourceClass, ProjectionMode, later_request_record};

fn message(message_id: &str, text: String) -> EarlierAuthorMessage {
    EarlierAuthorMessage {
        source: EarlierAuthorMessageSource {
            run_id: format!("run-of-{message_id}"),
            message_id: message_id.to_owned(),
        },
        text,
    }
}

fn later_record() -> crate::CurrentPassageAssemblyRecord {
    let decision = crate::assemble_current_passage_context(&crate::CurrentPassageAssembly {
        operation_requirement_id: "requirement-1".to_owned(),
        input_snapshot_id: "snapshot-1".to_owned(),
        run_id: "run-1".to_owned(),
        owner_user_id: "owner-1".to_owned(),
        project_id: "project-1".to_owned(),
        author_message: "Help again.".to_owned(),
        chapter_id: "chapter-1".to_owned(),
        chapter_revision_id: Some("rev-1".to_owned()),
        proposal_target_block_ids: None,
        chapter_body: "Passage.".to_owned(),
        instruction: crate::InstructionBindingInput::Absent,
        destination_identity: "destination-1".to_owned(),
    });
    later_request_record(&decision, "compaction-1", "Summary.", &[])
}

#[test]
fn the_host_keeps_whole_messages_from_the_newest_and_a_message_that_does_not_fit_ends_the_selection()
 {
    // The two newest messages fill the limit exactly. The next message does not fit, so it and
    // the oldest message, which alone would fit, are left out.
    let messages = [
        message("oldest", "a".to_owned()),
        message("third", "bb".to_owned()),
        message("second", "c".repeat(15_000)),
        message("newest", "d".repeat(5_000)),
    ];
    let mut record = later_record();
    keep_earlier_author_messages(&mut record, &messages);
    let selected: Vec<_> = record
        .selected
        .iter()
        .map(|item| {
            (
                item.source_class,
                item.source_version.as_str(),
                item.projection_mode,
                item.token_count,
                is_earlier_author_message(&record, item),
            )
        })
        .collect();

    assert_eq!(
        (
            selected,
            record.considered[1..3]
                .iter()
                .map(|item| (item.source_version.as_str(), item.eligible))
                .collect::<Vec<_>>(),
            record.operation_requirement.earlier_author_messages.clone(),
        ),
        (
            vec![
                (
                    ContextSourceClass::ConversationSummary,
                    "compaction-1",
                    ProjectionMode::DerivedSummary,
                    8,
                    false,
                ),
                (
                    ContextSourceClass::AuthorInstruction,
                    "second",
                    ProjectionMode::ExactRequired,
                    15_000,
                    true,
                ),
                (
                    ContextSourceClass::AuthorInstruction,
                    "newest",
                    ProjectionMode::ExactRequired,
                    EARLIER_AUTHOR_MESSAGES_TOKEN_LIMIT - 15_000,
                    true,
                ),
                (
                    ContextSourceClass::AuthorInstruction,
                    "snapshot-1",
                    ProjectionMode::ExactRequired,
                    11,
                    false,
                ),
                (
                    ContextSourceClass::WorkingTarget,
                    "rev-1",
                    ProjectionMode::ExactRequired,
                    8,
                    false,
                ),
            ],
            vec![("second", true), ("newest", true)],
            Some(vec![messages[2].source.clone(), messages[3].source.clone()]),
        )
    );
}

#[test]
fn a_newest_message_above_the_limit_leaves_out_each_message() {
    let mut record = later_record();
    let unchanged = record.clone();
    keep_earlier_author_messages(
        &mut record,
        &[
            message("short", "a".to_owned()),
            message("long", "e".repeat(20_001)),
        ],
    );

    assert_eq!(record, unchanged);
}
