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
