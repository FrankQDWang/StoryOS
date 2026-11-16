//! The manuscript Tool uses of one Run, for the Run read (ADR 0095 section 1). The record comes
//! from the stored `read_chapter` and `search_manuscript` calls of the Run. It has no table.

use serde_json::Value;

use crate::{READ_CHAPTER_TOOL_SPEC, SEARCH_MANUSCRIPT_TOOL_SPEC, ToolCallState, ToolIntake};

/// One manuscript Tool use of a Run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManuscriptUse {
    /// The Agent read the Chapter `chapter_id`, which had the title `title` at the time of the
    /// read.
    ChapterRead { chapter_id: String, title: String },
    /// The Agent searched the manuscript for the exact text `query`.
    KeywordSearch { query: String },
}

/// One stored ToolCall of a Run: its ToolSpec revision, its state, its stored arguments, and the
/// text of its stored result.
#[derive(Clone, Copy, Debug)]
pub struct StoredManuscriptCall<'a> {
    pub tool_spec_revision: &'a str,
    pub state: ToolCallState,
    pub arguments: &'a Value,
    pub result_text: Option<&'a str>,
}

/// The manuscript Tool uses of a Run from its stored calls `calls` in item order: one entry for
/// each Chapter that a settled `read_chapter` call gave and one entry for each query of a settled
/// `search_manuscript` call, in the order of the first use. A call that did not settle and a call
/// of another Tool give no entry.
pub fn manuscript_uses(calls: &[StoredManuscriptCall<'_>]) -> Vec<ManuscriptUse> {
    let mut uses: Vec<ManuscriptUse> = Vec::new();
    for call in calls {
        let entry = match call.state {
            ToolCallState::Settled => settled_use(call),
            ToolCallState::Pending
            | ToolCallState::Dispatched
            | ToolCallState::Failed
            | ToolCallState::Unknown
            | ToolCallState::Refused
            | ToolCallState::AwaitingApproval => None,
        };
        if let Some(entry) = entry
            && !uses.iter().any(|known| same_target(known, &entry))
        {
            uses.push(entry);
        }
    }
    uses
}

/// The use of one settled call, or `None` for a call of another Tool.
fn settled_use(call: &StoredManuscriptCall<'_>) -> Option<ManuscriptUse> {
    if call.tool_spec_revision == READ_CHAPTER_TOOL_SPEC.revision {
        // The first line of a settled read is its header, with the Chapter at the time of the
        // read.
        let header: Value = serde_json::from_str(call.result_text?.lines().next()?).ok()?;
        return Some(ManuscriptUse::ChapterRead {
            chapter_id: header.get("chapter_id")?.as_str()?.to_owned(),
            title: header.get("title")?.as_str()?.to_owned(),
        });
    }
    if call.tool_spec_revision == SEARCH_MANUSCRIPT_TOOL_SPEC.revision {
        return match SEARCH_MANUSCRIPT_TOOL_SPEC.intake(call.arguments) {
            Ok(ToolIntake::SearchManuscript { query }) => {
                Some(ManuscriptUse::KeywordSearch { query })
            }
            // A settled search always has valid arguments.
            Ok(_) | Err(_) => None,
        };
    }
    None
}

/// Two uses of one Chapter or of one query are one entry.
fn same_target(known: &ManuscriptUse, entry: &ManuscriptUse) -> bool {
    match (known, entry) {
        (
            ManuscriptUse::ChapterRead { chapter_id, .. },
            ManuscriptUse::ChapterRead {
                chapter_id: other, ..
            },
        ) => chapter_id == other,
        (ManuscriptUse::KeywordSearch { query }, ManuscriptUse::KeywordSearch { query: other }) => {
            query == other
        }
        (ManuscriptUse::ChapterRead { .. }, ManuscriptUse::KeywordSearch { .. })
        | (ManuscriptUse::KeywordSearch { .. }, ManuscriptUse::ChapterRead { .. }) => false,
    }
}

#[cfg(test)]
#[path = "manuscript_use_tests.rs"]
mod tests;
