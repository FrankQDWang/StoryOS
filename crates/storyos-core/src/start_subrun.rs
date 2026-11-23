//! The rules of the Host Tool `start_subrun` (ADR 0103 section 3A): a Run of the author gives one
//! reading task to a Subrun and waits for its final answer. The Tool needs no approval: a call is
//! `pending`, then `settled` or `refused`.

use crate::{AgentRunStatus, ToolCallState, ToolResultText};

/// The most characters of the title of a Subrun.
pub const SUBRUN_TITLE_MAX_CHARS: usize = 40;
/// The most characters of the brief of a Subrun.
pub const SUBRUN_BRIEF_MAX_CHARS: usize = 4000;
/// The most Subrun Requests of one Run of the author.
pub const SUBRUN_REQUESTS_MAX: u64 = 4;

/// The description of the Tool for the model (item 1 of #436).
pub const START_SUBRUN_DESCRIPTION: &str = "Give one reading task to a Subrun and wait for its \
final answer. The Subrun has a new context: it sees only the brief, and not this conversation. It \
can read the manuscript and the memory, and it can change nothing. One author request can start \
at most 4 Subruns.";

/// The result of a call whose Subrun ended with no final answer.
pub const SUBRUN_NO_RESULT: &str = "The Subrun ended with no result.";

/// The result of a call whose Subrun ended because its request was too long.
pub const SUBRUN_TOO_LONG: &str = "The Subrun ended with no result, because its reading was too \
long for one Subrun. Give a smaller task.";

/// Why the Host refused one `start_subrun` call by its rules. A call that a rule refuses has no
/// Subrun.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubrunRefusal {
    Title,
    Brief,
    RequestCount,
}

impl SubrunRefusal {
    /// The stable label of the refusal, for the `refusal` of the ToolCall row.
    pub fn label(self) -> &'static str {
        match self {
            Self::Title => "subrun_title",
            Self::Brief => "subrun_brief",
            Self::RequestCount => "subrun_request_count",
        }
    }

    /// The result of the refused call.
    pub fn result_text(self) -> &'static str {
        match self {
            Self::Title => "The title must have 1 through 40 characters on one line.",
            Self::Brief => "The brief must have 1 through 4000 characters.",
            Self::RequestCount => "One author request can start at most 4 Subruns.",
        }
    }
}

/// The first rule that a call with `title` and `brief` breaks, for a Run with `requests` Subrun
/// Requests. The rules are in the order of the results. The title has 1 through 40 characters on
/// one line, and it is not only white space. The brief has 1 through 4000 characters, and it is
/// not only white space. The Run has fewer than 4 Subrun Requests. The caller examines only a
/// call that has no Subrun Request.
pub fn subrun_refusal(title: &str, brief: &str, requests: u64) -> Option<SubrunRefusal> {
    let text = |text: &str, max: usize| !text.trim().is_empty() && text.chars().count() <= max;
    if !text(title, SUBRUN_TITLE_MAX_CHARS) || title.chars().any(crate::update_plan::line_break) {
        return Some(SubrunRefusal::Title);
    }
    if !text(brief, SUBRUN_BRIEF_MAX_CHARS) {
        return Some(SubrunRefusal::Brief);
    }
    (requests >= SUBRUN_REQUESTS_MAX).then_some(SubrunRefusal::RequestCount)
}

/// The refused resolution of a call that breaks `refusal`: its state, its refusal label, and its
/// result.
pub fn refused_subrun_call(
    refusal: SubrunRefusal,
) -> (ToolCallState, Option<&'static str>, ToolResultText) {
    (
        ToolCallState::Refused,
        Some(refusal.label()),
        crate::host_tool_result(refusal.result_text()),
    )
}

/// How a Subrun ended: its status, its final answer text, and whether its request was too long.
#[derive(Clone, Copy, Debug)]
pub struct SubrunEnd<'a> {
    pub status: AgentRunStatus,
    pub final_answer: Option<&'a str>,
    pub too_long: bool,
}

/// Whether a Subrun with `status` works: it is `queued` or `claimed`. No command continues a
/// Subrun, so each other status is its end. A call continues a Subrun that works, and the
/// cancellation of the parent cancels it.
pub fn subrun_works(status: AgentRunStatus) -> bool {
    matches!(status, AgentRunStatus::Queued | AgentRunStatus::Claimed)
}

/// The result of the call of the parent for its Subrun at `end`, or `None` while the Subrun
/// works. The result is the final answer text of a completed Subrun when the ToolCall record can
/// hold it (ADR 0103 section 3A). A Subrun refused because its request was too long gives
/// `SUBRUN_TOO_LONG`. Each other end gives `SUBRUN_NO_RESULT`.
pub fn subrun_result(end: SubrunEnd<'_>) -> Option<ToolResultText> {
    if subrun_works(end.status) {
        return None;
    }
    let answer = (end.final_answer)
        .filter(|text| !text.is_empty() && text.len() <= crate::TOOL_RESULT_TEXT_MAX_BYTES);
    let text = match (end.status, answer) {
        (AgentRunStatus::Completed, Some(answer)) => answer,
        (AgentRunStatus::Refused, _) if end.too_long => SUBRUN_TOO_LONG,
        _ => SUBRUN_NO_RESULT,
    };
    Some(crate::host_tool_result(text))
}

#[cfg(test)]
#[path = "start_subrun_tests.rs"]
mod tests;
