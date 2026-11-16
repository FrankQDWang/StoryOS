use serde_json::json;

use super::{ChapterReadRefusal, READ_CHAPTER_TOOL_SPEC, ReadableChapter, admit_chapter_read};
use crate::assemble_context::count_context_item_tokens;
use crate::{
    AGENT_PLAN_REGISTRATION, CONTEXT_ITEM_TOKEN_LIMIT, ContextSourceClass, ExposureFacts,
    HOST_FAKE_TOOL_LOOP_REGISTRATION, IntakeRefusal, ToolExposure, ToolIntake,
    ToolResultProjection, host_tool_result,
};

fn chapter(title: &str, blocks: &[&str]) -> ReadableChapter {
    ReadableChapter {
        chapter_id: format!("chapter-{title}"),
        title: title.to_owned(),
        revision_id: format!("revision-{title}"),
        blocks: blocks.iter().map(|text| (*text).to_owned()).collect(),
    }
}

/// The token count of the `tool_result` item of the result `text` of the call `call-1`.
fn item_tokens(text: &str) -> u64 {
    count_context_item_tokens(&ToolResultProjection::Ok(text).continuation_text("call-1"))
}

/// ADR 0095 section 1: the mapping `v8` and the fake tool loop mapping declare `read_chapter`
/// in each Run, before `finish`, with a strict schema of one integer.
#[test]
fn each_current_mapping_declares_read_chapter_with_a_strict_schema() {
    let declared = |revision| {
        ToolExposure::compute(&ExposureFacts {
            bound: &[],
            model_registration_revision: revision,
            skill_catalog: false,
        })
        .tools
        .into_iter()
        .map(|tool| tool.declaration())
        .collect::<Vec<_>>()
    };
    for revision in [
        AGENT_PLAN_REGISTRATION.revision,
        HOST_FAKE_TOOL_LOOP_REGISTRATION.revision,
    ] {
        let tools = declared(revision);
        let names: Vec<&str> = tools.iter().map(|tool| tool.name).collect();
        assert_eq!(
            names,
            [
                "propose_edit",
                "submit_research",
                "read_tool_result",
                "read_chapter",
                "finish"
            ]
        );
        assert_eq!(
            (tools[3].description, &tools[3].parameters),
            (
                "Reads the text of one Chapter of the manuscript. Call it when the author request \
                 needs the text of a Chapter that is not the Working text.",
                &json!({
                    "type": "object",
                    "properties": {
                        "chapter": {
                            "type": "integer",
                            "description": "The number of a Chapter in the section Chapter list."
                        }
                    },
                    "required": ["chapter"],
                    "additionalProperties": false
                })
            )
        );
    }
}

#[test]
fn the_intake_takes_one_integer() {
    let intake = |arguments| READ_CHAPTER_TOOL_SPEC.intake(&arguments);
    assert_eq!(
        [
            intake(json!({"chapter": 2})),
            intake(json!({"chapter": -1})),
            intake(json!({"chapter": "2"})),
            intake(json!({"chapter": 1.5})),
            intake(json!({})),
            intake(json!({"chapter": 2, "range": 1})),
        ],
        [
            Ok(ToolIntake::ReadChapter { chapter: 2 }),
            Ok(ToolIntake::ReadChapter { chapter: -1 }),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::Malformed),
        ]
    );
}

/// The result is one canonical JSON header line and then each Block with its position. A Chapter
/// with no Block gives the header only.
#[test]
fn a_read_gives_the_header_and_each_block_in_order() {
    let chapters = [
        chapter("Harbor", &["Rain <fell>.", "", "The \"end\" & after."]),
        chapter("Empty", &[]),
    ];
    assert_eq!(
        [
            admit_chapter_read(&chapters, 1, "call-1"),
            admit_chapter_read(&chapters, 2, "call-1"),
        ],
        [
            Ok(host_tool_result(
                "{\"blocks\":3,\"chapter\":1,\"chapter_id\":\"chapter-Harbor\",\"last_block\":3,\
                 \"revision_id\":\"revision-Harbor\",\"title\":\"Harbor\"}\n[block 1]\nRain \
                 <fell>.\n[block 2]\n\n[block 3]\nThe \"end\" & after."
            )),
            Ok(host_tool_result(
                "{\"blocks\":0,\"chapter\":2,\"chapter_id\":\"chapter-Empty\",\"last_block\":0,\
                 \"revision_id\":\"revision-Empty\",\"title\":\"Empty\"}"
            )),
        ]
    );
}

/// ADR 0039: the one Host refusal is `chapter_not_found`, for a number that no live Chapter has.
/// The store gives only the live Chapters, so after the author removes the last Chapter, its
/// number is above the count, as in the third case.
#[test]
fn a_number_of_no_live_chapter_is_refused() {
    let chapters = [chapter("Harbor", &["Rain."])];
    let refused = ChapterReadRefusal::NotFound;
    assert_eq!(
        [
            admit_chapter_read(&chapters, 0, "call-1"),
            admit_chapter_read(&chapters, -3, "call-1"),
            admit_chapter_read(&chapters, 2, "call-1"),
            admit_chapter_read(&[], 1, "call-1"),
        ],
        [Err(refused), Err(refused), Err(refused), Err(refused)]
    );
    assert_eq!(
        (refused.label(), refused.result()),
        (
            "chapter_not_found",
            host_tool_result(
                "The Host refused this call because no Chapter of the manuscript has this number \
                 now: use a number of the section Chapter list, so no result is available, and \
                 this is not an empty result."
            )
        )
    );
}

/// ADR 0052 and ADR 0085 section 2: the item of a result, with its boundary and its escape, is at
/// or below the item limit. Whole Blocks fit from the first Block, and the first Block that does
/// not fit ends the result with one line. A first Block alone above the limit is the whole result,
/// and the bounded projection cuts it.
#[test]
fn the_result_keeps_whole_blocks_while_its_item_fits_the_limit() {
    let header = |blocks: usize, last: usize| {
        format!(
            "{{\"blocks\":{blocks},\"chapter\":1,\"chapter_id\":\"chapter-Long\",\"last_block\":\
             {last},\"revision_id\":\"revision-Long\",\"title\":\"Long\"}}"
        )
    };
    // Each `<` costs four counted tokens after the escape.
    let filler = |tokens: u64| "<".repeat(usize::try_from(tokens / 4).unwrap());
    let read = |blocks: &[&str]| {
        admit_chapter_read(&[chapter("Long", blocks)], 1, "call-1")
            .map(|result| result.text)
            .unwrap()
    };
    // Two Blocks that fill the item exactly: the second Block is 4 tokens shorter than the space.
    let two = item_tokens(&format!("{}\n[block 1]\n\n[block 2]\n", header(2, 2)));
    let first = filler(16_000);
    let room = CONTEXT_ITEM_TOKEN_LIMIT - two - 16_000;
    let at_limit = filler(room - room % 4);
    let exact = read(&[&first, &at_limit]);
    // One more `<` puts the whole Chapter above the limit, so the result stops at block 2.
    let above = format!("{at_limit}<<<<");
    let cut = read(&[&first, &above]);
    // A first Block that fits alone but not with the line after it: the result has the first
    // Block and the line, and the bounded projection cuts it (#1605 round 1).
    let one = item_tokens(&format!("{}\n[block 1]\n", header(2, 1)));
    let fills = filler(CONTEXT_ITEM_TOKEN_LIMIT - one);
    let alone = read(&[&fills, "Rain."]);
    let notice = "\n[The Chapter continues at block 2. This result does not have block 2 or the \
                  blocks after it.]";
    let projected =
        ToolResultProjection::Ok(&alone).bounded_text("call-1", CONTEXT_ITEM_TOKEN_LIMIT);

    assert_eq!(
        (
            exact
                == format!(
                    "{}\n[block 1]\n{first}\n[block 2]\n{at_limit}",
                    header(2, 2)
                ),
            item_tokens(&exact) <= CONTEXT_ITEM_TOKEN_LIMIT,
            item_tokens(&exact) + 4 > CONTEXT_ITEM_TOKEN_LIMIT,
            cut == format!("{}\n[block 1]\n{first}{notice}", header(2, 1)),
            alone == format!("{}\n[block 1]\n{fills}{notice}", header(2, 1)),
            item_tokens(&format!("{}\n[block 1]\n{fills}", header(2, 1)))
                <= CONTEXT_ITEM_TOKEN_LIMIT,
            projected.kept_result_tokens.is_some(),
            count_context_item_tokens(&projected.text) <= CONTEXT_ITEM_TOKEN_LIMIT,
        ),
        (true, true, true, true, true, true, true, true)
    );
}

/// ADR 0095 section 1: one Core rule gives the source class of the settled result of a Host
/// Tool from its ToolSpec. The result of `read_chapter` is a `manuscript_read_result` item.
#[test]
fn the_result_of_read_chapter_has_the_manuscript_read_class() {
    assert_eq!(
        [
            crate::host_tool_result_class(&READ_CHAPTER_TOOL_SPEC),
            crate::host_tool_result_class(&crate::FINISH_TOOL_SPEC),
        ],
        [
            ContextSourceClass::ManuscriptReadResult,
            ContextSourceClass::ToolResult,
        ]
    );
    assert_eq!(
        [
            crate::ToolResultOutput::of_spec(Some(READ_CHAPTER_TOOL_SPEC.revision)),
            crate::ToolResultOutput::of_spec(Some(crate::READ_TOOL_RESULT_TOOL_SPEC.revision)),
            crate::ToolResultOutput::of_spec(Some(crate::WEB_SEARCH_TOOL_SPEC.revision)),
            crate::ToolResultOutput::of_spec(None),
        ],
        [
            crate::ToolResultOutput::HostTool(ContextSourceClass::ManuscriptReadResult),
            crate::ToolResultOutput::StoredRead,
            crate::ToolResultOutput::Research,
            crate::ToolResultOutput::HostTool(ContextSourceClass::ToolResult),
        ]
    );
}
