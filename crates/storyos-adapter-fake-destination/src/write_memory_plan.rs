//! The Memory write script of the fake model (ADR 0094 section 5). Each line
//! `SCRIPT:write_memory <action> <JSON arguments>` of an author message gets one `write_memory`
//! call in the first request, in line order and in one response, with the call identifiers
//! `call_write_memory_0`, `call_write_memory_1`, and so on. The JSON object gives the other
//! arguments, and each argument that it does not give is null. A `Search the web: <query>` line
//! after the script lines adds one `web_search` call before them. Each other request gets the
//! usual script of the fake model, so the next request calls `finish`.

use storyos_application::CreateRequest;
use storyos_core::{ToolChoice, ToolOperation};

use crate::create_plan::FakeCreate;
use crate::research_plan::function_call;

/// The scripted response of `request`, or `None` when the usual script applies.
pub(crate) fn plan(request: &CreateRequest) -> Option<FakeCreate> {
    if request.tool_choice == ToolChoice::None || !request.tool_outputs.is_empty() {
        return None;
    }
    let writes: Vec<_> = crate::script_message(&request.author_message)
        .lines()
        .filter_map(|line| line.strip_prefix("SCRIPT:write_memory "))
        .enumerate()
        .map(|(index, line)| {
            let (action, given) = line.split_once(' ').unwrap_or((line, "{}"));
            let mut arguments = serde_json::json!({
                "action": action, "entry": null, "type": null, "title": null,
                "description": null, "text": null,
            });
            if let Ok(serde_json::Value::Object(given)) = serde_json::from_str(given) {
                arguments
                    .as_object_mut()
                    .into_iter()
                    .for_each(|map| map.extend(given.clone()));
            }
            let arguments = arguments.to_string();
            function_call(ToolOperation::WriteMemory, &format!("_{index}"), &arguments)
        })
        .collect();
    let search = crate::tool_loop_plan::search_call(request);
    (!writes.is_empty()).then(|| FakeCreate {
        items: search.into_iter().chain(writes).collect(),
    })
}
