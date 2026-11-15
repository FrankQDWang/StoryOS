//! The resource script of the fake model (ADR 0089 section 3). An author message
//! `SCRIPT:read_skill_resource [load|load_beside] <skill> <path>[ <path>...]` gets one
//! `read_skill_resource` call of `<skill>` for each path, in order, in one response, with the call
//! identifiers `call_read_skill_resource_0`, `call_read_skill_resource_1`, and so on. With `load`,
//! the first request gets one `load_skill` call of `<skill>` (`call_load_skill_0`), and the
//! request that sends its output gets the reads. With `load_beside`, the first response has the
//! `load_skill` call and then the reads. With the suffix line `SCRIPT:read_tool_result_again`,
//! the request that sends the outputs of the reads gets one `read_tool_result` call of
//! `call_load_skill_0` (`call_read_tool_result_load`) and one of `call_read_skill_resource_0`
//! (`call_read_tool_result_resource`). Each other request gets the usual script of the fake
//! model, so the next request calls `finish`. The script can follow the compaction prefix of the
//! Host fake (ADR 0093).

use storyos_application::CreateRequest;
use storyos_core::{ToolChoice, ToolOperation};

use crate::create_plan::FakeCreate;
use crate::research_plan::function_call;

/// The suffix line that reads the Skill calls again with `read_tool_result`.
const READ_AGAIN: &str = "SCRIPT:read_tool_result_again";

/// The scripted response of `request`, or `None` when the usual script applies.
pub(crate) fn plan(request: &CreateRequest) -> Option<FakeCreate> {
    let message = crate::script_message(&request.author_message);
    let script = message
        .lines()
        .next()?
        .strip_prefix("SCRIPT:read_skill_resource ")?;
    let mut words = script.split_whitespace().peekable();
    let load = words
        .next_if(|word| matches!(*word, "load" | "load_beside"))
        .unwrap_or_default();
    let skill = words.next()?;
    let loads = ToolOperation::LoadSkill.function_name();
    let reads = ToolOperation::ReadSkillResource.function_name();
    if request.tool_choice == ToolChoice::None {
        return None;
    }
    if message.lines().any(|line| line == READ_AGAIN)
        && request
            .tool_outputs
            .iter()
            .any(|output| output.name == reads)
        && request
            .tool_outputs
            .iter()
            .all(|output| [loads, reads].contains(&output.name.as_str()))
    {
        let read_again = |suffix: &str, call_id: &str| {
            let arguments = serde_json::json!({ "call_id": call_id });
            function_call(
                ToolOperation::ReadToolResult,
                suffix,
                &arguments.to_string(),
            )
        };
        return Some(FakeCreate {
            items: vec![
                read_again("_load", "call_load_skill_0"),
                read_again("_resource", "call_read_skill_resource_0"),
            ],
        });
    }
    if request
        .tool_outputs
        .iter()
        .any(|output| output.name != loads)
    {
        return None;
    }
    let load_call = || {
        let arguments = serde_json::json!({"skill": skill, "reason": crate::LOAD_REASON});
        function_call(ToolOperation::LoadSkill, "_0", &arguments.to_string())
    };
    let first = request.tool_outputs.is_empty();
    let mut items = match (load, first) {
        ("load", true) => {
            return Some(FakeCreate {
                items: vec![load_call()],
            });
        }
        ("load_beside", true) => vec![load_call()],
        ("load", false) | ("", true) => Vec::new(),
        _ => return None,
    };
    items.extend(words.enumerate().map(|(index, path)| {
        let arguments = serde_json::json!({"skill": skill, "path": path});
        function_call(
            ToolOperation::ReadSkillResource,
            &format!("_{index}"),
            &arguments.to_string(),
        )
    }));
    Some(FakeCreate { items })
}
