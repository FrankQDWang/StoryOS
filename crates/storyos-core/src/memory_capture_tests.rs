use super::*;
use crate::{decode_assembly_record, encode_assembly_record};

fn facts(input: &str) -> CaptureAdmissionFacts<'_> {
    CaptureAdmissionFacts {
        binding: true,
        assistance_available: true,
        grant_scope: Some("project_writing_assistance_v5"),
        input,
    }
}

/// One row for each refusal, in the order of the checks, and the admitted start.
#[test]
fn the_start_of_a_capture_is_refused_in_the_order_of_its_facts() {
    let over = "字".repeat(usize::try_from(CONTEXT_ITEM_TOKEN_LIMIT).unwrap() + 1);
    let at_limit = "字".repeat(usize::try_from(CONTEXT_ITEM_TOKEN_LIMIT).unwrap());
    let cases = [
        (
            CaptureAdmissionFacts {
                binding: false,
                assistance_available: false,
                ..facts(&over)
            },
            Err(CaptureRefusal::NoModelUseBinding),
        ),
        (
            CaptureAdmissionFacts {
                assistance_available: false,
                grant_scope: None,
                ..facts(&over)
            },
            Err(CaptureRefusal::AssistanceUnavailable),
        ),
        (
            CaptureAdmissionFacts {
                grant_scope: Some("project_writing_assistance_v4"),
                ..facts(&over)
            },
            Err(CaptureRefusal::PurposeNotGranted),
        ),
        (
            CaptureAdmissionFacts {
                grant_scope: Some("research_tool_v1"),
                ..facts("对话。")
            },
            Err(CaptureRefusal::PurposeNotGranted),
        ),
        (facts(&over), Err(CaptureRefusal::InputOverItemLimit)),
        (facts(&at_limit), Ok(())),
    ];

    for (facts, admission) in cases {
        assert_eq!(admit_memory_capture(&facts), admission);
    }
    assert_eq!(
        CaptureRefusal::InputOverItemLimit.label(),
        "input_over_item_limit"
    );
}

#[test]
fn the_assembly_of_a_capture_has_one_exact_input_item_and_no_chapter() {
    let record = assemble_memory_capture_context(&MemoryCaptureAssembly {
        operation_requirement_id: "requirement".to_owned(),
        input_snapshot_id: "snapshot".to_owned(),
        run_id: "run".to_owned(),
        owner_user_id: "owner".to_owned(),
        project_id: "project".to_owned(),
        input: "林舟上了船。".to_owned(),
        destination_identity: "destination".to_owned(),
    });
    let encoded = encode_assembly_record(&record);

    assert_eq!(
        (
            &encoded["operation_requirement"]["purpose"],
            &encoded["operation_requirement"]["cause"],
            &encoded["operation_requirement"]["chapter_id"],
            &encoded["selected"][0]["source_class"],
            &encoded["selected"][0]["projection_mode"],
            &encoded["selected"][0]["content"],
            encoded["selected"].as_array().map(Vec::len),
        ),
        (
            &serde_json::json!("memory_capture"),
            &serde_json::json!("host_schedule"),
            &serde_json::Value::Null,
            &serde_json::json!("memory_capture_input"),
            &serde_json::json!("exact_required"),
            &serde_json::json!("林舟上了船。"),
            Some(1),
        )
    );
    assert_eq!(decode_assembly_record(&encoded), Some(record));
}

/// The outcome of a capture from the facts of its Run: the last RunStep with no settled output
/// call gives `step_limit`, a Run that ends in another way fails the capture with its reason, and
/// a Run that continues keeps the capture running.
#[test]
fn the_outcome_of_a_capture_comes_from_the_facts_of_its_run() {
    let status = |status, reason| capture_end(CaptureRunEnd::Status { status, reason });
    let failed = |reason| {
        Some(CaptureEnd {
            outcome: crate::CaptureOutcome::Failed,
            failure_reason: Some(reason),
        })
    };
    assert_eq!(
        [
            capture_end(CaptureRunEnd::LastRunStepEnded),
            status("queued", None),
            status("claimed", None),
            status("refused", Some("disclosure_outside_grant")),
            status("refused", Some("destination_refused")),
            status("waiting", Some("unknown_result")),
            status("paused", Some("request_changed")),
            status("completed", None),
            status("cancelled", None),
        ],
        [
            Some(CaptureEnd {
                outcome: crate::CaptureOutcome::StepLimit,
                failure_reason: None
            }),
            None,
            None,
            failed("disclosure_outside_grant"),
            failed("destination_refused"),
            failed("unknown_result"),
            failed("request_changed"),
            failed("no_agent_decision"),
            failed("cancelled"),
        ]
    );
    assert_eq!(
        [
            crate::CaptureOutcome::StepLimit,
            crate::CaptureOutcome::Failed
        ]
        .map(crate::CaptureOutcome::label),
        ["step_limit", "failed"]
    );
}

/// The first RunStep of a capture selects the Memory Index at the head revision of its start, the
/// Chapter list, and the delete marks after its input, also when no Conversation Memory Settings
/// permit a read. The next RunStep keeps them. A Project with none of them adds no item.
#[test]
fn a_capture_gets_its_reference_items_after_its_input_and_keeps_them() {
    let assembly = |input: &str| {
        assemble_memory_capture_context(&MemoryCaptureAssembly {
            operation_requirement_id: "requirement".to_owned(),
            input_snapshot_id: "snapshot".to_owned(),
            run_id: "run".to_owned(),
            owner_user_id: "owner".to_owned(),
            project_id: "project".to_owned(),
            input: input.to_owned(),
            destination_identity: "destination".to_owned(),
        })
    };
    let index = [crate::MemoryIndexEntry {
        number: 1,
        entry_type: crate::MemoryEntryType::Character,
        title: "林舟".to_owned(),
        description: "撑船的人。".to_owned(),
    }];
    let chapters = [crate::ChapterListEntry {
        volume_order: 1,
        volume_title: "第一卷".to_owned(),
        chapter_id: "chapter".to_owned(),
        title: "雨夜".to_owned(),
        length: 6,
    }];
    let marks = [crate::MemoryDeleteMark {
        entry_type: crate::MemoryEntryType::Other,
        title: "旧船".to_owned(),
        description: "作者不要的内容。".to_owned(),
    }];
    let references = |memory_head| CaptureReferences {
        memory_head,
        memory_index: &index,
        chapters: &chapters,
        delete_marks: &marks,
    };
    let classes = |record: &CurrentPassageAssemblyRecord| {
        (
            record.operation_requirement.memory_revision,
            record
                .selected
                .iter()
                .map(|item| item.source_class)
                .collect::<Vec<_>>(),
        )
    };
    let mut first = assembly("林舟上了船。");
    assemble_capture_references(&mut first, &references(3));
    let mut next = assembly("林舟上了船。");
    keep_capture_references(&mut next, &first);
    let mut empty = assembly("林舟上了船。");
    assemble_capture_references(
        &mut empty,
        &CaptureReferences {
            memory_head: 0,
            memory_index: &[],
            chapters: &[],
            delete_marks: &[],
        },
    );
    let all = vec![
        ContextSourceClass::MemoryCaptureInput,
        ContextSourceClass::MemoryIndex,
        ContextSourceClass::ChapterList,
        ContextSourceClass::MemoryDeleteMarks,
    ];

    assert_eq!(
        [classes(&first), classes(&next), classes(&empty)],
        [
            (Some(3), all.clone()),
            (Some(3), all),
            (None, vec![ContextSourceClass::MemoryCaptureInput]),
        ]
    );
    assert_eq!(first.selected[1..], next.selected[1..]);
    assert_eq!(
        decode_assembly_record(&encode_assembly_record(&first)),
        Some(first)
    );
}
