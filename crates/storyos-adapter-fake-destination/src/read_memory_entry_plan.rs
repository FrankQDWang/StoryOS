//! The Memory read script of the fake model (ADR 0094 section 4). An author message with a line
//! `SCRIPT:read_memory_entry <number>[,<number>...]` gets one `read_memory_entry` call for each
//! number, in order, in one response, with the call identifiers `call_read_memory_entry_0`,
//! `call_read_memory_entry_1`, and so on. Each number is the JSON text of the argument `entry`.
//! The calls go in the first request, or, when the message also has `SCRIPT:write_memory` lines,
//! in the request that sends the outputs of those writes. A `Search the web: <query>` line adds
//! one `web_search` call before the reads. Each other request gets the usual script
//! of the fake model, so the next request calls `finish`. The script can follow the compaction
//! prefix of the Host fake (ADR 0093).

use storyos_application::CreateRequest;
use storyos_core::{ToolChoice, ToolOperation};

use crate::create_plan::FakeCreate;
use crate::research_plan::function_call;

/// The scripted response of `request`, or `None` when the usual script applies.
pub(crate) fn plan(request: &CreateRequest) -> Option<FakeCreate> {
    let message = crate::script_message(&request.author_message);
    let numbers = message
        .lines()
        .find_map(|line| line.strip_prefix("SCRIPT:read_memory_entry "))?;
    let writes = message
        .lines()
        .any(|line| line.starts_with("SCRIPT:write_memory "));
    let written = |name: &str| name == ToolOperation::WriteMemory.function_name();
    let after_writes = match request.tool_outputs.is_empty() {
        true => !writes,
        false => (request.tool_outputs.iter()).all(|output| written(&output.name)),
    };
    if request.tool_choice == ToolChoice::None || !after_writes {
        return None;
    }
    let reads = numbers.split(',').enumerate().map(|(index, number)| {
        let arguments = format!("{{\"entry\":{}}}", number.trim());
        function_call(
            ToolOperation::ReadMemoryEntry,
            &format!("_{index}"),
            &arguments,
        )
    });
    let search = crate::tool_loop_plan::search_call(request);
    Some(FakeCreate {
        items: search.into_iter().chain(reads).collect(),
    })
}
