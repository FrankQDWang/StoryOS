//! The Host Tool `search_manuscript` (ADR 0095 section 1): the Agent finds an exact term in the
//! whole manuscript. The Tool and the author search of the writing workspace use one exact match.
//! The result is the authoritative head text of each Block that has a match, at the time of the
//! call, in the bounds of one Context item. The call has no external effect and gives the Agent
//! no target.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::read_chapter::{kept_prefix, text_cost};
use crate::{
    IntakeRefusal, ReadableChapter, TOOL_RESULT_TEXT_MAX_BYTES, ToolIntake, ToolOperation,
    ToolResultText, ToolSpec,
};

/// The count of matches after which a search stops. The author search and the Tool have this one
/// limit.
pub const MANUSCRIPT_SEARCH_MATCH_LIMIT: usize = 500;

pub const SEARCH_MANUSCRIPT_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.search-manuscript.v1",
    operation: ToolOperation::SearchManuscript,
    intake_contract: &[],
    envelope: None,
    // The Context item of a result is at or below the item limit (ADR 0052).
    result_text_max_bytes: TOOL_RESULT_TEXT_MAX_BYTES,
};

pub(crate) const DESCRIPTION: &str = "Finds each place where an exact word or phrase occurs in \
the manuscript, for example a name. The result gives each Block that has the text. Then read the \
Blocks near a match with read_chapter.";

const NO_MATCH: &str = "No Block of the manuscript has this exact text, and this is not a failure.";

/// The strict JSON Schema of the arguments.
pub(crate) fn parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "The exact text to find, with the same letter case. It is not a \
                                question."
            }
        },
        "required": ["query"],
        "additionalProperties": false
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    query: String,
}

/// The typed intake: the query rule of `web_search`.
pub(crate) fn intake(arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
    let Arguments { query } =
        Arguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
    match crate::tool_spec::query_fits(&query) {
        true => Ok(ToolIntake::SearchManuscript { query }),
        false => Err(IntakeRefusal::Query),
    }
}

/// The byte position of the start of each exact match of `query` in the text `text` of one Block:
/// the same text with the same letter case, from the start of the Block, with no overlap. An
/// empty query has no match.
pub fn exact_matches<'a>(text: &'a str, query: &'a str) -> impl Iterator<Item = usize> + 'a {
    text.match_indices(query)
        .take_while(move |_| !query.is_empty())
        .map(|(start, _)| start)
}

/// The result of a search of `query` in the live Chapters `chapters`, in manuscript order, for
/// the native call `call_id`. The result has one header line and then each Block that has a
/// match, below the line of its Chapter, while the `tool_result` item of the call, with its
/// boundary and its escape, fits in the item limit. The first Block that does not fit ends the
/// result with one line. When the first Block does not fit with that line, the result has only
/// the first Block and the line, and the bounded projection cuts it (ADR 0085 section 2).
pub fn admit_manuscript_search(
    chapters: &[ReadableChapter],
    query: &str,
    call_id: &str,
) -> ToolResultText {
    // Each Block with a match: the index of its Chapter, its index, and its count of matches.
    let mut hits: Vec<(usize, usize, usize)> = Vec::new();
    let mut found = 0;
    let mut complete = true;
    'chapters: for (chapter, read) in chapters.iter().enumerate() {
        for (block, text) in read.blocks.iter().enumerate() {
            let mut count = 0;
            for _ in exact_matches(text, query) {
                if found == MANUSCRIPT_SEARCH_MATCH_LIMIT {
                    complete = false;
                    break;
                }
                found += 1;
                count += 1;
            }
            if count > 0 {
                hits.push((chapter, block, count));
            }
            if !complete {
                break 'chapters;
            }
        }
    }
    let header = |returned: usize| {
        crate::project_archive::canonical_json(&json!({
            "query": query,
            "matches": found,
            "complete": complete,
            "blocks": hits.len(),
            "blocks_returned": returned,
        }))
    };
    if hits.is_empty() {
        return crate::host_tool_result(&format!("{}\n{NO_MATCH}", header(0)));
    }
    let parts: Vec<String> = (hits.iter().enumerate())
        .map(|(index, &(chapter, block, count))| {
            let read = &chapters[chapter];
            let chapter_line = match index == 0 || hits[index - 1].0 != chapter {
                true => format!(
                    "\n{}",
                    crate::project_archive::canonical_json(&json!({
                        "chapter": chapter + 1,
                        "title": read.title,
                        "chapter_id": read.chapter_id,
                    }))
                ),
                false => String::new(),
            };
            let matches = match count {
                1 => "1 match".to_owned(),
                count => format!("{count} matches"),
            };
            format!(
                "{chapter_line}\n[block {}: {matches}]\n{}",
                block + 1,
                read.blocks[block]
            )
        })
        .collect();
    let count = parts.len();
    let more = |kept: usize| {
        format!(
            "\n[This result does not have the other {} Blocks with a match. Search for a more \
             exact term to find them.]",
            count - kept
        )
    };
    let costs: Vec<u64> = parts.iter().map(|part| text_cost(part)).collect();
    let kept = kept_prefix(
        &costs,
        |kept| match kept < count {
            true => text_cost(&header(kept)) + text_cost(&more(kept)),
            false => text_cost(&header(kept)),
        },
        call_id,
    );
    let tail = match kept < count {
        true => more(kept),
        false => String::new(),
    };
    let text = format!("{}{}{tail}", header(kept), parts[..kept].concat());
    crate::host_tool_result(&text)
}

#[cfg(test)]
#[path = "search_manuscript_tests.rs"]
mod tests;
