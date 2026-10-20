use crate::assemble_context::{ConsideredSource, RejectedSource};
use crate::{
    ContextBlockReason, ContextSourceClass, ContextSufficiency, CurrentPassageAssembly,
    CurrentPassageAssemblyRecord, InstructionBindingInput, PassageContextTarget, RejectionReason,
    assemble_passage_collection,
};

fn passage(chapter_id: &str, body: &str) -> CurrentPassageAssembly {
    CurrentPassageAssembly {
        operation_requirement_id: "req-1".to_owned(),
        input_snapshot_id: "snap-1".to_owned(),
        run_id: "run-1".to_owned(),
        owner_user_id: "user-1".to_owned(),
        project_id: "proj-1".to_owned(),
        author_message: "Help with these passages.".to_owned(),
        chapter_id: chapter_id.to_owned(),
        chapter_revision_id: Some(format!("{chapter_id}-rev")),
        proposal_target_block_ids: Some(vec![format!("{chapter_id}-block")]),
        chapter_body: body.to_owned(),
        instruction: InstructionBindingInput::Absent,
        destination_identity: "dest-1".to_owned(),
    }
}

fn target(chapter_id: &str) -> PassageContextTarget {
    PassageContextTarget {
        chapter_id: chapter_id.to_owned(),
        base_authoritative_revision_id: format!("{chapter_id}-rev"),
        manuscript_block_ids: vec![format!("{chapter_id}-block")],
    }
}

fn ineligible(source_version: &str, token_count: u64) -> ConsideredSource {
    ConsideredSource {
        source_class: ContextSourceClass::WorkingTarget,
        source_version: source_version.to_owned(),
        token_count,
        eligible: false,
    }
}

fn rejected(source_version: &str, token_count: u64, reason: RejectionReason) -> RejectedSource {
    RejectedSource {
        source_class: ContextSourceClass::WorkingTarget,
        source_version: source_version.to_owned(),
        token_count,
        reason,
    }
}

fn working_target_outcome(
    record: &CurrentPassageAssemblyRecord,
) -> (
    Vec<ConsideredSource>,
    usize,
    Vec<RejectedSource>,
    ContextSufficiency,
) {
    let is_target = |class: ContextSourceClass| class == ContextSourceClass::WorkingTarget;
    (
        record
            .considered
            .iter()
            .filter(|item| is_target(item.source_class))
            .cloned()
            .collect(),
        record
            .selected
            .iter()
            .filter(|item| is_target(item.source_class))
            .count(),
        record.rejected.clone(),
        record.sufficiency.clone(),
    )
}

#[test]
fn a_collection_above_the_limit_rejects_each_passage_with_its_own_count() {
    let first = passage("ch-1", &"a".repeat(/*n*/ 16_001));
    let second = passage("ch-2", &"b".repeat(/*n*/ 16_000));
    let record = assemble_passage_collection(
        &first,
        vec![target("ch-1"), target("ch-2")],
        &[first.clone(), second],
    );

    assert_eq!(
        working_target_outcome(&record),
        (
            vec![
                ineligible("ch-1-rev", 16_001),
                ineligible("ch-2-rev", 16_000)
            ],
            0,
            vec![
                rejected(
                    "ch-1-rev",
                    16_001,
                    RejectionReason::OverCollectionTokenLimit
                ),
                rejected(
                    "ch-2-rev",
                    16_000,
                    RejectionReason::OverCollectionTokenLimit
                ),
            ],
            ContextSufficiency::Blocked {
                reasons: vec![ContextBlockReason::ExactRequiredOverLimit {
                    source_class: ContextSourceClass::WorkingTarget,
                }],
            },
        )
    );
}

#[test]
fn an_empty_target_list_rejects_one_unavailable_working_target() {
    let source = passage("ch-1", "The tide turned.");
    let record = assemble_passage_collection(&source, Vec::new(), &[]);

    assert_eq!(
        working_target_outcome(&record),
        (
            vec![ineligible("unavailable", 0)],
            0,
            vec![rejected(
                "unavailable",
                0,
                RejectionReason::WorkingTargetRevisionUnavailable
            )],
            ContextSufficiency::Blocked {
                reasons: vec![ContextBlockReason::WorkingTargetRevisionUnavailable],
            },
        )
    );
}
