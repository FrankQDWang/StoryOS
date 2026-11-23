//! The reads of a Subrun for the panel (ADR 0103 section 3A, "The reads"): the state of a
//! Subrun, and the entries of its RunSteps from its stored responses and ToolCalls. An entry has
//! no Chapter text, no passage text, and no Memory Entry text.

use serde_json::Value;

use crate::{
    AgentRunStatus, ManuscriptUse, READ_MEMORY_ENTRY_TOOL_SPEC, SEARCH_MEMORY_TOOL_SPEC,
    StoredManuscriptCall, SubrunEnd, ToolCallState, ToolIntake,
};

/// The state of a Subrun for the author.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubrunState {
    /// The Subrun did not end. The Subrun of a paused parent Run is in this state.
    Working,
    /// The Subrun ended `completed` with a final answer.
    Completed,
    /// Each other end that is not a cancellation.
    NoResult,
    /// The Subrun is `cancelled`.
    Stopped,
}

/// The state of a Subrun at `end`.
pub fn subrun_state(end: SubrunEnd<'_>) -> SubrunState {
    if crate::subrun_works(end.status) {
        return SubrunState::Working;
    }
    match (end.status, end.final_answer) {
        (AgentRunStatus::Cancelled, _) => SubrunState::Stopped,
        (AgentRunStatus::Completed, Some(answer)) if !answer.is_empty() => SubrunState::Completed,
        _ => SubrunState::NoResult,
    }
}

/// One entry of the RunSteps of a Subrun.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubrunStep {
    /// A text that the Subrun wrote in a response beside its function calls.
    Text { text: String },
    /// A settled `read_chapter` call: the Chapter and its title at the time of the read.
    ChapterRead { chapter_id: String, title: String },
    /// A settled `search_manuscript` call: the query and the count of the matches of its result.
    KeywordSearch { query: String, matches: u32 },
    /// A settled `hybrid_search_manuscript` call: the question and the count of the passages of
    /// its result.
    HybridSearch { question: String, passages: u32 },
    /// A settled `read_memory_entry` call: the title of the entry at the time of the read.
    MemoryRead { title: String },
    /// A settled `search_memory` call: the query and the count of the entries of its result.
    MemorySearch { query: String, entries: u32 },
}

/// The entries of the RunSteps of a Subrun, in RunStep order and in the order of the output
/// items. `texts` are the texts of its responses beside function calls, each with its RunStep and
/// the count of the function calls before it in its response. `calls` are its stored ToolCalls,
/// each with its RunStep and its call position. Only a settled call of a read Tool gives an
/// entry, so a `finish` call gives none.
pub fn subrun_steps(
    texts: &[(u64, u64, String)],
    calls: &[(u64, u64, StoredManuscriptCall<'_>)],
) -> Vec<SubrunStep> {
    // A text with `n` calls before it comes before the call at position `n`.
    let texts = (texts.iter())
        .map(|(step, before, text)| ((*step, *before, 0), SubrunStep::Text { text: text.clone() }));
    let calls = (calls.iter())
        .filter_map(|(step, position, call)| Some(((*step, *position, 1), call_step(call)?)));
    let mut steps: Vec<_> = texts.chain(calls).collect();
    // The sort is stable, so two texts with the same place keep their order.
    steps.sort_by_key(|(place, _)| *place);
    steps.into_iter().map(|(_, step)| step).collect()
}

/// The entry of one stored call, or `None` for a call that did not settle and a call of another
/// Tool. Each count comes from the stored result: the header line of a manuscript search, and the
/// entry lines of a Memory search.
fn call_step(call: &StoredManuscriptCall<'_>) -> Option<SubrunStep> {
    if call.state != ToolCallState::Settled {
        return None;
    }
    let result = call.result_text?;
    if call.tool_spec_revision == READ_MEMORY_ENTRY_TOOL_SPEC.revision {
        let title = result
            .lines()
            .find_map(|line| line.strip_prefix("title: "))?;
        return Some(SubrunStep::MemoryRead {
            title: title.to_owned(),
        });
    }
    if call.tool_spec_revision == SEARCH_MEMORY_TOOL_SPEC.revision {
        let Ok(ToolIntake::SearchMemory { query }) = SEARCH_MEMORY_TOOL_SPEC.intake(call.arguments)
        else {
            return None;
        };
        let entries = (result.lines()).filter(|line| line_number(line, "id").is_some());
        return Some(SubrunStep::MemorySearch {
            query,
            entries: u32::try_from(entries.count()).ok()?,
        });
    }
    let header = |name| result.lines().find_map(|line| line_number(line, name));
    Some(
        match crate::manuscript_uses(std::slice::from_ref(call)).pop()? {
            ManuscriptUse::ChapterRead { chapter_id, title } => {
                SubrunStep::ChapterRead { chapter_id, title }
            }
            ManuscriptUse::KeywordSearch { query } => SubrunStep::KeywordSearch {
                query,
                matches: header("matches")?,
            },
            ManuscriptUse::HybridSearch { question } => SubrunStep::HybridSearch {
                question,
                passages: header("passages")?,
            },
        },
    )
}

/// The number `name` of `line` when the line is a JSON object with that number.
fn line_number(line: &str, name: &str) -> Option<u32> {
    let number = serde_json::from_str::<Value>(line)
        .ok()?
        .get(name)?
        .as_u64()?;
    u32::try_from(number).ok()
}

#[cfg(test)]
#[path = "subrun_read_tests.rs"]
mod tests;
