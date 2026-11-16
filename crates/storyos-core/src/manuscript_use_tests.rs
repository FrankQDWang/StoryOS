use serde_json::{Value, json};

use super::*;
use crate::{
    BlockRange, LOAD_SKILL_TOOL_SPEC, ReadableChapter, admit_chapter_read, admit_manuscript_search,
};

fn chapter(id: &str, title: &str) -> ReadableChapter {
    ReadableChapter {
        chapter_id: id.to_owned(),
        title: title.to_owned(),
        revision_id: format!("{id}-head"),
        blocks: vec!["Lin ran.".to_owned(), "The bell.".to_owned()],
    }
}

/// One stored call: its ToolSpec revision, its state, its arguments, and its result text.
type Call = (&'static str, ToolCallState, Value, Option<String>);

fn read(chapters: &[ReadableChapter], number: i64, range: BlockRange) -> Call {
    let arguments =
        json!({"chapter": number, "first_block": range.first, "last_block": range.last});
    match admit_chapter_read(chapters, number, range, "call_read") {
        Ok(result) => (
            READ_CHAPTER_TOOL_SPEC.revision,
            ToolCallState::Settled,
            arguments,
            Some(result.text),
        ),
        Err(refusal) => (
            READ_CHAPTER_TOOL_SPEC.revision,
            ToolCallState::Refused,
            arguments,
            Some(refusal.result().text),
        ),
    }
}

fn search(chapters: &[ReadableChapter], query: &str, state: ToolCallState) -> Call {
    let result = admit_manuscript_search(chapters, query, "call_search");
    (
        SEARCH_MANUSCRIPT_TOOL_SPEC.revision,
        state,
        json!({"query": query}),
        Some(result.text),
    )
}

fn uses(calls: &[Call]) -> Vec<ManuscriptUse> {
    let stored: Vec<StoredManuscriptCall<'_>> = calls
        .iter()
        .map(
            |(revision, state, arguments, result)| StoredManuscriptCall {
                tool_spec_revision: revision,
                state: *state,
                arguments,
                result_text: result.as_deref(),
            },
        )
        .collect();
    manuscript_uses(&stored)
}

fn chapter_read(id: &str, title: &str) -> ManuscriptUse {
    ManuscriptUse::ChapterRead {
        chapter_id: id.to_owned(),
        title: title.to_owned(),
    }
}

fn keyword_search(query: &str) -> ManuscriptUse {
    ManuscriptUse::KeywordSearch {
        query: query.to_owned(),
    }
}

#[test]
fn each_chapter_and_each_query_is_one_entry_in_the_order_of_the_first_use() {
    let before = [chapter("a", "Chapter A"), chapter("b", "Fog")];
    // An Author Edit renames Chapter A between two reads of it.
    let after = [chapter("a", "Dawn"), chapter("b", "Fog")];
    let whole = BlockRange::default();
    let calls = [
        read(&before, 1, whole),
        search(&before, "Lin", ToolCallState::Settled),
        read(
            &after,
            1,
            BlockRange {
                first: Some(2),
                last: None,
            },
        ),
        read(&before, 2, whole),
        search(&before, "Lin", ToolCallState::Settled),
        search(&before, "bell", ToolCallState::Settled),
    ];

    assert_eq!(
        uses(&calls),
        [
            chapter_read("a", "Chapter A"),
            keyword_search("Lin"),
            chapter_read("b", "Fog"),
            keyword_search("bell"),
        ]
    );
}

#[test]
fn a_call_that_did_not_settle_or_of_another_tool_gives_no_entry() {
    let chapters = [chapter("a", "Chapter A")];
    let refused = read(&chapters, 3, BlockRange::default());
    let load = (
        LOAD_SKILL_TOOL_SPEC.revision,
        ToolCallState::Settled,
        json!({"skill": "internal-comms", "reason": "The author asks."}),
        Some("SKILL".to_owned()),
    );
    let calls = [
        refused.clone(),
        search(&chapters, "Lin", ToolCallState::Refused),
        search(&chapters, "bell", ToolCallState::Failed),
        (
            READ_CHAPTER_TOOL_SPEC.revision,
            ToolCallState::Pending,
            refused.2.clone(),
            None,
        ),
        load,
    ];

    assert_eq!(refused.1, ToolCallState::Refused);
    assert_eq!(uses(&calls), []);
}
