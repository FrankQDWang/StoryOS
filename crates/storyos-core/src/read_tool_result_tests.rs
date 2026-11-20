use super::*;
use crate::shared_tests::harbour_passage;
use crate::shared_tests::tool_result_candidate;
use crate::{
    CitedSource, ToolResultCandidate, ToolResultOutput, assemble_tool_result_continuation,
};

fn sources() -> Vec<CitableSource> {
    vec![CitableSource {
        id: "snapshot-1".to_owned(),
        cited: CitedSource::Snapshot,
        locator: "https://example.org/harbour".to_owned(),
        title: None,
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
        sources,
        output,
        ..tool_result_candidate(tool_call_id, ToolCallState::Settled, Some(result_text))
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
    let mut record = harbour_passage();
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
    let limit = harbour_passage().operation_requirement.item_token_limit;
    // One short result, and one result above the item limit that the bound cuts.
    for text in [
        "Title: Harbour\nURL: https://example.org/harbour\nBuilt in 1890.".to_owned(),
        "港".repeat(usize::try_from(limit).unwrap() + 10),
    ] {
        let first = items(&[candidate(
            "call_web_search",
            &text,
            &sources,
            ToolResultOutput::Research,
        )]);
        let read = read_tool_result(ReadTarget::SameRun(search(&text, &sources)), limit).unwrap();
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
        (ReadTarget::Unknown, ReadToolResultRefusal::UnknownCall),
        (ReadTarget::OtherRun, ReadToolResultRefusal::OtherRun),
        (
            ReadTarget::SameRun(finish),
            ReadToolResultRefusal::HostToolCall,
        ),
        (
            ReadTarget::SameRun(failed),
            ReadToolResultRefusal::NoStoredResult,
        ),
        (
            ReadTarget::SameRun(unpinned),
            ReadToolResultRefusal::NoStoredResult,
        ),
    ];
    for (target, refusal) in cases {
        assert_eq!(read_tool_result(target, 32_000), Err(refusal));
    }
    assert_eq!(
        ReadToolResultRefusal::OtherRun.result().text,
        "The Host refused this call because the call with this identity belongs to an earlier \
         author request, so no result is available, and this is not an empty result."
    );
}
