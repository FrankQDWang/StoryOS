//! The script of the tool loop profile (ADR 0091): the fake calls `finish` in each request with
//! `tool_choice: "required"`, and it writes the final answer in the closing request. Other author
//! messages script the other cases of the finish rule and the RunStep limit:
//!
//! - A message that ends with `SCRIPT:text_before_finish` gets a text with no function call when
//!   each function output of the request answers the prior response.
//! - `SCRIPT:finish_twice` gets a second `finish` call in the first closing request.
//! - `SCRIPT:empty_final_answer` gets a closing response with an empty text.
//! - `SCRIPT:reply_form_answer` gets a closing text that maps to a reply form question.
//! - `SCRIPT:refuse <name> <arguments>` gets a call of `<name>` with the `<arguments>` text
//!   before the `finish` call of the first request. The later requests are as usual, so the
//!   model corrects the call.
//! - `SCRIPT:never_finish` gets a call of an unexposed name beside each `finish` call, so no
//!   `finish` call counts. Its request with `tool_choice: "none"` gets a text and a `finish`
//!   call, and `SCRIPT:never_finish_quietly` gets only the `finish` call.
//! - `SCRIPT:submit_research ...` gets the research script of `research_plan`.

use storyos_application::{CreateRequest, ToolOutputOrigin};
use storyos_core::{
    ADVISORY_TEXT, DecisionCandidate, NativeStreamItem, OutputPhase, StreamItemRole,
    StreamItemState, ToolChoice, ToolOperation,
};

use crate::create_plan::{FakeCreate, assistant_item, complete, evidence_item};

/// The text of a response with no function call before a `finish` call that counted.
const TEXT_BEFORE_FINISH: &str = "I have read the results.";

/// The final answer of the last RunStep of a Run that never finishes.
pub const LIMIT_ANSWER: &str = "I stop here at the step limit.";

/// The function name of `SCRIPT:never_finish` that no Run exposes.
const UNEXPOSED_NAME: &str = "look_up_notes";

pub(crate) fn plan(request: &CreateRequest) -> FakeCreate {
    if let Some(planned) = crate::research_plan::plan(request) {
        return planned;
    }
    let message = request.author_message.as_str();
    let finished = request
        .tool_outputs
        .iter()
        .filter(|output| output.name == ToolOperation::Finish.function_name())
        .count();
    let never_finish = matches!(
        message,
        "SCRIPT:never_finish" | "SCRIPT:never_finish_quietly"
    );
    match request.tool_choice {
        ToolChoice::None if never_finish => {
            let mut planned = calls(request, finished);
            if message == "SCRIPT:never_finish" {
                planned.items.insert(
                    0,
                    assistant_item("1", StreamItemState::Complete, LIMIT_ANSWER),
                );
            }
            planned
        }
        ToolChoice::None if message == "SCRIPT:finish_twice" && finished == 1 => {
            calls(request, finished)
        }
        ToolChoice::None if message == "SCRIPT:reply_form_answer" => complete(
            DecisionCandidate::Clarification {
                question: "Which chapter do you mean?".to_owned(),
            },
            OutputPhase::FinalAnswer,
        ),
        ToolChoice::None => complete(
            DecisionCandidate::Advisory {
                text: match message {
                    "SCRIPT:empty_final_answer" => String::new(),
                    _ => ADVISORY_TEXT.to_owned(),
                },
            },
            OutputPhase::FinalAnswer,
        ),
        ToolChoice::Auto | ToolChoice::Required
            if message.ends_with("SCRIPT:text_before_finish")
                && !request.tool_outputs.is_empty()
                && request
                    .tool_outputs
                    .iter()
                    .all(|output| output.origin == ToolOutputOrigin::PriorResponse) =>
        {
            complete(
                DecisionCandidate::Advisory {
                    text: TEXT_BEFORE_FINISH.to_owned(),
                },
                OutputPhase::FinalAnswer,
            )
        }
        ToolChoice::Auto | ToolChoice::Required => calls(request, finished),
    }
}

/// A `finish` call and no text. The first request of a Run with an author message that holds
/// `Search the web: <query>` also gets one `web_search` call first when the Run exposes it, and
/// the first request of `SCRIPT:refuse` gets its scripted call first.
fn calls(request: &CreateRequest, finished: usize) -> FakeCreate {
    let call = |name: &str, suffix: String, arguments: String| NativeStreamItem {
        item_id: format!("fc_{name}{suffix}"),
        call_id: Some(format!("call_{name}{suffix}")),
        arguments: Some(arguments),
        native_type: Some(storyos_core::FUNCTION_CALL_NATIVE_TYPE.to_owned()),
        name: Some(name.to_owned()),
        ..evidence_item(
            StreamItemRole::Tool,
            StreamItemState::Complete,
            "host_fake_function_call",
        )
    };
    // The script can follow the compaction prefix.
    let search = request
        .author_message
        .split_once("Search the web: ")
        .map(|(_, query)| query)
        .filter(|_| {
            request.tool_outputs.is_empty()
                && request
                    .tool_exposure
                    .tools
                    .iter()
                    .any(|tool| tool.spec.operation == ToolOperation::WebSearch)
        })
        .map(|query| {
            let arguments = serde_json::json!({"query": query, "result_count": null});
            call("web_search", String::new(), arguments.to_string())
        });
    // Each `finish` call of a Run has its own call identifier.
    let suffix = match finished {
        0 => String::new(),
        count => format!("_{count}"),
    };
    let refused = request
        .author_message
        .strip_prefix("SCRIPT:refuse ")
        .and_then(|script| script.split_once(' '))
        .filter(|_| request.tool_outputs.is_empty())
        .map(|(name, arguments)| call(name, "_refused".to_owned(), arguments.to_owned()));
    let unexposed = request
        .author_message
        .starts_with("SCRIPT:never_finish")
        .then(|| call(UNEXPOSED_NAME, suffix.clone(), "{}".to_owned()));
    let (proposed, finish) =
        crate::propose_edit_plan::calls(request, &call).unwrap_or((Vec::new(), true));
    FakeCreate {
        items: search
            .into_iter()
            .chain(refused)
            .chain(proposed)
            .chain(unexposed)
            .chain(finish.then(|| call("finish", suffix, "{}".to_owned())))
            .collect(),
        output: None,
    }
}
