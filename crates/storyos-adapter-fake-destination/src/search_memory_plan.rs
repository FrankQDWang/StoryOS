//! The Memory search script of the fake model (ADR 0094 section 4). An author message with a line
//! `SCRIPT:search_memory <term>` gets one `search_memory` call with the rest of the line as its
//! query, with the call identifier `call_search_memory`, in its first request. Each other
//! request gets the usual script of the fake model, so the next request calls `finish`. The
//! script can follow the compaction prefix of the Host fake (ADR 0093).

use storyos_application::CreateRequest;
use storyos_core::{ToolChoice, ToolOperation};

use crate::create_plan::FakeCreate;
use crate::research_plan::function_call;

/// The scripted response of `request`, or `None` when the usual script applies.
pub(crate) fn plan(request: &CreateRequest) -> Option<FakeCreate> {
    let message = crate::script_message(&request.author_message);
    let term = message
        .lines()
        .find_map(|line| line.strip_prefix("SCRIPT:search_memory "))?;
    if request.tool_choice == ToolChoice::None || !request.tool_outputs.is_empty() {
        return None;
    }
    let arguments = serde_json::json!({ "query": term }).to_string();
    Some(FakeCreate {
        items: vec![function_call(ToolOperation::SearchMemory, "", &arguments)],
    })
}
