//! The shared test support from `agent_run_tool_call_tests.rs`.

use crate::statement_cache::Client;
use serde_json::json;
use storyos_core::EXA_TOOL_REGISTRATION;

/// A stream that reports `reference`, one complete message, and one complete function call with
/// `call_id`, and ends with the terminal `status` of the response.
pub(crate) fn text_and_call_stream(
    reference: &str,
    call_id: &str,
    name: &str,
    arguments: &str,
    status: &str,
) -> String {
    let message = json!({"id": "msg_01", "type": "message", "role": "assistant",
        "status": "completed", "content": [{"type": "output_text", "text": "Looking it up."}]});
    let call = json!({"id": "fc_01", "type": "function_call", "status": "completed",
        "call_id": call_id, "name": name, "arguments": arguments});
    let events = [
        json!({"type": "response.created",
               "response": {"id": reference, "status": "in_progress", "output": []}}),
        json!({"type": "response.output_item.done", "output_index": 0, "item": message}),
        json!({"type": "response.output_item.done", "output_index": 1, "item": call}),
        json!({"type": format!("response.{status}"), "response": {
            "id": reference, "status": status, "output": [message, call],
            "usage": {"input_tokens": 90, "output_tokens": 30}}}),
    ];
    let mut reply = String::from(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
    );
    for event in events {
        reply.push_str(&format!("data: {event}\n\n"));
    }
    reply
}

/// The administration statement of a Tool Contract Drift that quarantines the pinned research
/// revision while the model response is pending.
pub(crate) fn quarantine_statement() -> String {
    format!(
        "UPDATE storyos.tool_registration_revisions
            SET lifecycle = 'quarantined', quarantined_at = clock_timestamp(),
                quarantine_differences = '[{{\"kind\": \"tool_removed\", \"tool\": \"web_fetch_exa\"}}]'
          WHERE tool_registration_revision = '{}'",
        EXA_TOOL_REGISTRATION.revision
    )
}

/// Makes the shared research revision active again after a quarantine, so that later tests
/// expose it.
pub(crate) async fn reactivate_research_revision(admin: &Client) {
    admin
        .execute(
            "UPDATE storyos.tool_registration_revisions
                SET lifecycle = 'active', quarantine_differences = NULL, quarantined_at = NULL
              WHERE tool_registration_revision = $1::text::uuid",
            &[&EXA_TOOL_REGISTRATION.revision],
        )
        .await
        .unwrap();
}
