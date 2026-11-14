//! The read script of the fake model (ADR 0093). An author message
//! `SCRIPT:read_tool_result <first> <identity>` gets, in its first request, one `web_search`
//! call (`web_search`), one `web_search` call with an empty query that the Host refuses
//! (`refused_search`), or no call (`none`). The next request gets one `read_tool_result` call
//! with the `<identity>`. The request that carries the output of the read gets a `submit_research`
//! call that cites the first source of that output and a `finish` call, or only a `finish` call
//! when the output has no source. Each later request gets the usual script of the fake model.
//! The script can follow the compaction prefix of the Host fake (ADR 0093).

use storyos_application::CreateRequest;
use storyos_core::{ToolChoice, ToolOperation};

use crate::create_plan::FakeCreate;
use crate::research_plan::function_call;

/// The scripted response of `request`, or `None` when the usual script applies.
pub(crate) fn plan(request: &CreateRequest) -> Option<FakeCreate> {
    let script =
        crate::script_message(&request.author_message).strip_prefix("SCRIPT:read_tool_result ")?;
    let (first, identity) = script.split_once(' ')?;
    if request.tool_choice == ToolChoice::None {
        return None;
    }
    let output = |operation: ToolOperation| {
        request
            .tool_outputs
            .iter()
            .find(|output| output.name == operation.function_name())
    };
    let read = function_call(
        ToolOperation::ReadToolResult,
        "",
        &serde_json::json!({"call_id": identity}).to_string(),
    );
    let finish = function_call(ToolOperation::Finish, "", "{}");
    let items = match output(ToolOperation::ReadToolResult) {
        None if request.tool_outputs.is_empty() => match first {
            "web_search" => vec![function_call(
                ToolOperation::WebSearch,
                "",
                r#"{"query":"tide tables of 1890","result_count":null}"#,
            )],
            "refused_search" => vec![function_call(
                ToolOperation::WebSearch,
                "",
                r#"{"query":"","result_count":null}"#,
            )],
            _ => vec![read],
        },
        None => vec![read],
        Some(_) if output(ToolOperation::Finish).is_some() => return None,
        Some(read) => {
            let source = read
                .output
                .split("<source id=\"")
                .nth(1)
                .and_then(|rest| rest.split('"').next());
            let submit = source.map(|source| {
                let arguments = serde_json::json!({"summary": "The tides of 1890.", "claims": [
                    {"text": "High water came at 10.42.", "citations": [source]}]});
                function_call(ToolOperation::SubmitResearch, "", &arguments.to_string())
            });
            submit.into_iter().chain([finish]).collect()
        }
    };
    Some(FakeCreate { items })
}
