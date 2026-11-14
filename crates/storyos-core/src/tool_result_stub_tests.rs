use super::*;
use crate::{CitedSource, SourceGapReason};

fn snapshot(index: usize) -> CitableSource {
    CitableSource {
        id: format!("snapshot-{index:02}"),
        cited: CitedSource::Snapshot,
        locator: format!("https://example.org/tides/{index:02}"),
        title: Some(format!("Liverpool tide table {index:02}")),
        owner_user_id: "user-1".to_owned(),
        project_id: "proj-1".to_owned(),
        run_id: "run-1".to_owned(),
    }
}

fn search(sources: &[CitableSource]) -> StubCall<'_> {
    StubCall {
        call_id: "call_web_search",
        tool_name: "web_search",
        arguments: r#"{"query":"tide tables of 1890","result_count":null}"#,
        sources,
        read_call_id: None,
    }
}

const SEARCH_BODY: &str = "<tool_result source=\"call_web_search\" state=\"ok\" form=\"stub\">\
The Host replaced this result with a stub after a compaction of the conversation. Tool: \
web_search. Arguments: {&quot;query&quot;:&quot;tide tables of 1890&quot;,\
&quot;result_count&quot;:null}. To read the full result again, call read_tool_result with the \
call_id call_web_search.";

#[test]
fn a_stub_gives_the_tool_the_arguments_each_source_line_and_the_read_sentence() {
    let gap = CitableSource {
        id: "gap-01".to_owned(),
        cited: CitedSource::Gap(SourceGapReason::Failed),
        locator: "https://example.org/missing".to_owned(),
        title: None,
        ..snapshot(0)
    };
    let sources = [snapshot(1), gap];

    assert_eq!(
        tool_result_stub(&search(&sources)),
        format!(
            "{SEARCH_BODY}</tool_result><tool_sources source=\"call_web_search\">\n\
             <source id=\"snapshot-01\" title=\"Liverpool tide table 01\" \
             locator=\"https://example.org/tides/01\"/>\n\
             <gap id=\"gap-01\" locator=\"https://example.org/missing\" \
             reason=\"failed\"/>\n</tool_sources>"
        )
    );
}

#[test]
fn a_stub_with_too_many_source_lines_keeps_the_first_lines_and_counts_the_others() {
    let sources: Vec<CitableSource> = (1..=40).map(snapshot).collect();
    let stub = tool_result_stub(&search(&sources));
    let kept = stub.matches("<source id=").count();
    let left_out = 40 - kept;

    assert_eq!(
        (
            count_context_item_tokens(&stub) <= TOOL_RESULT_STUB_TOKEN_LIMIT,
            stub.starts_with(&format!(
                "{SEARCH_BODY} The tool_sources tag gives only the first {kept} of the 40 \
                 sources of the result. Read the full result again before you cite one of the \
                 other {left_out} sources.</tool_result>"
            )),
            stub.contains("<source id=\"snapshot-01\" "),
            stub.contains(&format!("<source id=\"snapshot-{kept:02}\" ")),
            stub.contains(&format!("<source id=\"snapshot-{:02}\" ", kept + 1)),
            (10..40).contains(&kept),
        ),
        (true, true, true, true, false, true)
    );
}

#[test]
fn the_stub_of_a_read_names_the_call_that_it_read() {
    let read = StubCall {
        call_id: "call_read_tool_result",
        tool_name: "read_tool_result",
        arguments: r#"{"call_id":"call_web_search"}"#,
        sources: &[],
        read_call_id: Some("call_web_search"),
    };

    assert_eq!(
        tool_result_stub(&read),
        "<tool_result source=\"call_read_tool_result\" state=\"ok\" form=\"stub\">The Host \
         replaced this result with a stub after a compaction of the conversation. Tool: \
         read_tool_result. Arguments: {&quot;call_id&quot;:&quot;call_web_search&quot;}. This \
         result was the full result of the call call_web_search, and the stub of that call \
         gives its sources. To read the full result again, call read_tool_result with the \
         call_id call_web_search.</tool_result>"
    );
}

#[test]
fn long_arguments_are_cut_so_that_the_stub_stays_in_its_limit() {
    let arguments = format!(
        r#"{{"query":"{}","result_count":null}}"#,
        "tide ".repeat(800)
    );
    let sources = [snapshot(1)];
    let stub = tool_result_stub(&StubCall {
        arguments: &arguments,
        ..search(&sources)
    });

    assert_eq!(
        (
            count_context_item_tokens(&stub) <= TOOL_RESULT_STUB_TOKEN_LIMIT,
            stub.contains(ARGUMENTS_CUT_MARK),
            stub.contains("<source id=\"snapshot-01\" "),
        ),
        (true, true, true)
    );
}
