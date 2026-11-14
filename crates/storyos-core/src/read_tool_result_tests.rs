use super::*;
use crate::{
    CitedSource, CurrentPassageAssembly, InstructionBindingInput, ToolResultCandidate,
    ToolResultOutput, assemble_current_passage_context, assemble_tool_result_continuation,
};

fn passage() -> crate::CurrentPassageAssemblyRecord {
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

fn sources() -> Vec<CitableSource> {
    vec![CitableSource {
        id: "snapshot-1".to_owned(),
        cited: CitedSource::Snapshot,
        locator: "https://example.org/harbour".to_owned(),
        owner_user_id: "user-1".to_owned(),
        project_id: "proj-1".to_owned(),
        run_id: "run-1".to_owned(),
    }]
}

fn candidate<'a>(
    tool_call_id: &'a str,
    result_text: &'a str,
    sources: &'a [CitableSource],
    output: ToolResultOutput,
) -> ToolResultCandidate<'a> {
    ToolResultCandidate {
        tool_call_id,
        call_id: tool_call_id,
        owner_user_id: "user-1",
        project_id: "proj-1",
        run_id: "run-1",
        model_attempt_id: "attempt-1",
        decision_id: "decision-1",
        state: ToolCallState::Settled,
        result_text: Some(result_text),
        sources,
        output,
    }
}

fn search<'a>(text: &'a str, sources: &'a [CitableSource]) -> StoredCall<'a> {
    StoredCall {
        call_id: "call_web_search",
        spec_revision: Some(crate::WEB_SEARCH_TOOL_SPEC.revision),
        state: ToolCallState::Settled,
        result_text: Some(text),
        sources,
    }
}

/// The content of each selected item of the continuation of `candidates`.
fn items(candidates: &[ToolResultCandidate<'_>]) -> Vec<String> {
    let mut record = passage();
    assemble_tool_result_continuation(&mut record, candidates);
    record
        .selected
        .into_iter()
        .filter(|item| item.source_class == crate::ContextSourceClass::ToolResult)
        .map(|item| item.content)
        .collect()
}

#[test]
fn a_read_gives_the_bounded_form_of_the_first_read_and_the_next_request_sends_it_as_it_is() {
    let sources = sources();
    let limit = passage().operation_requirement.item_token_limit;
    // One short result, and one result above the item limit that the bound cuts.
    for text in [
        "Title: Harbour\nURL: https://example.org/harbour\nBuilt in 1890.".to_owned(),
        "港".repeat(usize::try_from(limit).unwrap() + 10),
    ] {
        let first = items(&[candidate(
            "call_web_search",
            &text,
            &sources,
            ToolResultOutput::Projected,
        )]);
        let read =
            read_tool_result(ReadTarget::SameRun(search(&text, &sources)), false, limit).unwrap();
        assert_eq!(vec![read.text.clone()], first);
        let next = items(&[candidate(
            "call_read_tool_result",
            &read.text,
            &[],
            ToolResultOutput::StoredRead,
        )]);
        assert_eq!(next, first);
    }
}

#[test]
fn each_refusal_of_a_read_gives_its_reason() {
    let sources = sources();
    let finish = StoredCall {
        spec_revision: Some(crate::FINISH_TOOL_SPEC.revision),
        ..search("", &[])
    };
    let failed = StoredCall {
        state: ToolCallState::Failed,
        result_text: None,
        ..search("", &sources)
    };
    let unpinned = StoredCall {
        spec_revision: None,
        state: ToolCallState::Refused,
        ..search("The Host refused this call.", &[])
    };
    let cases = [
        (
            ReadTarget::Unknown,
            false,
            ReadToolResultRefusal::UnknownCall,
        ),
        (ReadTarget::OtherRun, false, ReadToolResultRefusal::OtherRun),
        (
            ReadTarget::SameRun(finish),
            false,
            ReadToolResultRefusal::HostToolCall,
        ),
        (
            ReadTarget::SameRun(failed),
            false,
            ReadToolResultRefusal::NoStoredResult,
        ),
        (
            ReadTarget::SameRun(unpinned),
            false,
            ReadToolResultRefusal::NoStoredResult,
        ),
        (
            ReadTarget::SameRun(search("Built in 1890.", &sources)),
            true,
            ReadToolResultRefusal::RestrictedSource,
        ),
    ];
    for (target, restricted, refusal) in cases {
        assert_eq!(read_tool_result(target, restricted, 32_000), Err(refusal));
    }
    assert_eq!(
        ReadToolResultRefusal::OtherRun.result().text,
        "The Host refused this call because the call with this identity belongs to an earlier \
         author request, so no result is available, and this is not an empty result."
    );
}
