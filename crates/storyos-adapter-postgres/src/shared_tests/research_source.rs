//! The shared test support from `research_source_tests.rs`.

use crate::shared_tests::agent_run_tool_exposure::sse;
use serde_json::{Value, json};

/// A search result in the text format of the research Tool: two entries with a URL each.
pub(crate) const SEARCH_TEXT: &str = "Title: Liverpool tide table 1890\nURL: https://example.org/tides\n\
Published: 1890-01-01T00:00:00.000Z\nAuthor: N/A\nHighlights:\nHigh water at 10.42.\n\n---\n\n\
Title: Port log\nURL: https://example.org/port\nPublished: N/A\nAuthor: N/A\nHighlights:\n\
The port log.";

/// One response that requests each `(call_id, name, arguments)` call in order.
pub(crate) fn calls_stream(reference: &str, calls: &[(&str, &str, &str)]) -> String {
    calls_stream_in(reference, calls, "completed")
}

/// As `calls_stream`, with the terminal event and the response in `status`.
pub(crate) fn calls_stream_in(
    reference: &str,
    calls: &[(&str, &str, &str)],
    status: &str,
) -> String {
    let items: Vec<Value> = calls
        .iter()
        .enumerate()
        .map(|(index, (call_id, name, arguments))| {
            json!({"id": format!("fc_{index}"), "type": "function_call", "status": "completed",
                   "call_id": call_id, "name": name, "arguments": arguments})
        })
        .collect();
    let mut events = vec![json!({"type": "response.created",
        "response": {"id": reference, "status": "in_progress", "output": []}})];
    events.extend(items.iter().enumerate().map(|(index, item)| {
        json!({"type": "response.output_item.done", "output_index": index, "item": item})
    }));
    events.push(json!({"type": format!("response.{status}"), "response": {
        "id": reference, "status": status, "output": items,
        "usage": {"input_tokens": 90, "output_tokens": 30}}}));
    sse(&events)
}
