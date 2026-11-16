use super::{BoundInstruction, assemble_instruction_binding, project_instruction_binding};
use crate::assemble_context::{
    CONTEXT_ITEM_TOKEN_LIMIT, ConsideredSource, ContextBlockReason, ContextSourceClass,
    ContextSufficiency, CurrentPassageAssembly, CurrentPassageAssemblyRecord,
    InstructionBindingInput, ProjectionMode, RejectedSource, RejectionReason, SelectedProjection,
    assemble_current_passage_context, decode_assembly_record, encode_assembly_record,
};

fn revision(text: &str) -> BoundInstruction {
    BoundInstruction {
        revision_id: "instruction-1".to_owned(),
        text: text.to_owned(),
    }
}

fn assembled() -> CurrentPassageAssemblyRecord {
    assemble_current_passage_context(&CurrentPassageAssembly {
        operation_requirement_id: "req-1".to_owned(),
        input_snapshot_id: "snap-1".to_owned(),
        run_id: "run-1".to_owned(),
        owner_user_id: "user-1".to_owned(),
        project_id: "proj-1".to_owned(),
        author_message: "Help with this passage.".to_owned(),
        chapter_id: "ch-1".to_owned(),
        chapter_revision_id: Some("rev-1".to_owned()),
        proposal_target_block_ids: Some(vec!["block-1".to_owned()]),
        chapter_body: "Once upon a time.".to_owned(),
        instruction: InstructionBindingInput::Absent,
        destination_identity: "dest-1".to_owned(),
    })
}

/// The source class of each selected item of `record`, in item order.
fn order(record: &CurrentPassageAssemblyRecord) -> Vec<ContextSourceClass> {
    record
        .selected
        .iter()
        .map(|item| item.source_class)
        .collect()
}

fn instruction_source(record: &CurrentPassageAssemblyRecord) -> Option<&ConsideredSource> {
    record
        .considered
        .iter()
        .find(|source| source.source_class == ContextSourceClass::InstructionBinding)
}

#[test]
fn only_a_current_revision_with_text_other_than_white_space_binds() {
    let cases = [
        None,
        Some(revision("")),
        // U+0085, U+00A0, U+2028, and U+3000 have the Unicode property White_Space.
        Some(revision(" \t\n\r\u{85}\u{a0}\u{2028}\u{3000}")),
        // U+200B does not have the property White_Space.
        Some(revision("\u{200b}")),
        Some(revision(" Write in the past tense. ")),
    ];

    assert_eq!(
        cases
            .iter()
            .map(|current| project_instruction_binding(current.as_ref()))
            .collect::<Vec<_>>(),
        vec![
            None,
            None,
            None,
            Some("instruction-1".to_owned()),
            Some("instruction-1".to_owned()),
        ]
    );
}

#[test]
fn a_bound_revision_is_one_exact_item_after_the_author_instruction() {
    let text = " Write in the past tense.\n";
    let mut record = assembled();
    assemble_instruction_binding(&mut record, Some(&revision(text)));
    let mut unbound = assembled();
    assemble_instruction_binding(&mut unbound, None);

    assert_eq!(
        (
            record.operation_requirement.instruction.clone(),
            instruction_source(&record).cloned(),
            order(&record),
            record.selected[1].clone(),
            record.sufficiency.clone(),
            decode_assembly_record(&encode_assembly_record(&record)),
            unbound,
        ),
        (
            InstructionBindingInput::RequiredRevision {
                revision_id: "instruction-1".to_owned(),
                available: true,
            },
            Some(ConsideredSource {
                source_class: ContextSourceClass::InstructionBinding,
                source_version: "instruction-1".to_owned(),
                token_count: 26,
                eligible: true,
            }),
            vec![
                ContextSourceClass::AuthorInstruction,
                ContextSourceClass::InstructionBinding,
                ContextSourceClass::WorkingTarget,
            ],
            SelectedProjection {
                source_class: ContextSourceClass::InstructionBinding,
                source_version: "instruction-1".to_owned(),
                projection_mode: ProjectionMode::ExactRequired,
                token_count: 26,
                content: text.to_owned(),
            },
            ContextSufficiency::Complete,
            Some(record.clone()),
            assembled(),
        )
    );
}

#[test]
fn a_bound_text_above_the_item_limit_blocks_the_assembly() {
    let text = "字".repeat(CONTEXT_ITEM_TOKEN_LIMIT as usize + 1);
    let mut record = assembled();
    assemble_instruction_binding(&mut record, Some(&revision(&text)));

    assert_eq!(
        (
            order(&record),
            record.rejected.clone(),
            record.sufficiency.clone()
        ),
        (
            vec![
                ContextSourceClass::AuthorInstruction,
                ContextSourceClass::WorkingTarget,
            ],
            vec![RejectedSource {
                source_class: ContextSourceClass::InstructionBinding,
                source_version: "instruction-1".to_owned(),
                token_count: CONTEXT_ITEM_TOKEN_LIMIT + 1,
                reason: RejectionReason::OverItemTokenLimit,
            }],
            ContextSufficiency::Blocked {
                reasons: vec![ContextBlockReason::ExactRequiredOverLimit {
                    source_class: ContextSourceClass::InstructionBinding,
                }],
            },
        )
    );
}

#[test]
fn the_expected_input_counts_the_item_when_the_request_sends_the_current_input() {
    let mut record = assembled();
    assemble_instruction_binding(&mut record, Some(&revision("Write in the past tense.")));
    let sent = |record: &CurrentPassageAssemblyRecord| {
        crate::CompactionTrigger::from_facts(&crate::CompactionRequestFacts {
            model_registration_revision: crate::AGENT_PLAN_REGISTRATION.revision,
            budget_bounds: crate::BudgetBounds::AgentPlanRequestV2.as_str(),
            author_message: "Help with this passage.",
            decision: Some(record),
            declared_target_texts: &[],
            prior_reported_tokens: Some(0),
            prior_decision_id: None,
            prior_input: crate::PriorInput::Current,
            decision_position: "0",
            compacted_earlier: false,
        })
        .sent_input_tokens
    };

    // The author message has 23 tokens, the working text 17, and the instruction 24.
    assert_eq!((sent(&assembled()), sent(&record)), (23 + 17, 23 + 17 + 24));
}
