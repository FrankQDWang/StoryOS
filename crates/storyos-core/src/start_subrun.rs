//! The Host Tool `start_subrun` (ADR 0103 section 3A): a Run of the author gives one reading
//! task to a Subrun and waits for its final answer. The Tool needs no approval: a call is
//! `pending`, then `settled` or `refused`.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AgentRunStatus, IntakeRefusal, ToolCallState, ToolIntake, ToolOperation, ToolResultText,
    ToolSpec,
};

pub const START_SUBRUN_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.start-subrun.v1",
    operation: ToolOperation::StartSubrun,
    intake_contract: &[],
    envelope: None,
    result_text_max_bytes: crate::TOOL_RESULT_TEXT_MAX_BYTES,
};

/// The most characters of the title of a Subrun.
pub const SUBRUN_TITLE_MAX_CHARS: usize = 40;
/// The most characters of the brief of a Subrun.
pub const SUBRUN_BRIEF_MAX_CHARS: usize = 4000;
/// The most Subrun Requests of one Run of the author.
pub const SUBRUN_REQUESTS_MAX: u64 = 4;

pub(crate) const DESCRIPTION: &str = "Give one reading task to a Subrun and wait for its final \
answer. The Subrun has a new context: it sees only the brief, and not this conversation. It can \
read the manuscript and the memory, and it can change nothing. One author request can start at \
most 4 Subruns.";

/// The result of a call whose Subrun ended with no final answer.
pub const SUBRUN_NO_RESULT: &str = "The Subrun ended with no result.";

/// The result of a call whose Subrun ended because its request was too long.
pub const SUBRUN_TOO_LONG: &str = "The Subrun ended with no result, because its reading was too \
long for one Subrun. Give a smaller task.";

/// The strict JSON Schema of the arguments.
pub(crate) fn parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "title": {
                "type": "string",
                "description": format!(
                    "A short title of the task on one line, at most {SUBRUN_TITLE_MAX_CHARS} \
                     characters."
                )
            },
            "brief": {
                "type": "string",
                "description": format!(
                    "The complete task, at most {SUBRUN_BRIEF_MAX_CHARS} characters."
                )
            }
        },
        "required": ["title", "brief"],
        "additionalProperties": false
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    title: String,
    brief: String,
}

/// The typed intake. The ToolCall record cannot hold U+0000, so a text with it does not fit. The
/// rules of a call are not part of the intake: the Host refuses a call that breaks one of them
/// with `subrun_refusal`.
pub(crate) fn intake(arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
    let Arguments { title, brief } =
        Arguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
    match title.contains('\0') || brief.contains('\0') {
        true => Err(IntakeRefusal::UnstorableText),
        false => Ok(ToolIntake::StartSubrun { title, brief }),
    }
}

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

/// The first rule that a call with `title` and `brief` breaks, in the order of the results, for a
/// Run with `requests` Subrun Requests: a title of 1 through 40 characters on one line that is
/// not only white space, a brief of 1 through 4000 characters that is not only white space, and
/// fewer than 4 Subrun Requests. The caller examines a call that has no Subrun Request only.
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

/// The result of the call of the parent for its Subrun at `end`, or `None` while the Subrun is
/// `queued` or `claimed`: then the call continues the same Subrun. No command continues a Subrun,
/// so each other status is its end. The result is the final answer text of a completed Subrun
/// when the ToolCall record can hold it, and else a fixed sentence.
pub fn subrun_result(end: SubrunEnd<'_>) -> Option<ToolResultText> {
    if matches!(end.status, AgentRunStatus::Queued | AgentRunStatus::Claimed) {
        return None;
    }
    let answer = (end.final_answer)
        .filter(|text| !text.is_empty() && text.len() <= crate::TOOL_RESULT_TEXT_MAX_BYTES);
    let text = match (end.status, answer) {
        (AgentRunStatus::Completed, Some(answer)) => answer,
        _ if end.too_long => SUBRUN_TOO_LONG,
        _ => SUBRUN_NO_RESULT,
    };
    Some(crate::host_tool_result(text))
}

#[cfg(test)]
#[path = "start_subrun_tests.rs"]
mod tests;
