//! The fixtures that many test files of this crate build.

use crate::{
    AgentRunKind, CurrentPassageAssembly, CurrentPassageAssemblyRecord, ExposureFacts,
    InstructionBindingInput, ToolCallState, ToolResultCandidate, ToolResultForm, ToolResultOutput,
    assemble_current_passage_context,
};

/// The input of a current-passage Context Assembly with `author_message` on the revision `rev-1`
/// of the Chapter `chapter-1` with the text `chapter_body`. The identities are `requirement-1`,
/// `snapshot-1`, `run-1`, `owner-1`, `project-1`, and `destination-1`. It has no Proposal
/// target and no Project Instruction.
pub(crate) fn passage_assembly(author_message: &str, chapter_body: &str) -> CurrentPassageAssembly {
    CurrentPassageAssembly {
        operation_requirement_id: "requirement-1".to_owned(),
        input_snapshot_id: "snapshot-1".to_owned(),
        run_id: "run-1".to_owned(),
        owner_user_id: "owner-1".to_owned(),
        project_id: "project-1".to_owned(),
        author_message: author_message.to_owned(),
        chapter_id: "chapter-1".to_owned(),
        chapter_revision_id: Some("rev-1".to_owned()),
        proposal_target_block_ids: None,
        chapter_body: chapter_body.to_owned(),
        instruction: InstructionBindingInput::Absent,
        destination_identity: "destination-1".to_owned(),
    }
}

/// The input of a current-passage Context Assembly with `author_message` on the revision `rev-1`
/// of the Chapter `ch-1` with the text `chapter_body`, with the Proposal target `block-1`. The
/// identities are `req-1`, `snap-1`, `run-1`, `user-1`, `proj-1`, and `dest-1`. It has no
/// Project Instruction.
pub(crate) fn targeted_passage_assembly(
    author_message: &str,
    chapter_body: &str,
) -> CurrentPassageAssembly {
    CurrentPassageAssembly {
        operation_requirement_id: "req-1".to_owned(),
        input_snapshot_id: "snap-1".to_owned(),
        run_id: "run-1".to_owned(),
        owner_user_id: "user-1".to_owned(),
        project_id: "proj-1".to_owned(),
        author_message: author_message.to_owned(),
        chapter_id: "ch-1".to_owned(),
        chapter_revision_id: Some("rev-1".to_owned()),
        proposal_target_block_ids: Some(vec!["block-1".to_owned()]),
        chapter_body: chapter_body.to_owned(),
        instruction: InstructionBindingInput::Absent,
        destination_identity: "dest-1".to_owned(),
    }
}

/// The decision record of the question "When was the harbour built?" on the revision `ch-1-rev`
/// of the Chapter `ch-1`, with the Proposal target `ch-1-block` and the identities `req-2` and
/// `snap-2`.
pub(crate) fn harbour_passage() -> CurrentPassageAssemblyRecord {
    assemble_current_passage_context(&CurrentPassageAssembly {
        operation_requirement_id: "req-2".to_owned(),
        input_snapshot_id: "snap-2".to_owned(),
        chapter_revision_id: Some("ch-1-rev".to_owned()),
        proposal_target_block_ids: Some(vec!["ch-1-block".to_owned()]),
        ..targeted_passage_assembly("When was the harbour built?", "The harbour was old.")
    })
}

/// The facts of the Tool Exposure of an Author Run that pinned `model_registration_revision`,
/// with no bound Tool, no Skill catalog, no Memory use, and no search by meaning.
pub(crate) fn exposure_facts(model_registration_revision: &str) -> ExposureFacts<'_> {
    ExposureFacts {
        bound: &[],
        model_registration_revision,
        skill_catalog: false,
        memory_use: false,
        hybrid_search: false,
        run_kind: AgentRunKind::Author,
    }
}

/// A `web_search` ToolCall of the Run `run-1` with `state` and `result_text`, with no source, in
/// the exact form of a research result.
pub(crate) fn tool_result_candidate<'a>(
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
        sources: &[],
        output: ToolResultOutput::Research,
        function_name: "web_search",
        arguments: "{}",
        form: ToolResultForm::Exact,
        skill: None,
        resource_path: None,
    }
}
