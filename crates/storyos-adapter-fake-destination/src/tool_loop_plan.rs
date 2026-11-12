//! The script of the tool loop profile (ADR 0091): the fake calls `finish` in each request with
//! `tool_choice: "required"`, and it writes the final answer in the closing request. Four author
//! messages script the other cases of the finish rule:
//!
//! - A message that ends with `SCRIPT:text_before_finish` gets a text with no function call when
//!   each function output of the request answers the prior response.
//! - `SCRIPT:finish_twice` gets a second `finish` call in the first closing request.
//! - `SCRIPT:empty_final_answer` gets a closing response with an empty text.
//! - `SCRIPT:reply_form_answer` gets a closing text that maps to a reply form question.

use storyos_application::{CreateRequest, ToolOutputOrigin};
use storyos_core::{
    ADVISORY_TEXT, DecisionCandidate, NativeStreamItem, OutputPhase, StreamItemRole,
    StreamItemState, ToolChoice, ToolOperation,
};

use crate::create_plan::{FakeCreate, complete, evidence_item};

/// The text of a response with no function call before a `finish` call that counted.
const TEXT_BEFORE_FINISH: &str = "I have read the results.";

pub(crate) fn plan(request: &CreateRequest) -> FakeCreate {
    let message = request.author_message.as_str();
    let finished = request
        .tool_outputs
        .iter()
        .filter(|output| output.name == ToolOperation::Finish.function_name())
        .count();
    match request.tool_choice {
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

/// A `finish` call and no text. The first request of a Run with the author message
/// `Search the web: <query>` also gets one `web_search` call first when the Run exposes it.
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
    let search = request
        .author_message
        .strip_prefix("Search the web: ")
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
    FakeCreate {
        items: search
            .into_iter()
            .chain([call("finish", suffix, "{}".to_owned())])
            .collect(),
        output: None,
    }
}
