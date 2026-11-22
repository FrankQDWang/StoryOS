use serde_json::json;

use super::{SEARCH_MEMORY_TOOL_SPEC, search_memory};
use crate::assemble_context::count_context_item_tokens;
use crate::shared_tests::exposure_facts;
use crate::{
    AGENT_PLAN_REGISTRATION, CONTEXT_ITEM_TOKEN_LIMIT, ContextSourceClass, CurrentMemoryEntry,
    ExposureFacts, HOST_FAKE_TOOL_LOOP_REGISTRATION, IntakeRefusal, MEMORY_DESCRIPTION_MAX_CHARS,
    MEMORY_TITLE_MAX_CHARS, MemoryEntryType, ToolExposure, ToolIntake, ToolResultProjection,
};

fn names(revision: &str, memory_use: bool) -> Vec<&'static str> {
    let exposure = ToolExposure::compute(&ExposureFacts {
        memory_use,
        ..exposure_facts(revision)
    });
    exposure.tools.iter().map(|tool| tool.name).collect()
}

/// The token count of the `tool_result` item of the result `text` of the call `call-1`.
fn item_tokens(text: &str) -> u64 {
    count_context_item_tokens(&ToolResultProjection::Ok(text).continuation_text("call-1"))
}

fn entry(number: u64, entry_type: MemoryEntryType, fields: [&str; 3]) -> CurrentMemoryEntry {
    let [title, description, text] = fields.map(str::to_owned);
    CurrentMemoryEntry {
        memory_entry_id: format!("entry-{number}"),
        number,
        version: 1,
        entry_type,
        title,
        description,
        text,
        flagged: false,
        author_part: false,
    }
}

/// ADR 0094 section 4: the current mapping and the fake tool loop mapping declare
/// `search_memory` with a strict schema after `read_memory_entry`, only for a Run whose captured
/// Conversation Memory Settings have `use_enabled`.
#[test]
fn a_run_with_memory_use_declares_search_memory_with_a_strict_schema() {
    let before = [
        "propose_edit",
        "submit_research",
        "read_tool_result",
        "read_chapter",
        "search_manuscript",
        "write_memory",
    ];
    for revision in [
        AGENT_PLAN_REGISTRATION.revision,
        HOST_FAKE_TOOL_LOOP_REGISTRATION.revision,
    ] {
        let exposure = ToolExposure::compute(&ExposureFacts {
            memory_use: true,
            ..exposure_facts(revision)
        });
        let search = exposure.tools[7].declaration();
        assert_eq!(
            (
                names(revision, true),
                names(revision, false),
                search.name,
                search.description,
                search.parameters
            ),
            (
                [
                    &before[..],
                    &["read_memory_entry", "search_memory", "finish"]
                ]
                .concat(),
                [&before[..], &["finish"]].concat(),
                "search_memory",
                "Finds the entries of the Memory of this Project that hold an exact word or \
                 phrase in the title, the description, or the text. The result lists each entry \
                 that matches, with no text. Then read an entry with read_memory_entry.",
                json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The exact text to find, with the same letter case. \
                                            It is not a question."
                        }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                })
            )
        );
    }
}

/// The intake takes one query of 1 to 512 characters with no control character, the bound of
/// the query of `web_search`. Each other value is refused with the reason in one sentence.
#[test]
fn the_intake_takes_one_query_in_the_bound_of_a_web_search() {
    let intake = |arguments| SEARCH_MEMORY_TOOL_SPEC.intake(&arguments);
    let longest = "雪".repeat(512);
    let query = |text: &str| {
        Ok(ToolIntake::SearchMemory {
            query: text.to_owned(),
        })
    };
    assert_eq!(
        [
            json!({"query": "雪"}),
            json!({"query": longest}),
            json!({"query": "雪".repeat(513)}),
            json!({"query": ""}),
            json!({"query": "  "}),
            json!({"query": "七\n结"}),
            json!({"query": 7}),
            json!({}),
            json!({"query": "雪", "limit": 3}),
        ]
        .map(intake),
        [
            query("雪"),
            query(&longest),
            Err(IntakeRefusal::Query),
            Err(IntakeRefusal::Query),
            Err(IntakeRefusal::Query),
            Err(IntakeRefusal::Query),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::Malformed),
        ]
    );
}

/// A term of one Chinese character matches in each field, a term in each one field names that
/// field, and the result lists the matches in the order of the Memory Index with no text of an
/// entry. A term that no entry has gives one sentence. The settled result has the source class
/// `memory_search_result` (the row of the rule of S7-B1b).
#[test]
fn a_search_lists_the_entries_that_hold_the_term_in_the_order_of_the_index() {
    let entries = [
        entry(
            3,
            MemoryEntryType::Other,
            ["渡口", "旧渡口在河湾", "雪夜里渡口没有灯。"],
        ),
        entry(
            1,
            MemoryEntryType::Character,
            ["林雪", "渡口船工", "船头系七结绳。"],
        ),
        entry(
            2,
            MemoryEntryType::Foreshadowing,
            ["铜铃", "铃声", "铃在雪中响。"],
        ),
        entry(
            4,
            MemoryEntryType::Character,
            ["林舟", "林雪之妹", "不上船。"],
        ),
    ];
    let text = |query| search_memory(&entries, query, "call-1").text;
    assert_eq!(
        (
            text("雪"),
            text("林雪"),
            text("河湾"),
            text("七结"),
            text("灯塔"),
            crate::host_tool_result_class(&SEARCH_MEMORY_TOOL_SPEC),
        ),
        (
            [
                r#"{"type":"foreshadowing"}"#,
                r#"{"description":"铃声","id":2,"matched":["text"],"title":"铜铃"}"#,
                r#"{"type":"character"}"#,
                r#"{"description":"渡口船工","id":1,"matched":["title"],"title":"林雪"}"#,
                r#"{"description":"林雪之妹","id":4,"matched":["description"],"title":"林舟"}"#,
                r#"{"type":"other"}"#,
                r#"{"description":"旧渡口在河湾","id":3,"matched":["text"],"title":"渡口"}"#,
            ]
            .join("\n"),
            [
                r#"{"type":"character"}"#,
                r#"{"description":"渡口船工","id":1,"matched":["title"],"title":"林雪"}"#,
                r#"{"description":"林雪之妹","id":4,"matched":["description"],"title":"林舟"}"#,
            ]
            .join("\n"),
            [
                r#"{"type":"other"}"#,
                r#"{"description":"旧渡口在河湾","id":3,"matched":["description"],"title":"渡口"}"#,
            ]
            .join("\n"),
            [
                r#"{"type":"character"}"#,
                r#"{"description":"渡口船工","id":1,"matched":["text"],"title":"林雪"}"#,
            ]
            .join("\n"),
            "No current entry of the Memory has this exact text, and this is not a failure."
                .to_owned(),
            ContextSourceClass::MemorySearchResult,
        )
    );
}

/// ADR 0052 and ADR 0085 section 2: a valid Memory can hold more matches than one Context item.
/// The result keeps whole entry lines while its item, with its boundary and its escape, fits the
/// item limit, and the first entry that does not fit ends the result with one line. Thus the
/// bounded projection does not cut the item. The entries that the result keeps give a complete
/// result with no line, and one more entry does not fit.
#[test]
fn a_result_keeps_whole_entries_while_its_item_fits_the_limit() {
    let entries: Vec<CurrentMemoryEntry> = (1..=300)
        .map(|number| {
            let title = format!("{number:03}{}", "题".repeat(MEMORY_TITLE_MAX_CHARS - 3));
            let description = format!("铜铃{}", "长".repeat(MEMORY_DESCRIPTION_MAX_CHARS - 2));
            entry(
                number,
                MemoryEntryType::Other,
                [&title, &description, "铜铃"],
            )
        })
        .collect();
    let text = |count: usize| search_memory(&entries[..count], "铜铃", "call-1").text;
    let line = |other: usize| {
        format!(
            "\n[This result does not have the other {other} entries with a match. Search for a \
             more exact term to find them.]"
        )
    };
    let cut = text(300);
    let kept = cut.matches("\"matched\"").count();
    let projected = ToolResultProjection::Ok(&cut).bounded_text("call-1", CONTEXT_ITEM_TOKEN_LIMIT);
    assert_eq!(
        (
            (100..300).contains(&kept),
            cut == format!("{}{}", text(kept), line(300 - kept)),
            text(kept).contains("[This result"),
            text(kept + 1) == format!("{}{}", text(kept), line(1)),
            item_tokens(&cut) <= CONTEXT_ITEM_TOKEN_LIMIT,
            projected.kept_result_tokens.is_none(),
        ),
        (true, true, false, true, true, true)
    );
}
