//! The Subrun script of the fake model (ADR 0103 section 3A). An author message
//! `SCRIPT:start_subrun` gets one `start_subrun` call in its first request, with the call
//! identifier `call_start_subrun`. Its brief is `SCRIPT:read_chapter <number>`, where `<number>`
//! is the number of the Current Chapter in the Chapter list, so the Subrun reads the Current
//! Chapter, then calls `finish`, and then gives its final answer. The parent then calls `finish`,
//! and its final answer is `SUBRUN_ANSWER_PREFIX` and the result of each `start_subrun` call, one
//! for each line. `SCRIPT:start_subrun <count>` gets `<count>` such calls in one response, with
//! the call identifiers `call_start_subrun_0`, `call_start_subrun_1`, and so on. The count is at
//! most `SUBRUN_SCRIPT_CALLS_MAX`. The script can follow the compaction prefix of the Host fake
//! (ADR 0093).

use storyos_application::CreateRequest;
use storyos_core::{ContextSourceClass, SUBRUN_REQUESTS_MAX, ToolChoice, ToolOperation};

use crate::create_plan::FakeCreate;
use crate::research_plan::function_call;

/// The author message of the Subrun script.
pub const START_SUBRUN_SCRIPT: &str = "SCRIPT:start_subrun";

/// The title of each Subrun of the script.
pub const SUBRUN_SCRIPT_TITLE: &str = "读当前章";

/// The start of the final answer of the parent Run of the script.
pub const SUBRUN_ANSWER_PREFIX: &str = "The Subrun answered:";

/// The most `start_subrun` calls of one scripted response: one more than the Subrun Requests of
/// one Run, so that a test can see the refusal of the count.
const SUBRUN_SCRIPT_CALLS_MAX: usize = SUBRUN_REQUESTS_MAX as usize + 1;

/// The scripted response of `request`, or `None` when the usual script applies.
pub(crate) fn plan(request: &CreateRequest) -> Option<FakeCreate> {
    let count = call_count(crate::script_message(&request.author_message))?;
    let answers: Vec<&str> = (request.tool_outputs.iter())
        .filter(|output| output.name == ToolOperation::StartSubrun.function_name())
        .map(|output| output.output.as_str())
        .collect();
    match (request.tool_choice, answers.is_empty()) {
        (ToolChoice::None, false) => Some(crate::create_plan::complete(
            &[&[SUBRUN_ANSWER_PREFIX][..], &answers].concat().join("\n"),
        )),
        (ToolChoice::None, true) | (_, false) => None,
        (_, true) => {
            let brief = format!("SCRIPT:read_chapter {}", current_chapter(request));
            let arguments =
                serde_json::json!({ "title": SUBRUN_SCRIPT_TITLE, "brief": brief }).to_string();
            let call = |suffix: &str| function_call(ToolOperation::StartSubrun, suffix, &arguments);
            let items = match count {
                None => vec![call("")],
                Some(count) => (0..count).map(|index| call(&format!("_{index}"))).collect(),
            };
            Some(FakeCreate { items })
        }
    }
}

/// The count of `start_subrun` calls of `script`: `Some(None)` for one call with no index,
/// `Some(Some(count))` for `count` indexed calls, and `None` when the Subrun script does not apply.
fn call_count(script: &str) -> Option<Option<usize>> {
    match script.strip_prefix(START_SUBRUN_SCRIPT)? {
        "" => Some(None),
        count => {
            let count = count.strip_prefix(' ')?.parse::<usize>().ok()?;
            Some(Some((count <= SUBRUN_SCRIPT_CALLS_MAX).then_some(count)?))
        }
    }
}

/// The number of the Current Chapter in the `chapter_list` item of `request`: the first element
/// of the line whose fourth element is `"current"`. A request with no such line gets 1.
fn current_chapter(request: &CreateRequest) -> u64 {
    (request.context.iter())
        .filter(|item| item.source_class == ContextSourceClass::ChapterList)
        .flat_map(|item| item.content.lines())
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|line| line.get(3).and_then(serde_json::Value::as_str) == Some("current"))
        .and_then(|line| line.get(0).and_then(serde_json::Value::as_u64))
        .unwrap_or(1)
}

#[cfg(test)]
#[path = "subrun_plan_tests.rs"]
mod tests;
