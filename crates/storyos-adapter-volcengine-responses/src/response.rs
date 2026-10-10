//! Map one complete Responses object to the typed Model Gateway observation.

use serde_json::Value;
use storyos_application::{DeclaredTarget, ModelResponse, ModelUsage, Observation};
use storyos_core::{NativeStreamItem, StreamItemRole, StreamItemState};

/// The observation of a successful HTTP response body. A body that is not a Responses object
/// is an unknown outcome, because the destination can have processed the request.
pub(crate) fn observe(body: &[u8], targets: &[DeclaredTarget]) -> Observation {
    let Ok(response) = serde_json::from_slice::<Value>(body) else {
        return Observation::OutcomeUnknown {
            response_reference: None,
        };
    };
    let Some(status) = response_state(&response) else {
        return Observation::OutcomeUnknown {
            response_reference: None,
        };
    };
    let items: Vec<_> = response
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(index, item)| native_item(index, item, status))
        .collect();
    let output = (status == StreamItemState::Complete)
        .then(|| final_text(&items))
        .flatten()
        .map(|text| crate::decision::map_output(&text, targets));
    Observation::Terminal(ModelResponse {
        items,
        output,
        usage: usage(&response),
        response_reference: response
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

/// The state of a terminal response. Another status is not a terminal response.
fn response_state(response: &Value) -> Option<StreamItemState> {
    match response.get("status")?.as_str()? {
        "completed" => Some(StreamItemState::Complete),
        "incomplete" => Some(StreamItemState::Incomplete),
        "failed" => Some(StreamItemState::Failed),
        "cancelled" => Some(StreamItemState::Cancelled),
        _ => None,
    }
}

fn native_item(index: usize, item: &Value, response_state: StreamItemState) -> NativeStreamItem {
    let text_of = |key: &str, kind: &str, field: &str| {
        let parts: Vec<_> = item
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|part| part.get("type").and_then(Value::as_str) == Some(kind))
            .filter_map(|part| part.get(field).and_then(Value::as_str))
            .collect();
        (!parts.is_empty()).then(|| parts.concat())
    };
    let string = |field: &str| item.get(field).and_then(Value::as_str).map(str::to_owned);
    let kind = item.get("type").and_then(Value::as_str).unwrap_or_default();
    let mut native = NativeStreamItem {
        item_id: string("id").unwrap_or_else(|| index.to_string()),
        role: StreamItemRole::Assistant,
        state: item_state(item, response_state),
        text: None,
        summary: None,
        call_id: None,
        arguments: None,
        refusal: None,
        hosted_report: None,
    };
    match kind {
        "message" => {
            native.text = text_of("content", "output_text", "text");
            native.refusal = text_of("content", "refusal", "refusal");
        }
        "reasoning" => native.summary = text_of("summary", "summary_text", "text"),
        "function_call" => {
            native.role = StreamItemRole::Tool;
            native.call_id = string("call_id");
            native.arguments = string("arguments");
        }
        _ => {
            native.role = StreamItemRole::Hosted;
            native.hosted_report = Some(if kind.is_empty() { "unknown" } else { kind }.to_owned());
        }
    }
    native
}

/// An item without its own status has the state of its response.
fn item_state(item: &Value, response_state: StreamItemState) -> StreamItemState {
    match item.get("status").and_then(Value::as_str) {
        Some("completed") => StreamItemState::Complete,
        Some("incomplete") => StreamItemState::Incomplete,
        Some("in_progress") => StreamItemState::Provisional,
        Some("failed") => StreamItemState::Failed,
        Some("cancelled") => StreamItemState::Cancelled,
        Some(_) => StreamItemState::Unknown,
        None => response_state,
    }
}

/// The text of the one complete assistant message, when the response has no refusal, no
/// function call, and no other incomplete item.
fn final_text(items: &[NativeStreamItem]) -> Option<String> {
    if items.iter().any(|item| {
        item.state != StreamItemState::Complete
            || item.refusal.is_some()
            || item.role != StreamItemRole::Assistant
    }) {
        return None;
    }
    let mut messages = items.iter().filter_map(|item| item.text.clone());
    let text = messages.next()?;
    messages.next().is_none().then_some(text)
}

/// Reported only when the destination reports both counts. Absent usage is unknown, never zero.
fn usage(response: &Value) -> ModelUsage {
    let count = |field: &str| response.get("usage")?.get(field)?.as_u64();
    match (count("input_tokens"), count("output_tokens")) {
        (Some(input_tokens), Some(output_tokens)) => ModelUsage::Reported {
            input_tokens,
            output_tokens,
        },
        _ => ModelUsage::Unknown,
    }
}
