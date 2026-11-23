use serde_json::{Value, json};

use super::{SubrunState, SubrunStep, subrun_state, subrun_steps};
use crate::{
    AgentRunStatus, BlockRange, ChapterPassages, CurrentMemoryEntry,
    HYBRID_SEARCH_MANUSCRIPT_TOOL_SPEC, LocatedPassage, MemoryEntryRead, MemoryEntryType,
    PassageTerms, READ_CHAPTER_TOOL_SPEC, READ_MEMORY_ENTRY_TOOL_SPEC, ReadableChapter,
    SEARCH_MANUSCRIPT_TOOL_SPEC, SEARCH_MEMORY_TOOL_SPEC, StoredManuscriptCall, SubrunEnd,
    ToolCallState,
};

/// Item 1 of #1836: a Subrun that did not end is `working`, a cancelled Subrun is `stopped`, a
/// completed Subrun whose call got its final answer is `completed`, and each other end is
/// `no_result`.
#[test]
fn the_state_of_a_subrun_follows_its_status_and_its_final_answer() {
    let state = |status, final_answer| {
        subrun_state(SubrunEnd {
            status,
            final_answer,
            too_long: false,
        })
    };
    let long = "雪".repeat(crate::TOOL_RESULT_TEXT_MAX_BYTES / 3 + 1);
    assert_eq!(
        [
            state(AgentRunStatus::Queued, None),
            state(AgentRunStatus::Claimed, None),
            state(AgentRunStatus::Completed, Some("第一章有灯。")),
            state(AgentRunStatus::Completed, Some("")),
            state(AgentRunStatus::Completed, Some(&long)),
            state(AgentRunStatus::Refused, None),
            state(AgentRunStatus::Paused, None),
            state(AgentRunStatus::Waiting, None),
            state(AgentRunStatus::Cancelled, None),
        ],
        [
            SubrunState::Working,
            SubrunState::Working,
            SubrunState::Completed,
            SubrunState::NoResult,
            SubrunState::NoResult,
            SubrunState::NoResult,
            SubrunState::NoResult,
            SubrunState::NoResult,
            SubrunState::Stopped,
        ]
    );
}

/// Item 2 of #1836: each count comes from a stored result that the Tool of the call made. The
/// manuscript has the name 林舟 three times in two Blocks, and the name 林雪 in one passage. Of
/// three Memory entries, two have the term 渡口. No entry has a Chapter text, a passage text, or
/// a Memory Entry text. The texts of a RunStep come before its calls, and a call that did not
/// settle and a `finish` call give no entry.
#[test]
fn the_entries_give_the_counts_of_the_stored_results_and_no_read_text() {
    let chapter = ReadableChapter {
        chapter_id: "chapter-1".to_owned(),
        title: "渡口".to_owned(),
        revision_id: "revision-1".to_owned(),
        blocks: vec![
            "林舟在渡口。林舟不上船。".to_owned(),
            "风很大，林舟走了。".to_owned(),
        ],
    };
    let chapters = std::slice::from_ref(&chapter);
    let whole = BlockRange {
        first: None,
        last: None,
    };
    let read = crate::admit_chapter_read(chapters, 1, whole, "call-1")
        .unwrap()
        .text;
    let keyword = crate::admit_manuscript_search(chapters, "林舟", "call-2").text;
    let passages = ChapterPassages {
        chapter_id: "chapter-1".to_owned(),
        title: "渡口".to_owned(),
        passages: vec![LocatedPassage {
            first_block: 2,
            last_block: 2,
            text: "林雪在渡口等船。".to_owned(),
            digest: "sha256:0".to_owned(),
        }],
    };
    let terms = PassageTerms::new(&["林雪".to_owned()]).unwrap();
    let hybrid = crate::hybrid_search_result(&[passages], None, &terms, "call-3").text;
    let entry = |number, title: &str, description: &str, text: &str| CurrentMemoryEntry {
        memory_entry_id: format!("entry-{number}"),
        number,
        version: 1,
        entry_type: MemoryEntryType::Character,
        title: title.to_owned(),
        description: description.to_owned(),
        text: text.to_owned(),
        flagged: false,
        author_part: false,
    };
    let entries = [
        entry(1, "林舟", "船夫。", "他住在渡口东边。"),
        entry(2, "沈月", "渡口的人。", "她住在渡口。"),
        entry(3, "顾川", "商人。", "城里的人。"),
    ];
    let memory = MemoryEntryRead {
        memory_entry_id: "entry-1".to_owned(),
        version: 1,
        entry_type: MemoryEntryType::Character,
        title: "林舟".to_owned(),
        description: "沉默的船夫。".to_owned(),
        origin: "author_request".to_owned(),
        changed_at: "2026-11-23T08:00:00.000Z".to_owned(),
        text: "- 十年前落水\n- 从此不上船".to_owned(),
        flag: None,
        intent: None,
    }
    .result()
    .text;
    let searched = crate::search_memory(&entries, "渡口", "call-5").text;
    let arguments = [
        json!({"chapter": 1, "first_block": null, "last_block": null}),
        json!({"query": "林舟"}),
        json!({"question": "谁在等船？", "terms": ["林雪"]}),
        json!({"entry": 1}),
        json!({"query": "渡口"}),
        json!({}),
    ];
    let settled = ToolCallState::Settled;
    // A `finish` call settles with an empty result.
    let finished = String::new();
    let calls = [
        (
            0,
            call(
                &arguments,
                0,
                READ_CHAPTER_TOOL_SPEC.revision,
                settled,
                Some(&read),
            ),
        ),
        (
            0,
            call(
                &arguments,
                1,
                SEARCH_MANUSCRIPT_TOOL_SPEC.revision,
                ToolCallState::Unknown,
                None,
            ),
        ),
        (
            1,
            call(
                &arguments,
                1,
                SEARCH_MANUSCRIPT_TOOL_SPEC.revision,
                settled,
                Some(&keyword),
            ),
        ),
        (
            1,
            call(
                &arguments,
                2,
                HYBRID_SEARCH_MANUSCRIPT_TOOL_SPEC.revision,
                settled,
                Some(&hybrid),
            ),
        ),
        (
            2,
            call(
                &arguments,
                3,
                READ_MEMORY_ENTRY_TOOL_SPEC.revision,
                settled,
                Some(&memory),
            ),
        ),
        (
            2,
            call(
                &arguments,
                4,
                SEARCH_MEMORY_TOOL_SPEC.revision,
                settled,
                Some(&searched),
            ),
        ),
        (
            3,
            call(
                &arguments,
                5,
                crate::FINISH_TOOL_SPEC.revision,
                settled,
                Some(&finished),
            ),
        ),
    ];
    let texts = [(1, "先找名字。".to_owned()), (2, "再查记忆。".to_owned())];
    let steps = subrun_steps(&texts, &calls);
    let shown = format!("{steps:?}");
    let read_texts = [
        "林舟在渡口",
        "风很大",
        "林雪在渡口等船",
        "十年前落水",
        "住在渡口",
    ];

    assert_eq!(
        (
            steps,
            read_texts.map(|text| shown.contains(text)),
            // The source of each count: the header of each search, and the entries of the Memory search.
            [&keyword, &hybrid].map(|result| first_line(result)),
            searched.lines().count(),
        ),
        (
            vec![
                SubrunStep::ChapterRead {
                    chapter_id: "chapter-1".to_owned(),
                    title: "渡口".to_owned()
                },
                SubrunStep::Text {
                    text: "先找名字。".to_owned()
                },
                SubrunStep::KeywordSearch {
                    query: "林舟".to_owned(),
                    matches: 3
                },
                SubrunStep::HybridSearch {
                    question: "谁在等船？".to_owned(),
                    passages: 1
                },
                SubrunStep::Text {
                    text: "再查记忆。".to_owned()
                },
                SubrunStep::MemoryRead {
                    title: "林舟".to_owned()
                },
                SubrunStep::MemorySearch {
                    query: "渡口".to_owned(),
                    entries: 2
                },
            ],
            [false; 5],
            [
                json!({"blocks": 2, "blocks_returned": 2, "complete": true, "matches": 3, "query": "林舟"}),
                json!({"passages": 1, "passages_returned": 1, "terms": ["林雪"]}),
            ],
            3,
        )
    );
}

/// The first JSON line of `result`.
fn first_line(result: &str) -> Value {
    result
        .lines()
        .find_map(|line| serde_json::from_str(line).ok())
        .unwrap()
}

/// One stored call with the arguments `arguments[index]`.
fn call<'a>(
    arguments: &'a [Value],
    index: usize,
    revision: &'a str,
    state: ToolCallState,
    result: Option<&'a String>,
) -> StoredManuscriptCall<'a> {
    StoredManuscriptCall {
        tool_spec_revision: revision,
        state,
        arguments: &arguments[index],
        result_text: result.map(String::as_str),
    }
}
