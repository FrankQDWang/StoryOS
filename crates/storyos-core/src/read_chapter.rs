//! The Host Tool `read_chapter` (ADR 0095 section 1): the Agent reads the text of one Chapter of
//! the manuscript, or a range of its Blocks. The call names the Chapter by its number in the
//! Chapter list, and the range by the numbers of its first and last Block. The result is the
//! authoritative head text of the Chapter at the time of the call, in the bounds of one Context
//! item, and the next request sends it as the output of the call. The call has no external effect
//! and gives the Agent no target.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::assemble_context::count_context_item_tokens;
use crate::{
    CONTEXT_ITEM_TOKEN_LIMIT, IntakeRefusal, TOOL_RESULT_TEXT_MAX_BYTES, ToolIntake, ToolOperation,
    ToolResultProjection, ToolResultText, ToolSpec,
};

pub const READ_CHAPTER_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.read-chapter.v1",
    operation: ToolOperation::ReadChapter,
    intake_contract: &[],
    envelope: None,
    // The Context item of a result is at or below the item limit (ADR 0052).
    result_text_max_bytes: TOOL_RESULT_TEXT_MAX_BYTES,
};

pub(crate) const DESCRIPTION: &str = "Reads the text of one Chapter of the manuscript. Call it \
when the author request needs the text of a Chapter that is not the Working text. Give a range of \
Blocks to continue a Chapter after an earlier result or to read the Blocks near a match of \
search_manuscript.";

/// The strict JSON Schema of the arguments.
pub(crate) fn parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "chapter": {
                "type": "integer",
                "description": "The number of a Chapter in the section Chapter list."
            },
            "first_block": {
                "type": ["integer", "null"],
                "description": "The number of the first Block to read, from 1. Null reads from \
                                the first Block of the Chapter."
            },
            "last_block": {
                "type": ["integer", "null"],
                "description": "The number of the last Block to read. Null reads to the last \
                                Block of the Chapter."
            }
        },
        "required": ["chapter", "first_block", "last_block"],
        "additionalProperties": false
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    chapter: i64,
    first_block: Option<i64>,
    last_block: Option<i64>,
}

/// The typed intake. A value that is not an integer or, for a Block, null does not fit. The
/// manuscript at the time of the call decides each integer.
pub(crate) fn intake(arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
    let Arguments {
        chapter,
        first_block,
        last_block,
    } = Arguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
    Ok(ToolIntake::ReadChapter {
        chapter,
        first_block,
        last_block,
    })
}

/// Why the Host refused one validated `read_chapter` call. The result gives the reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChapterReadRefusal {
    /// No live Chapter of the Project has the number at the time of the call.
    NotFound,
    /// The first Block is below 1 or above the count `blocks` of the Blocks of the Chapter, or
    /// the last Block is below the first Block.
    BlockRangeInvalid { blocks: usize },
}

impl ChapterReadRefusal {
    /// The stable label of the refusal, for the `refusal` of the ToolCall row.
    pub fn label(self) -> &'static str {
        match self {
            Self::NotFound => "chapter_not_found",
            Self::BlockRangeInvalid { .. } => "block_range_invalid",
        }
    }

    /// The result of the refused call: the reason in one sentence.
    pub fn result(self) -> ToolResultText {
        let reason = match self {
            Self::NotFound => "no Chapter of the manuscript has this number now: use a number of \
                               the section Chapter list"
                .to_owned(),
            Self::BlockRangeInvalid { blocks } => format!(
                "the range of Blocks is not in this Chapter, which has {blocks} Blocks now: give a \
                 first_block from 1 to {blocks}, or null, and a last_block that is not below it"
            ),
        };
        crate::host_tool_result(&format!(
            "The Host refused this call because {reason}, so no result is available, and this is \
             not an empty result."
        ))
    }
}

/// One live Chapter of the Project at the time of the call, in manuscript order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadableChapter {
    pub chapter_id: String,
    pub title: String,
    /// The head revision of the Chapter that the read gave.
    pub revision_id: String,
    /// The text of each Block of the head, in Block order.
    pub blocks: Vec<String>,
}

/// The numbers of the first and the last Block that a call names. `None` is the first or the last
/// Block of the Chapter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BlockRange {
    pub first: Option<i64>,
    pub last: Option<i64>,
}

/// The result of a read of the Blocks `range` of the Chapter with the number `chapter` in
/// `chapters`, for the native call `call_id`. A last Block above the count of the Blocks reads to
/// the last Block. The result has one header line and then whole Blocks from the first Block of
/// the range while the `tool_result` item of the call, with its boundary and its escape, fits in
/// the item limit. The first Block that does not fit ends the result with one line that says
/// where the Chapter continues. When the first Block does not fit with that line, the result has
/// only the first Block and the line, and the bounded projection cuts it with its mark (ADR 0085
/// section 2).
pub fn admit_chapter_read(
    chapters: &[ReadableChapter],
    chapter: i64,
    range: BlockRange,
    call_id: &str,
) -> Result<ToolResultText, ChapterReadRefusal> {
    let read = usize::try_from(chapter)
        .ok()
        .and_then(|number| chapters.get(number.checked_sub(1)?))
        .ok_or(ChapterReadRefusal::NotFound)?;
    let count = read.blocks.len();
    let invalid = ChapterReadRefusal::BlockRangeInvalid { blocks: count };
    // The index of the first Block of the range, and the index after its last Block.
    let start = match range.first {
        None => 0,
        Some(first) => {
            usize::try_from(first)
                .ok()
                .filter(|first| (1..=count).contains(first))
                .ok_or(invalid)?
                - 1
        }
    };
    let end = match range.last {
        None => count,
        Some(last) if last < i64::try_from(start + 1).unwrap_or(i64::MAX) => return Err(invalid),
        Some(last) => usize::try_from(last).map_or(count, |last| last.min(count)),
    };
    let header = |kept: usize| {
        crate::project_archive::canonical_json(&json!({
            "chapter": chapter,
            "title": read.title,
            "chapter_id": read.chapter_id,
            "revision_id": read.revision_id,
            "blocks": count,
            "first_block": start + 1,
            "last_block": start + kept,
        }))
    };
    let block = |index: usize| format!("\n[block {}]\n{}", index + 1, read.blocks[index]);
    let continues = |next: usize| {
        format!(
            "\n[The Chapter continues at block {next}. This result does not have block {next} or \
             the blocks after it. Read them with first_block {next}.]"
        )
    };
    let blocks: Vec<String> = (start..end).map(block).collect();
    Ok(bounded_result(
        &blocks,
        header,
        |kept| continues(start + kept + 1),
        call_id,
    ))
}

/// The result of the native call `call_id` with the first parts of `parts` that fit in the item
/// limit (`kept_prefix`). The text is `head` of the kept count, the kept parts, and, when the
/// result does not keep each part, `tail` of the kept count.
pub(crate) fn bounded_result(
    parts: &[String],
    head: impl Fn(usize) -> String,
    tail: impl Fn(usize) -> String,
    call_id: &str,
) -> ToolResultText {
    let count = parts.len();
    let costs: Vec<u64> = parts.iter().map(|part| text_cost(part)).collect();
    let kept = kept_prefix(
        &costs,
        |kept| match kept < count {
            true => text_cost(&head(kept)) + text_cost(&tail(kept)),
            false => text_cost(&head(kept)),
        },
        call_id,
    );
    let tail = match kept < count {
        true => tail(kept),
        false => String::new(),
    };
    crate::host_tool_result(&format!("{}{}{tail}", head(kept), parts[..kept].concat()))
}

/// The counted tokens of `text` in a `tool_result` item, after its escape. The token count is
/// additive, so the cost of a text is the sum of the costs of its parts.
pub(crate) fn text_cost(text: &str) -> u64 {
    count_context_item_tokens(&crate::tool_result::escape(text))
}

/// The count of the parts with the costs `costs` that a result keeps from its first part, for the
/// native call `call_id`. `frame` gives the cost of the other text of a result that keeps `kept`
/// parts. The result keeps each part while its `tool_result` item, with its boundary and its
/// escape, fits in the item limit. When the first part does not fit, the result keeps it, and the
/// bounded projection cuts the result with its mark (ADR 0085 section 2).
pub(crate) fn kept_prefix(costs: &[u64], frame: impl Fn(usize) -> u64, call_id: &str) -> usize {
    let budget = CONTEXT_ITEM_TOKEN_LIMIT.saturating_sub(count_context_item_tokens(
        &ToolResultProjection::Ok("").continuation_text(call_id),
    ));
    let count = costs.len();
    if frame(count) + costs.iter().sum::<u64>() <= budget {
        return count;
    }
    let mut kept = 0;
    let mut used = 0;
    while kept + 1 < count && frame(kept + 1) + used + costs[kept] <= budget {
        used += costs[kept];
        kept += 1;
    }
    kept.max(count.min(1))
}

#[cfg(test)]
#[path = "read_chapter_tests.rs"]
mod tests;
