use serde_json::json;

use super::{
    BlockRange, ChapterReadRefusal, READ_CHAPTER_TOOL_SPEC, ReadableChapter, admit_chapter_read,
};
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

/// The range from the Block `first` to the Block `last`.
fn range(first: Option<i64>, last: Option<i64>) -> BlockRange {
    BlockRange { first, last }
}

/// The token count of the `tool_result` item of the result `text` of the call `call-1`.
fn item_tokens(text: &str) -> u64 {
    count_context_item_tokens(&ToolResultProjection::Ok(text).continuation_text("call-1"))
}

/// ADR 0095 section 1: the mapping `v8` and the fake tool loop mapping declare `read_chapter`
/// in each Run, before `finish`, with a strict schema of one integer and a range of Blocks.
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
                "search_manuscript",
                "finish"
            ]
        );
        assert_eq!(
            (tools[3].description, &tools[3].parameters),
            (
                "Reads the text of one Chapter of the manuscript. Call it when the author request \
                 needs the text of a Chapter that is not the Working text. Give a range of Blocks \
                 to continue a Chapter after an earlier result or to read the Blocks near a match \
                 of search_manuscript.",
                &json!({
                    "type": "object",
                    "properties": {
                        "chapter": {
                            "type": "integer",
                            "description": "The number of a Chapter in the section Chapter list."
                        },
                        "first_block": {
                            "type": ["integer", "null"],
                            "description": "The number of the first Block to read, from 1. Null \
                                            reads from the first Block of the Chapter."
                        },
                        "last_block": {
                            "type": ["integer", "null"],
                            "description": "The number of the last Block to read. Null reads to \
                                            the last Block of the Chapter."
                        }
                    },
                    "required": ["chapter", "first_block", "last_block"],
                    "additionalProperties": false
                })
            )
        );
    }
}

#[test]
fn the_intake_takes_one_integer_and_a_range_of_integers_or_null() {
    let intake = |arguments| READ_CHAPTER_TOOL_SPEC.intake(&arguments);
    let read = |chapter, first_block, last_block| {
        Ok(ToolIntake::ReadChapter {
            chapter,
            first_block,
            last_block,
        })
    };
    assert_eq!(
        [
            intake(json!({"chapter": 2})),
            intake(json!({"chapter": -1, "first_block": null, "last_block": null})),
            intake(json!({"chapter": 2, "first_block": 3, "last_block": 0})),
            intake(json!({"chapter": "2"})),
            intake(json!({"chapter": 1.5})),
            intake(json!({"chapter": 2, "first_block": "3"})),
            intake(json!({})),
            intake(json!({"chapter": 2, "range": 1})),
        ],
        [
            read(2, None, None),
            read(-1, None, None),
            read(2, Some(3), Some(0)),
            Err(IntakeRefusal::Malformed),
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
    let whole = range(None, None);
    assert_eq!(
        [
            admit_chapter_read(&chapters, 1, whole, "call-1"),
            admit_chapter_read(&chapters, 2, whole, "call-1"),
        ],
        [
            Ok(host_tool_result(
                "{\"blocks\":3,\"chapter\":1,\"chapter_id\":\"chapter-Harbor\",\"first_block\":1,\
                 \"last_block\":3,\"revision_id\":\"revision-Harbor\",\"title\":\"Harbor\"}\n\
                 [block 1]\nRain <fell>.\n[block 2]\n\n[block 3]\nThe \"end\" & after."
            )),
            Ok(host_tool_result(
                "{\"blocks\":0,\"chapter\":2,\"chapter_id\":\"chapter-Empty\",\"first_block\":1,\
                 \"last_block\":0,\"revision_id\":\"revision-Empty\",\"title\":\"Empty\"}"
            )),
        ]
    );
}

/// ADR 0095 section 1: a range gives its Blocks from `first_block` to `last_block`, and the
/// header names its first and last Block. A null value is the first or the last Block, and a
/// `last_block` above the count reads to the last Block.
#[test]
fn a_range_gives_its_blocks_and_the_header_names_them() {
    let chapters = [chapter("Harbor", &["One.", "Two.", "Three.", "Four."])];
    let read = |first, last| {
        admit_chapter_read(&chapters, 1, range(first, last), "call-1")
            .map(|result| result.text)
            .unwrap()
    };
    let header = |first: usize, last: usize| {
        format!(
            "{{\"blocks\":4,\"chapter\":1,\"chapter_id\":\"chapter-Harbor\",\"first_block\":\
             {first},\"last_block\":{last},\"revision_id\":\"revision-Harbor\",\"title\":\
             \"Harbor\"}}"
        )
    };
    assert_eq!(
        [
            read(Some(2), Some(3)),
            read(Some(3), None),
            read(None, Some(1)),
            read(Some(4), Some(9)),
            read(Some(2), Some(2)),
        ],
        [
            format!("{}\n[block 2]\nTwo.\n[block 3]\nThree.", header(2, 3)),
            format!("{}\n[block 3]\nThree.\n[block 4]\nFour.", header(3, 4)),
            format!("{}\n[block 1]\nOne.", header(1, 1)),
            format!("{}\n[block 4]\nFour.", header(4, 4)),
            format!("{}\n[block 2]\nTwo.", header(2, 2)),
        ]
    );
}

/// ADR 0039: the first refusal is `chapter_not_found`, for a number that no live Chapter has.
/// The store gives only the live Chapters, so after the author removes the last Chapter, its
/// number is above the count, as in the third case.
#[test]
fn a_number_of_no_live_chapter_is_refused() {
    let chapters = [chapter("Harbor", &["Rain."])];
    let refused = ChapterReadRefusal::NotFound;
    let read = |chapters: &[ReadableChapter], number| {
        admit_chapter_read(chapters, number, range(None, None), "call-1")
    };
    assert_eq!(
        [
            read(&chapters, 0),
            read(&chapters, -3),
            read(&chapters, 2),
            read(&[], 1),
            // The Chapter decides before the range.
            admit_chapter_read(&chapters, 2, range(Some(0), None), "call-1"),
        ],
        [
            Err(refused),
            Err(refused),
            Err(refused),
            Err(refused),
            Err(refused)
        ]
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

/// ADR 0039 and ADR 0095 section 1: the second refusal is `block_range_invalid`: `first_block`
/// is below 1 or above the count of the Blocks, or `last_block` is below `first_block`. The
/// result gives the count of the Blocks.
#[test]
fn a_range_outside_the_chapter_is_refused_with_the_count_of_its_blocks() {
    let chapters = [
        chapter("Harbor", &["One.", "Two.", "Three."]),
        chapter("Empty", &[]),
    ];
    let read =
        |number, first, last| admit_chapter_read(&chapters, number, range(first, last), "call-1");
    let three = ChapterReadRefusal::BlockRangeInvalid { blocks: 3 };
    assert_eq!(
        [
            read(1, Some(0), None),
            read(1, Some(-2), Some(1)),
            read(1, Some(4), None),
            read(1, Some(3), Some(2)),
            read(1, None, Some(0)),
            read(2, Some(1), None),
        ],
        [
            Err(three),
            Err(three),
            Err(three),
            Err(three),
            Err(three),
            Err(ChapterReadRefusal::BlockRangeInvalid { blocks: 0 }),
        ]
    );
    assert_eq!(
        (three.label(), three.result()),
        (
            "block_range_invalid",
            host_tool_result(
                "The Host refused this call because the range of Blocks is not in this Chapter, \
                 which has 3 Blocks now: give a first_block from 1 to 3, or null, and a \
                 last_block that is not below it, so no result is available, and this is not an \
                 empty result."
            )
        )
    );
}

/// ADR 0052 and ADR 0085 section 2: the item of a result, with its boundary and its escape, is at
/// or below the item limit. Whole Blocks fit from the first Block of the range, and the first
/// Block that does not fit ends the result with one line. A first Block alone above the limit is
/// the whole result, and the bounded projection cuts it.
#[test]
fn the_result_keeps_whole_blocks_while_its_item_fits_the_limit() {
    let header = |blocks: usize, first: usize, last: usize| {
        format!(
            "{{\"blocks\":{blocks},\"chapter\":1,\"chapter_id\":\"chapter-Long\",\"first_block\":\
             {first},\"last_block\":{last},\"revision_id\":\"revision-Long\",\"title\":\"Long\"}}"
        )
    };
    // Each `<` costs four counted tokens after the escape.
    let filler = |tokens: u64| "<".repeat(usize::try_from(tokens / 4).unwrap());
    let read = |blocks: &[&str], first| {
        admit_chapter_read(&[chapter("Long", blocks)], 1, range(first, None), "call-1")
            .map(|result| result.text)
            .unwrap()
    };
    // Two Blocks that fill the item exactly: the second Block is 4 tokens shorter than the space.
    let two = item_tokens(&format!("{}\n[block 1]\n\n[block 2]\n", header(2, 1, 2)));
    let first = filler(16_000);
    let room = CONTEXT_ITEM_TOKEN_LIMIT - two - 16_000;
    let at_limit = filler(room - room % 4);
    let exact = read(&[&first, &at_limit], None);
    // One more `<` puts the whole Chapter above the limit, so the result stops at block 2.
    let above = format!("{at_limit}<<<<");
    let cut = read(&[&first, &above], None);
    // A first Block that fits alone but not with the line after it: the result has the first
    // Block and the line, and the bounded projection cuts it (#1605 round 1).
    let one = item_tokens(&format!("{}\n[block 1]\n", header(2, 1, 1)));
    let fills = filler(CONTEXT_ITEM_TOKEN_LIMIT - one);
    let alone = read(&[&fills, "Rain."], None);
    let notice = |next: usize| {
        format!(
            "\n[The Chapter continues at block {next}. This result does not have block {next} or \
             the blocks after it. Read them with first_block {next}.]"
        )
    };
    let projected =
        ToolResultProjection::Ok(&alone).bounded_text("call-1", CONTEXT_ITEM_TOKEN_LIMIT);
    // The bound applies from the first Block of the range: the second read gives block 2, and a
    // range that starts at block 1 of three Blocks stops after block 1.
    let rest = read(&[&first, &above], Some(2));
    let from_one = read(&[&first, &above, "Rain."], Some(1));

    assert_eq!(
        (
            exact
                == format!(
                    "{}\n[block 1]\n{first}\n[block 2]\n{at_limit}",
                    header(2, 1, 2)
                ),
            item_tokens(&exact) <= CONTEXT_ITEM_TOKEN_LIMIT,
            item_tokens(&exact) + 4 > CONTEXT_ITEM_TOKEN_LIMIT,
            cut == format!("{}\n[block 1]\n{first}{}", header(2, 1, 1), notice(2)),
            alone == format!("{}\n[block 1]\n{fills}{}", header(2, 1, 1), notice(2)),
            item_tokens(&format!("{}\n[block 1]\n{fills}", header(2, 1, 1)))
                <= CONTEXT_ITEM_TOKEN_LIMIT,
            projected.kept_result_tokens.is_some(),
            count_context_item_tokens(&projected.text) <= CONTEXT_ITEM_TOKEN_LIMIT,
            rest == format!("{}\n[block 2]\n{above}", header(2, 2, 2)),
            from_one == format!("{}\n[block 1]\n{first}{}", header(3, 1, 1), notice(2)),
        ),
        (true, true, true, true, true, true, true, true, true, true)
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
