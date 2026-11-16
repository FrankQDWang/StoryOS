use serde_json::json;

use super::{
    MANUSCRIPT_SEARCH_MATCH_LIMIT, SEARCH_MANUSCRIPT_TOOL_SPEC, admit_manuscript_search,
    exact_matches,
};
use crate::assemble_context::count_context_item_tokens;
use crate::{
    AGENT_PLAN_REGISTRATION, CONTEXT_ITEM_TOKEN_LIMIT, ContextSourceClass, ExposureFacts,
    HOST_FAKE_TOOL_LOOP_REGISTRATION, IntakeRefusal, ReadableChapter, ToolExposure, ToolIntake,
    ToolResultProjection,
};

fn chapter(title: &str, blocks: &[&str]) -> ReadableChapter {
    ReadableChapter {
        chapter_id: format!("chapter-{title}"),
        title: title.to_owned(),
        revision_id: format!("revision-{title}"),
        blocks: blocks.iter().map(|text| (*text).to_owned()).collect(),
    }
}

fn search(chapters: &[ReadableChapter], query: &str) -> String {
    admit_manuscript_search(chapters, query, "call-1").text
}

/// The token count of the `tool_result` item of the result `text` of the call `call-1`.
fn item_tokens(text: &str) -> u64 {
    count_context_item_tokens(&ToolResultProjection::Ok(text).continuation_text("call-1"))
}

/// ADR 0095 section 1: the mapping `v8` and the fake tool loop mapping declare
/// `search_manuscript` in each Run, before `finish`, with a strict schema of one query.
#[test]
fn each_current_mapping_declares_search_manuscript_with_a_strict_schema() {
    for revision in [
        AGENT_PLAN_REGISTRATION.revision,
        HOST_FAKE_TOOL_LOOP_REGISTRATION.revision,
    ] {
        let tools: Vec<_> = ToolExposure::compute(&ExposureFacts {
            bound: &[],
            model_registration_revision: revision,
            skill_catalog: false,
        })
        .tools
        .into_iter()
        .map(|tool| tool.declaration())
        .collect();
        assert_eq!(
            (
                tools.iter().map(|tool| tool.name).collect::<Vec<_>>(),
                tools[4].description,
                &tools[4].parameters
            ),
            (
                vec![
                    "propose_edit",
                    "submit_research",
                    "read_tool_result",
                    "read_chapter",
                    "search_manuscript",
                    "finish"
                ],
                "Finds each place where an exact word or phrase occurs in the manuscript, for \
                 example a name. The result gives each Block that has the text. Then read the \
                 Blocks near a match with read_chapter.",
                &json!({
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

/// The intake is the query rule of `web_search`: 1 to 512 characters, not only white space, and
/// no control character.
#[test]
fn the_intake_takes_one_query_with_the_query_rule_of_web_search() {
    let intake = |arguments| SEARCH_MANUSCRIPT_TOOL_SPEC.intake(&arguments);
    let longest = "雾".repeat(512);
    let query = |text: &str| {
        Ok(ToolIntake::SearchManuscript {
            query: text.to_owned(),
        })
    };
    assert_eq!(
        [
            intake(json!({"query": "Lin Wei"})),
            intake(json!({"query": longest})),
            intake(json!({"query": format!("{longest}雾")})),
            intake(json!({"query": ""})),
            intake(json!({"query": " \n "})),
            intake(json!({"query": "Lin\tWei"})),
            intake(json!({"query": 2})),
            intake(json!({"query": "Lin", "chapter": 1})),
        ],
        [
            query("Lin Wei"),
            query(&longest),
            Err(IntakeRefusal::Query),
            Err(IntakeRefusal::Query),
            Err(IntakeRefusal::Query),
            Err(IntakeRefusal::Query),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::Malformed),
        ]
    );
}

/// The one exact match of the author search and the Tool: the same text with the same letter
/// case, from the start of the Block, with no overlap. An empty query has no match.
#[test]
fn the_exact_match_finds_each_match_of_one_block_from_its_start_with_no_overlap() {
    let starts = |text, query| exact_matches(text, query).collect::<Vec<_>>();
    assert_eq!(
        [
            starts("Lin met lin and Lin.", "Lin"),
            starts("aaaaa", "aa"),
            starts("雾中雾", "雾"),
            starts("Rain.", ""),
            starts("Rain.", "rain"),
        ],
        [vec![0, 16], vec![0, 2], vec![0, 6], vec![], vec![]]
    );
}

/// The result is one header line, the line of each Chapter that has a returned Block, and each
/// returned Block with its position and its count of matches. A match does not go across two
/// Blocks.
#[test]
fn a_search_gives_each_block_with_a_match_below_the_line_of_its_chapter() {
    let chapters = [
        chapter("Harbor", &["Lin <ran>. Lin & \"Mei\".", "No one.", "Lin"]),
        chapter("Empty", &[]),
        chapter("Fog", &["Li", "n came.", "Then Lin."]),
    ];
    assert_eq!(
        search(&chapters, "Lin"),
        "{\"blocks\":3,\"blocks_returned\":3,\"complete\":true,\"matches\":4,\"query\":\"Lin\"}\n\
         {\"chapter\":1,\"chapter_id\":\"chapter-Harbor\",\"title\":\"Harbor\"}\n\
         [block 1: 2 matches]\nLin <ran>. Lin & \"Mei\".\n\
         [block 3: 1 match]\nLin\n\
         {\"chapter\":3,\"chapter_id\":\"chapter-Fog\",\"title\":\"Fog\"}\n\
         [block 3: 1 match]\nThen Lin."
    );
}

/// A search with no match settles with the header and one sentence.
#[test]
fn a_search_with_no_match_says_that_it_is_not_a_failure() {
    assert_eq!(
        [
            search(&[chapter("Harbor", &["Rain."])], "lin"),
            search(&[], "Lin"),
        ],
        [
            "{\"blocks\":0,\"blocks_returned\":0,\"complete\":true,\"matches\":0,\"query\":\"lin\"}\n\
             No Block of the manuscript has this exact text, and this is not a failure.",
            "{\"blocks\":0,\"blocks_returned\":0,\"complete\":true,\"matches\":0,\"query\":\"Lin\"}\n\
             No Block of the manuscript has this exact text, and this is not a failure."
        ]
    );
}

/// The search stops at the match limit of the author search, and `complete` is then false.
#[test]
fn the_search_stops_at_the_match_limit_of_the_author_search() {
    let header = |text: &str| text.lines().next().unwrap().to_owned();
    let full = "x".repeat(MANUSCRIPT_SEARCH_MATCH_LIMIT - 1);
    let at_limit = [chapter("Harbor", &[&full, "x"])];
    let above = [
        chapter("Harbor", &[&full, "xx", "x"]),
        chapter("Fog", &["x"]),
    ];
    assert_eq!(
        [
            header(&search(&at_limit, "x")),
            header(&search(&above, "x"))
        ],
        [
            "{\"blocks\":2,\"blocks_returned\":2,\"complete\":true,\"matches\":500,\"query\":\"x\"}",
            "{\"blocks\":2,\"blocks_returned\":2,\"complete\":false,\"matches\":500,\"query\":\"x\"}",
        ]
    );
    assert!(search(&above, "x").ends_with("\n[block 2: 1 match]\nxx"));
}

/// ADR 0052 and ADR 0085 section 2: the item of a result, with its boundary and its escape, is at
/// or below the item limit. Whole Blocks fit in manuscript order, and the first Block that does
/// not fit ends the result with one line. A first Block that does not fit with that line is the
/// only Block, and the bounded projection cuts it.
#[test]
fn the_result_keeps_whole_blocks_while_its_item_fits_the_limit() {
    let header = |returned: usize| {
        format!(
            "{{\"blocks\":2,\"blocks_returned\":{returned},\"complete\":true,\"matches\":2,\
             \"query\":\"Lin\"}}\n{{\"chapter\":1,\"chapter_id\":\"chapter-Long\",\"title\":\
             \"Long\"}}"
        )
    };
    // Each `<` costs four counted tokens after the escape.
    let filler = |tokens: u64| "<".repeat(usize::try_from(tokens / 4).unwrap());
    let result = |first: &str, second: &str| search(&[chapter("Long", &[first, second])], "Lin");
    let two = item_tokens(&format!(
        "{}\n[block 1: 1 match]\nLin\n[block 2: 1 match]\nLin",
        header(2)
    ));
    let first = format!("Lin{}", filler(16_000));
    let room = CONTEXT_ITEM_TOKEN_LIMIT - two - 16_000;
    let at_limit = format!("Lin{}", filler(room - room % 4));
    let exact = result(&first, &at_limit);
    let above = format!("{at_limit}<<<<");
    let cut = result(&first, &above);
    let more = "\n[This result does not have the other 1 Blocks with a match. Search for a more \
                exact term to find them.]";
    let one = item_tokens(&format!("{}\n[block 1: 1 match]\nLin", header(1)));
    let fills = format!("Lin{}", filler(CONTEXT_ITEM_TOKEN_LIMIT - one));
    let alone = result(&fills, "Lin");
    let projected =
        ToolResultProjection::Ok(&alone).bounded_text("call-1", CONTEXT_ITEM_TOKEN_LIMIT);

    assert_eq!(
        (
            exact
                == format!(
                    "{}\n[block 1: 1 match]\n{first}\n[block 2: 1 match]\n{at_limit}",
                    header(2)
                ),
            item_tokens(&exact) <= CONTEXT_ITEM_TOKEN_LIMIT,
            item_tokens(&exact) + 4 > CONTEXT_ITEM_TOKEN_LIMIT,
            cut == format!("{}\n[block 1: 1 match]\n{first}{more}", header(1)),
            item_tokens(&cut) <= CONTEXT_ITEM_TOKEN_LIMIT,
            alone == format!("{}\n[block 1: 1 match]\n{fills}{more}", header(1)),
            projected.kept_result_tokens.is_some(),
            count_context_item_tokens(&projected.text) <= CONTEXT_ITEM_TOKEN_LIMIT,
        ),
        (true, true, true, true, true, true, true, true)
    );
}

/// ADR 0095 section 1: the settled result of `search_manuscript` is a `manuscript_search_result`
/// item.
#[test]
fn the_result_of_search_manuscript_has_the_manuscript_search_class() {
    assert_eq!(
        (
            crate::host_tool_result_class(&SEARCH_MANUSCRIPT_TOOL_SPEC),
            crate::ToolResultOutput::of_spec(Some(SEARCH_MANUSCRIPT_TOOL_SPEC.revision)),
        ),
        (
            ContextSourceClass::ManuscriptSearchResult,
            crate::ToolResultOutput::HostTool(ContextSourceClass::ManuscriptSearchResult),
        )
    );
}
