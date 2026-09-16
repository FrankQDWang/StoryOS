use super::{
    CONTEXT_ITEM_TOKEN_LIMIT, ConsideredSource, ContextBlockReason, ContextCause, ContextPurpose,
    ContextSourceClass, ContextSufficiency, CurrentPassageAssembly, CurrentPassageAssemblyRecord,
    DestinationIo, HostControlRecord, InstructionBindingInput, ManifestCommit,
    OperationRequirementRecord, ProjectionMode, RejectedSource, RejectionReason,
    SelectedProjection, TOKEN_COUNTING_ALGORITHM_REVISION, TOKEN_COUNTING_PROFILE_REVISION,
    assemble_current_passage_context, decode_assembly_record, encode_assembly_record,
};

fn complete_input() -> CurrentPassageAssembly {
    CurrentPassageAssembly {
        operation_requirement_id: "req-1".to_owned(),
        input_snapshot_id: "snap-1".to_owned(),
        run_id: "run-1".to_owned(),
        owner_user_id: "user-1".to_owned(),
        project_id: "proj-1".to_owned(),
        author_message: "Help with this passage.".to_owned(),
        chapter_id: "ch-1".to_owned(),
        chapter_revision_id: Some("rev-1".to_owned()),
        chapter_body: "Once upon a time.".to_owned(),
        instruction: InstructionBindingInput::Absent,
        destination_identity: "dest-1".to_owned(),
    }
}

fn requirement(input: &CurrentPassageAssembly) -> OperationRequirementRecord {
    OperationRequirementRecord {
        operation_requirement_id: input.operation_requirement_id.clone(),
        input_snapshot_id: input.input_snapshot_id.clone(),
        run_id: input.run_id.clone(),
        owner_user_id: input.owner_user_id.clone(),
        project_id: input.project_id.clone(),
        purpose: ContextPurpose::CurrentPassageAssistance,
        cause: ContextCause::AuthorRequest,
        chapter_id: input.chapter_id.clone(),
        chapter_revision_id: input.chapter_revision_id.clone(),
        instruction: input.instruction.clone(),
        destination_identity: input.destination_identity.clone(),
        item_token_limit: CONTEXT_ITEM_TOKEN_LIMIT,
        token_counting_profile_revision: TOKEN_COUNTING_PROFILE_REVISION.to_owned(),
    }
}

#[test]
fn assembles_complete_current_passage_context_without_destination_io() {
    let input = complete_input();
    assert_eq!(
        assemble_current_passage_context(&input),
        CurrentPassageAssemblyRecord {
            operation_requirement: requirement(&input),
            sufficiency: ContextSufficiency::Complete,
            considered: vec![
                ConsideredSource {
                    source_class: ContextSourceClass::HostControl,
                    source_version: "req-1".to_owned(),
                    token_count: 0,
                    eligible: true,
                },
                ConsideredSource {
                    source_class: ContextSourceClass::AuthorInstruction,
                    source_version: "snap-1".to_owned(),
                    token_count: 23,
                    eligible: true,
                },
                ConsideredSource {
                    source_class: ContextSourceClass::WorkingTarget,
                    source_version: "rev-1".to_owned(),
                    token_count: 17,
                    eligible: true,
                },
                ConsideredSource {
                    source_class: ContextSourceClass::InstructionBinding,
                    source_version: "absent".to_owned(),
                    token_count: 0,
                    eligible: true,
                },
            ],
            selected: vec![
                SelectedProjection {
                    source_class: ContextSourceClass::AuthorInstruction,
                    source_version: "snap-1".to_owned(),
                    projection_mode: ProjectionMode::ExactRequired,
                    token_count: 23,
                    content: "Help with this passage.".to_owned(),
                },
                SelectedProjection {
                    source_class: ContextSourceClass::WorkingTarget,
                    source_version: "rev-1".to_owned(),
                    projection_mode: ProjectionMode::ExactRequired,
                    token_count: 17,
                    content: "Once upon a time.".to_owned(),
                },
            ],
            rejected: Vec::new(),
            host_control: HostControlRecord {
                distinct_from_destination: true,
                destination_visible: false,
            },
            token_counting_profile_revision: TOKEN_COUNTING_PROFILE_REVISION.to_owned(),
            token_counting_algorithm_revision: TOKEN_COUNTING_ALGORITHM_REVISION.to_owned(),
            manifests: ManifestCommit {
                assembly: true,
                destination_and_outbound: false,
            },
            destination_io: DestinationIo::None,
        }
    );
}

#[test]
fn blocks_exact_required_working_target_over_the_item_limit() {
    let mut input = complete_input();
    input.chapter_body = "a".repeat(10_001);
    assert_eq!(
        assemble_current_passage_context(&input),
        CurrentPassageAssemblyRecord {
            operation_requirement: requirement(&input),
            sufficiency: ContextSufficiency::Blocked {
                reasons: vec![ContextBlockReason::ExactRequiredOverLimit {
                    source_class: ContextSourceClass::WorkingTarget,
                }],
            },
            considered: vec![
                ConsideredSource {
                    source_class: ContextSourceClass::HostControl,
                    source_version: "req-1".to_owned(),
                    token_count: 0,
                    eligible: true,
                },
                ConsideredSource {
                    source_class: ContextSourceClass::AuthorInstruction,
                    source_version: "snap-1".to_owned(),
                    token_count: 23,
                    eligible: true,
                },
                ConsideredSource {
                    source_class: ContextSourceClass::WorkingTarget,
                    source_version: "rev-1".to_owned(),
                    token_count: 10_001,
                    eligible: false,
                },
                ConsideredSource {
                    source_class: ContextSourceClass::InstructionBinding,
                    source_version: "absent".to_owned(),
                    token_count: 0,
                    eligible: true,
                },
            ],
            selected: vec![SelectedProjection {
                source_class: ContextSourceClass::AuthorInstruction,
                source_version: "snap-1".to_owned(),
                projection_mode: ProjectionMode::ExactRequired,
                token_count: 23,
                content: "Help with this passage.".to_owned(),
            }],
            rejected: vec![RejectedSource {
                source_class: ContextSourceClass::WorkingTarget,
                source_version: "rev-1".to_owned(),
                token_count: 10_001,
                reason: RejectionReason::OverItemTokenLimit,
            }],
            host_control: HostControlRecord {
                distinct_from_destination: true,
                destination_visible: false,
            },
            token_counting_profile_revision: TOKEN_COUNTING_PROFILE_REVISION.to_owned(),
            token_counting_algorithm_revision: TOKEN_COUNTING_ALGORITHM_REVISION.to_owned(),
            manifests: ManifestCommit {
                assembly: true,
                destination_and_outbound: false,
            },
            destination_io: DestinationIo::None,
        }
    );
}

#[test]
fn blocks_an_unavailable_required_instruction_revision() {
    let mut input = complete_input();
    input.instruction = InstructionBindingInput::RequiredRevision {
        revision_id: "instr-9".to_owned(),
        available: false,
    };
    assert_eq!(
        assemble_current_passage_context(&input),
        CurrentPassageAssemblyRecord {
            operation_requirement: requirement(&input),
            sufficiency: ContextSufficiency::Blocked {
                reasons: vec![ContextBlockReason::RequiredInstructionRevisionUnavailable],
            },
            considered: vec![
                ConsideredSource {
                    source_class: ContextSourceClass::HostControl,
                    source_version: "req-1".to_owned(),
                    token_count: 0,
                    eligible: true,
                },
                ConsideredSource {
                    source_class: ContextSourceClass::AuthorInstruction,
                    source_version: "snap-1".to_owned(),
                    token_count: 23,
                    eligible: true,
                },
                ConsideredSource {
                    source_class: ContextSourceClass::WorkingTarget,
                    source_version: "rev-1".to_owned(),
                    token_count: 17,
                    eligible: true,
                },
                ConsideredSource {
                    source_class: ContextSourceClass::InstructionBinding,
                    source_version: "instr-9".to_owned(),
                    token_count: 0,
                    eligible: false,
                },
            ],
            selected: vec![
                SelectedProjection {
                    source_class: ContextSourceClass::AuthorInstruction,
                    source_version: "snap-1".to_owned(),
                    projection_mode: ProjectionMode::ExactRequired,
                    token_count: 23,
                    content: "Help with this passage.".to_owned(),
                },
                SelectedProjection {
                    source_class: ContextSourceClass::WorkingTarget,
                    source_version: "rev-1".to_owned(),
                    projection_mode: ProjectionMode::ExactRequired,
                    token_count: 17,
                    content: "Once upon a time.".to_owned(),
                },
            ],
            rejected: vec![RejectedSource {
                source_class: ContextSourceClass::InstructionBinding,
                source_version: "instr-9".to_owned(),
                token_count: 0,
                reason: RejectionReason::RequiredRevisionUnavailable,
            }],
            host_control: HostControlRecord {
                distinct_from_destination: true,
                destination_visible: false,
            },
            token_counting_profile_revision: TOKEN_COUNTING_PROFILE_REVISION.to_owned(),
            token_counting_algorithm_revision: TOKEN_COUNTING_ALGORITHM_REVISION.to_owned(),
            manifests: ManifestCommit {
                assembly: true,
                destination_and_outbound: false,
            },
            destination_io: DestinationIo::None,
        }
    );
}

#[test]
fn blocks_a_working_target_without_an_exact_revision() {
    let mut input = complete_input();
    input.chapter_revision_id = None;
    input.chapter_body.clear();
    assert_eq!(
        assemble_current_passage_context(&input),
        CurrentPassageAssemblyRecord {
            operation_requirement: requirement(&input),
            sufficiency: ContextSufficiency::Blocked {
                reasons: vec![ContextBlockReason::WorkingTargetRevisionUnavailable],
            },
            considered: vec![
                ConsideredSource {
                    source_class: ContextSourceClass::HostControl,
                    source_version: "req-1".to_owned(),
                    token_count: 0,
                    eligible: true,
                },
                ConsideredSource {
                    source_class: ContextSourceClass::AuthorInstruction,
                    source_version: "snap-1".to_owned(),
                    token_count: 23,
                    eligible: true,
                },
                ConsideredSource {
                    source_class: ContextSourceClass::WorkingTarget,
                    source_version: "unavailable".to_owned(),
                    token_count: 0,
                    eligible: false,
                },
                ConsideredSource {
                    source_class: ContextSourceClass::InstructionBinding,
                    source_version: "absent".to_owned(),
                    token_count: 0,
                    eligible: true,
                },
            ],
            selected: vec![SelectedProjection {
                source_class: ContextSourceClass::AuthorInstruction,
                source_version: "snap-1".to_owned(),
                projection_mode: ProjectionMode::ExactRequired,
                token_count: 23,
                content: "Help with this passage.".to_owned(),
            }],
            rejected: vec![RejectedSource {
                source_class: ContextSourceClass::WorkingTarget,
                source_version: "unavailable".to_owned(),
                token_count: 0,
                reason: RejectionReason::WorkingTargetRevisionUnavailable,
            }],
            host_control: HostControlRecord {
                distinct_from_destination: true,
                destination_visible: false,
            },
            token_counting_profile_revision: TOKEN_COUNTING_PROFILE_REVISION.to_owned(),
            token_counting_algorithm_revision: TOKEN_COUNTING_ALGORITHM_REVISION.to_owned(),
            manifests: ManifestCommit {
                assembly: true,
                destination_and_outbound: false,
            },
            destination_io: DestinationIo::None,
        }
    );
}

#[test]
fn persisted_assembly_record_roundtrips() {
    let record = assemble_current_passage_context(&complete_input());
    assert_eq!(
        decode_assembly_record(&encode_assembly_record(&record)).expect("payload"),
        record
    );
}
