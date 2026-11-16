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
        proposal_target_block_ids: Some(vec!["block-1".to_owned()]),
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
        proposal_target_block_ids: input.proposal_target_block_ids.clone(),
        passage_targets: None,
        candidate_target: None,
        selection_references: None,
        ordinary_resolution: None,
        tool_results: None,
        earlier_author_messages: None,
        skill_requirements: None,
        skill_catalog: None,
        memory_revision: None,
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
fn selects_a_working_target_at_the_item_limit() {
    let mut input = complete_input();
    input.chapter_body = "a".repeat(/*n*/ 32_000);
    let record = assemble_current_passage_context(&input);
    assert_eq!(record.sufficiency, ContextSufficiency::Complete);
    assert_eq!(
        record.selected[1],
        SelectedProjection {
            source_class: ContextSourceClass::WorkingTarget,
            source_version: "rev-1".to_owned(),
            projection_mode: ProjectionMode::ExactRequired,
            token_count: 32_000,
            content: input.chapter_body,
        }
    );
}

#[test]
fn blocks_exact_required_working_target_over_the_item_limit() {
    let mut input = complete_input();
    input.chapter_body = "a".repeat(CONTEXT_ITEM_TOKEN_LIMIT as usize + 1);
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
                    token_count: CONTEXT_ITEM_TOKEN_LIMIT + 1,
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
                token_count: CONTEXT_ITEM_TOKEN_LIMIT + 1,
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

#[test]
fn each_skill_source_class_roundtrips_with_its_persisted_name() {
    let mut record = assemble_current_passage_context(&complete_input());
    for source_class in [
        ContextSourceClass::SkillCatalog,
        ContextSourceClass::SkillInstruction,
        ContextSourceClass::SkillResource,
    ] {
        record.selected.push(SelectedProjection {
            source_class,
            source_version: "sha256:skill".to_owned(),
            projection_mode: ProjectionMode::ExactRequired,
            token_count: 3,
            content: "Skill <text> & \"quotes\"".to_owned(),
        });
    }
    let payload = encode_assembly_record(&record);

    assert_eq!(
        payload["selected"]
            .as_array()
            .expect("selected")
            .iter()
            .map(|item| item["source_class"].as_str().expect("class"))
            .collect::<Vec<_>>(),
        [
            "author_instruction",
            "working_target",
            "skill_catalog",
            "skill_instruction",
            "skill_resource"
        ]
    );
    assert_eq!(decode_assembly_record(&payload), Some(record));
}

/// The Stage 7 source classes of ADR 0095 section 1 and ADR 0094 sections 4 to 6, with their
/// persisted names.
const STAGE_7_SOURCE_CLASSES: [(ContextSourceClass, &str); 10] = [
    (ContextSourceClass::ChapterList, "chapter_list"),
    (
        ContextSourceClass::ManuscriptReadResult,
        "manuscript_read_result",
    ),
    (
        ContextSourceClass::ManuscriptSearchResult,
        "manuscript_search_result",
    ),
    (ContextSourceClass::MemoryIndex, "memory_index"),
    (ContextSourceClass::MemoryReadResult, "memory_read_result"),
    (
        ContextSourceClass::MemorySearchResult,
        "memory_search_result",
    ),
    (
        ContextSourceClass::MemoryCaptureInput,
        "memory_capture_input",
    ),
    (ContextSourceClass::MemoryDeleteMarks, "memory_delete_marks"),
    (ContextSourceClass::MemoryEntries, "memory_entries"),
    (
        ContextSourceClass::MemoryObservations,
        "memory_observations",
    ),
];

#[test]
fn each_stage_7_source_class_and_purpose_roundtrips_with_its_persisted_name() {
    let mut record = assemble_current_passage_context(&complete_input());
    record.selected = STAGE_7_SOURCE_CLASSES
        .map(|(source_class, _)| SelectedProjection {
            source_class,
            source_version: "sha256:source".to_owned(),
            projection_mode: ProjectionMode::ExactRequired,
            token_count: 3,
            content: "Data <text> & \"quotes\"".to_owned(),
        })
        .to_vec();
    let with_purpose = |purpose| {
        let mut record = record.clone();
        record.operation_requirement.purpose = purpose;
        record
    };
    let records = [
        with_purpose(ContextPurpose::MemoryCapture),
        with_purpose(ContextPurpose::MemoryConsolidation),
    ];
    let payloads = records
        .clone()
        .map(|record| encode_assembly_record(&record));

    assert_eq!(
        (
            payloads[0]["selected"]
                .as_array()
                .expect("selected")
                .iter()
                .map(|item| item["source_class"].as_str().expect("class"))
                .collect::<Vec<_>>(),
            payloads
                .clone()
                .map(|payload| payload["operation_requirement"]["purpose"].clone()),
        ),
        (
            STAGE_7_SOURCE_CLASSES.map(|(_, name)| name).to_vec(),
            [
                serde_json::json!("memory_capture"),
                serde_json::json!("memory_consolidation")
            ],
        )
    );
    assert_eq!(
        payloads.map(|payload| decode_assembly_record(&payload)),
        records.map(Some)
    );
}

#[test]
fn the_host_schedule_cause_roundtrips_with_its_persisted_name() {
    let mut record = assemble_current_passage_context(&complete_input());
    record.operation_requirement.purpose = ContextPurpose::MemoryCapture;
    record.operation_requirement.cause = ContextCause::HostSchedule;
    let payload = encode_assembly_record(&record);
    let author = encode_assembly_record(&assemble_current_passage_context(&complete_input()));
    let mut unknown = payload.clone();
    unknown["operation_requirement"]["cause"] = serde_json::json!("host_timer");

    assert_eq!(
        (
            payload["operation_requirement"]["cause"].clone(),
            author["operation_requirement"].get("cause"),
            decode_assembly_record(&payload),
            decode_assembly_record(&unknown),
        ),
        (serde_json::json!("host_schedule"), None, Some(record), None)
    );
}

#[test]
fn historical_assembly_without_block_selection_remains_readable() {
    let record = assemble_current_passage_context(&complete_input());
    let mut payload = encode_assembly_record(&record);
    payload["operation_requirement"]
        .as_object_mut()
        .expect("requirement")
        .remove("proposal_target_block_ids");
    let decoded = decode_assembly_record(&payload).expect("historical payload");
    assert_eq!(
        decoded.operation_requirement.proposal_target_block_ids,
        None
    );
    assert_eq!(decoded.selected, record.selected);
    assert_eq!(
        decode_assembly_record(&encode_assembly_record(&decoded)),
        Some(decoded)
    );
}

#[test]
fn historical_assembly_keeps_the_item_limit_of_its_assembly() {
    let record = assemble_current_passage_context(&complete_input());
    let mut payload = encode_assembly_record(&record);
    payload["operation_requirement"]
        .as_object_mut()
        .expect("requirement")
        .remove("item_token_limit");
    let decoded = decode_assembly_record(&payload).expect("historical payload");
    assert_eq!(
        (
            record.operation_requirement.item_token_limit,
            decoded.operation_requirement.item_token_limit
        ),
        (32_000, 10_000)
    );
}

#[test]
fn present_invalid_block_selection_is_not_historical_absence() {
    let record = assemble_current_passage_context(&complete_input());
    for invalid in [
        serde_json::Value::Null,
        serde_json::json!({"block": "block-1"}),
        serde_json::json!(["block-1", 2]),
    ] {
        let mut payload = encode_assembly_record(&record);
        payload["operation_requirement"]["proposal_target_block_ids"] = invalid;
        assert_eq!(decode_assembly_record(&payload), None);
    }
}
