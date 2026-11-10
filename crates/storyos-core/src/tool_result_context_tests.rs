use super::*;
use crate::{
    CurrentPassageAssembly, InstructionBindingInput, assemble_current_passage_context,
    decode_assembly_record, encode_assembly_record,
};

fn passage() -> CurrentPassageAssemblyRecord {
    assemble_current_passage_context(&CurrentPassageAssembly {
        operation_requirement_id: "req-2".to_owned(),
        input_snapshot_id: "snap-2".to_owned(),
        run_id: "run-1".to_owned(),
        owner_user_id: "user-1".to_owned(),
        project_id: "proj-1".to_owned(),
        author_message: "When was the harbour built?".to_owned(),
        chapter_id: "ch-1".to_owned(),
        chapter_revision_id: Some("ch-1-rev".to_owned()),
        proposal_target_block_ids: Some(vec!["ch-1-block".to_owned()]),
        chapter_body: "The harbour was old.".to_owned(),
        instruction: InstructionBindingInput::Absent,
        destination_identity: "dest-1".to_owned(),
    })
}

fn candidate<'a>(
    tool_call_id: &'a str,
    state: ToolCallState,
    result_text: Option<&'a str>,
) -> ToolResultCandidate<'a> {
    ToolResultCandidate {
        tool_call_id,
        call_id: tool_call_id,
        owner_user_id: "user-1",
        project_id: "proj-1",
        run_id: "run-1",
        model_attempt_id: "attempt-1",
        decision_id: "decision-1",
        state,
        result_text,
    }
}

fn source(tool_call_id: &str, state: &str, kept_result_tokens: Option<u64>) -> ToolResultSource {
    ToolResultSource {
        tool_call_id: tool_call_id.to_owned(),
        call_id: tool_call_id.to_owned(),
        model_attempt_id: "attempt-1".to_owned(),
        decision_id: "decision-1".to_owned(),
        state: state.to_owned(),
        trust: "data_only".to_owned(),
        kept_result_tokens,
    }
}

/// The source version, projection mode, and content of each selected `tool_result` item.
fn tool_items(record: &CurrentPassageAssemblyRecord) -> Vec<(String, ProjectionMode, String)> {
    record
        .selected
        .iter()
        .filter(|item| item.source_class == ContextSourceClass::ToolResult)
        .map(|item| {
            (
                item.source_version.clone(),
                item.projection_mode,
                item.content.clone(),
            )
        })
        .collect()
}

#[test]
fn each_resolved_tool_call_is_one_data_only_candidate_and_states_its_outcome_in_one_sentence() {
    let mut record = passage();

    assemble_tool_result_continuation(
        &mut record,
        &[
            candidate("c1", ToolCallState::Settled, Some("Built in 1890.")),
            candidate("c2", ToolCallState::Failed, None),
            candidate("c3", ToolCallState::Unknown, None),
            candidate("c4", ToolCallState::Refused, None),
        ],
    );

    let exact = ProjectionMode::ExactRequired;
    assert_eq!(
        (
            record.operation_requirement.cause,
            record.operation_requirement.tool_results.clone(),
            tool_items(&record),
            record.sufficiency.clone(),
        ),
        (
            ContextCause::ToolResultContinuation,
            Some(vec![
                source("c1", "ok", None),
                source("c2", "unavailable", None),
                source("c3", "unavailable", None),
                source("c4", "rejected", None),
            ]),
            vec![
                (
                    "c1".to_owned(),
                    exact,
                    r#"<tool_result source="c1" state="ok">Built in 1890.</tool_result>"#
                        .to_owned()
                ),
                (
                    "c2".to_owned(),
                    exact,
                    "<tool_result source=\"c2\" state=\"unavailable\">The call failed, so no \
                     result is available, and this is not an empty result.</tool_result>"
                        .to_owned()
                ),
                (
                    "c3".to_owned(),
                    exact,
                    "<tool_result source=\"c3\" state=\"unavailable\">The outcome of the call is \
                     unknown, so no result is available, and this is not an empty \
                     result.</tool_result>"
                        .to_owned()
                ),
                (
                    "c4".to_owned(),
                    exact,
                    "<tool_result source=\"c4\" state=\"rejected\">The Host refused this call \
                     before it was sent, so no result is available, and this is not an empty \
                     result.</tool_result>"
                        .to_owned()
                ),
            ],
            ContextSufficiency::Complete,
        )
    );
    assert_eq!(
        decode_assembly_record(&encode_assembly_record(&record)),
        Some(record)
    );
}

#[test]
fn an_unresolved_tool_call_or_one_of_another_run_blocks_the_continuation() {
    let mut record = passage();
    let other_run = ToolResultCandidate {
        run_id: "run-2",
        ..candidate("c2", ToolCallState::Settled, Some("Built in 1890."))
    };

    assemble_tool_result_continuation(
        &mut record,
        &[candidate("c1", ToolCallState::Dispatched, None), other_run],
    );

    let rejected = |id: &str| RejectedSource {
        source_class: ContextSourceClass::ToolResult,
        source_version: id.to_owned(),
        token_count: 0,
        reason: RejectionReason::ToolResultUnavailable,
    };
    assert_eq!(
        (
            tool_items(&record),
            record.rejected.clone(),
            record.sufficiency.clone()
        ),
        (
            Vec::new(),
            vec![rejected("c1"), rejected("c2")],
            ContextSufficiency::Blocked {
                reasons: vec![ContextBlockReason::ToolResultUnavailable],
            },
        )
    );
}

#[test]
fn a_result_above_the_item_limit_keeps_a_prefix_and_an_explicit_truncation_mark() {
    let mut record = passage();
    record.operation_requirement.item_token_limit = 200;
    let long = "a<b".repeat(40);

    assemble_tool_result_continuation(
        &mut record,
        &[candidate("c1", ToolCallState::Settled, Some(&long))],
    );

    let item = record
        .selected
        .iter()
        .find(|item| item.source_class == ContextSourceClass::ToolResult)
        .unwrap();
    let kept = record.operation_requirement.tool_results.as_ref().unwrap()[0].kept_result_tokens;
    let prefix = format!("{}a", "a&lt;b".repeat(13));
    assert_eq!(
        (
            item.projection_mode,
            kept,
            item.content.as_str(),
            item.token_count <= 200,
            item.token_count == count_context_item_tokens(&item.content),
        ),
        (
            ProjectionMode::DeterministicExcerpt,
            Some(40),
            format!(
                "<tool_result source=\"c1\" state=\"ok\">{prefix} [The Host cut this result \
                 here. It keeps 40 of its 120 characters.]</tool_result>"
            )
            .as_str(),
            true,
            true,
        )
    );
}
