//! The Host Tool `read_chapter` (ADR 0095 section 1): the Agent reads the text of one Chapter of
//! the manuscript. The call names the Chapter by its number in the Chapter list. The result is the
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
when the author request needs the text of a Chapter that is not the Working text.";

/// The strict JSON Schema of the arguments.
pub(crate) fn parameters() -> Value {
    json!({
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
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    chapter: i64,
}

/// The typed intake. A value that is not an integer does not fit. The manuscript at the time of
/// the call decides each integer.
pub(crate) fn intake(arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
    let Arguments { chapter } =
        Arguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
    Ok(ToolIntake::ReadChapter { chapter })
}

/// Why the Host refused one validated `read_chapter` call. The result gives the reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChapterReadRefusal {
    /// No live Chapter of the Project has the number at the time of the call.
    NotFound,
}

impl ChapterReadRefusal {
    /// The stable label of the refusal, for the `refusal` of the ToolCall row.
    pub fn label(self) -> &'static str {
        match self {
            Self::NotFound => "chapter_not_found",
        }
    }

    /// The result of the refused call: the reason in one sentence.
    pub fn result(self) -> ToolResultText {
        let reason = match self {
            Self::NotFound => {
                "no Chapter of the manuscript has this number now: use a number of the section \
                 Chapter list"
            }
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

/// The result of a read of the Chapter with the number `chapter` in `chapters`, for the native
/// call `call_id`. The result has one header line and then whole Blocks from the first Block
/// while the `tool_result` item of the call, with its boundary and its escape, fits in the item
/// limit. The first Block that does not fit ends the result with one line that says where the
/// Chapter continues. When the first Block alone does not fit, the result has only that Block,
/// and the bounded projection cuts it with its mark (ADR 0085 section 2).
pub fn admit_chapter_read(
    chapters: &[ReadableChapter],
    chapter: i64,
    call_id: &str,
) -> Result<ToolResultText, ChapterReadRefusal> {
    let read = usize::try_from(chapter)
        .ok()
        .and_then(|number| chapters.get(number.checked_sub(1)?))
        .ok_or(ChapterReadRefusal::NotFound)?;
    let header = |last_block: usize| {
        crate::project_archive::canonical_json(&json!({
            "chapter": chapter,
            "title": read.title,
            "chapter_id": read.chapter_id,
            "revision_id": read.revision_id,
            "blocks": read.blocks.len(),
            "last_block": last_block,
        }))
    };
    let block = |index: usize| format!("\n[block {}]\n{}", index + 1, read.blocks[index]);
    let continues = |next: usize| {
        format!(
            "\n[The Chapter continues at block {next}. This result does not have block {next} or \
             the blocks after it.]"
        )
    };
    // The token count is additive, so the cost of a text is the sum of the costs of its parts.
    let cost = |text: &str| count_context_item_tokens(&crate::tool_result::escape(text));
    let budget = CONTEXT_ITEM_TOKEN_LIMIT.saturating_sub(count_context_item_tokens(
        &ToolResultProjection::Ok("").continuation_text(call_id),
    ));
    let costs: Vec<u64> = (0..read.blocks.len())
        .map(|index| cost(&block(index)))
        .collect();
    let total: u64 = costs.iter().sum();
    let count = read.blocks.len();
    let kept = match cost(&header(count)) + total <= budget {
        true => count,
        false => {
            let mut kept = 0;
            let mut used = 0;
            while kept + 1 < count {
                let with_next = used + costs[kept];
                if cost(&header(kept + 1)) + with_next + cost(&continues(kept + 2)) > budget {
                    break;
                }
                used = with_next;
                kept += 1;
            }
            kept
        }
    };
    let text = match kept {
        // The first Block alone does not fit: the projection cuts it.
        0 if count > 0 => format!("{}{}", header(1), block(0)),
        _ => {
            let blocks: String = (0..kept).map(block).collect();
            let tail = match kept < count {
                true => continues(kept + 1),
                false => String::new(),
            };
            format!("{}{blocks}{tail}", header(kept))
        }
    };
    Ok(crate::host_tool_result(&text))
}

#[cfg(test)]
#[path = "read_chapter_tests.rs"]
mod tests;
