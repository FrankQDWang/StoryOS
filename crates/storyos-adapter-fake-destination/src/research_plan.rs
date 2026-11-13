//! The research script of the tool loop profile. An author message
//! `SCRIPT:submit_research <variant> <first tool> <arguments>` gets one call of `<first tool>`
//! (`web_search` or `web_fetch`) in the first request. The request that carries its output gets
//! `submit_research` calls with the `<arguments>` text, in which `$SOURCE` and `$GAP` become the
//! first source and the first gap identity of the `tool_sources` tags of the outputs:
//!
//! - `once`: one call and a `finish` call.
//! - `twice`: two calls and a `finish` call in one response.
//! - `finish_first`: a `finish` call and one call, in that order.
//! - `again`: one call, and in the next request a second call and a `finish` call.
//!
//! Each later request gets the usual script of the tool loop profile.

use storyos_application::CreateRequest;
use storyos_core::{NativeStreamItem, StreamItemRole, StreamItemState, ToolChoice, ToolOperation};

use crate::create_plan::{FakeCreate, evidence_item};

const SEARCH: &str = r#"{"query":"tide tables of 1890","result_count":null}"#;
const FETCH: &str = r#"{"urls":["https://example.org/c","https://example.org/d"]}"#;

/// The scripted response of `request`, or `None` when the usual script applies.
pub(crate) fn plan(request: &CreateRequest) -> Option<FakeCreate> {
    if request.tool_choice == ToolChoice::None {
        return None;
    }
    let script = request
        .author_message
        .strip_prefix("SCRIPT:submit_research ")?;
    let mut parts = script.splitn(3, ' ');
    let (variant, first, arguments) = (parts.next()?, parts.next()?, parts.next()?);
    let output = |operation: ToolOperation| {
        request
            .tool_outputs
            .iter()
            .any(|output| output.name == operation.function_name())
    };
    let submit = |suffix: &str, arguments: &str| {
        function_call(ToolOperation::SubmitResearch, suffix, arguments)
    };
    let finish = function_call(ToolOperation::Finish, "", "{}");
    let items = if request.tool_outputs.is_empty() {
        let first = match first {
            "web_fetch" => function_call(ToolOperation::WebFetch, "", FETCH),
            _ => function_call(ToolOperation::WebSearch, "", SEARCH),
        };
        vec![first]
    } else if !output(ToolOperation::SubmitResearch) {
        let arguments = arguments
            .replace("$SOURCE", &first_tagged(request, "<source id=\""))
            .replace("$GAP", &first_tagged(request, "<gap id=\""));
        match variant {
            "twice" => vec![submit("", &arguments), submit("_2", &arguments), finish],
            "finish_first" => vec![finish, submit("", &arguments)],
            "again" => vec![submit("", &arguments)],
            _ => vec![submit("", &arguments), finish],
        }
    } else if variant == "again" && !output(ToolOperation::Finish) {
        vec![submit("_2", arguments), finish]
    } else {
        return None;
    };
    Some(FakeCreate {
        items,
        output: None,
    })
}

fn function_call(operation: ToolOperation, suffix: &str, arguments: &str) -> NativeStreamItem {
    let name = operation.function_name();
    NativeStreamItem {
        item_id: format!("fc_{name}{suffix}"),
        call_id: Some(format!("call_{name}{suffix}")),
        arguments: Some(arguments.to_owned()),
        native_type: Some(storyos_core::FUNCTION_CALL_NATIVE_TYPE.to_owned()),
        name: Some(name.to_owned()),
        ..evidence_item(
            StreamItemRole::Tool,
            StreamItemState::Complete,
            "host_fake_function_call",
        )
    }
}

/// The first identity after `tag` in the function outputs of `request`, or an empty text.
fn first_tagged(request: &CreateRequest, tag: &str) -> String {
    request
        .tool_outputs
        .iter()
        .find_map(|output| output.output.split(tag).nth(1))
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_default()
        .to_owned()
}
