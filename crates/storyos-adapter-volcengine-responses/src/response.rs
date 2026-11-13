//! Map one complete Responses object to the typed Model Gateway observation.

use serde_json::Value;
use storyos_application::{CacheUse, ModelResponse, ModelUsage, Observation};
use storyos_core::{NativeStreamItem, StreamItemRole, StreamItemState};

/// The observation of a successful HTTP response body. A body that is not a Responses object
/// is an unknown outcome, because the destination can have processed the request.
pub(crate) fn observe(body: &[u8]) -> Observation {
    let Ok(response) = serde_json::from_slice::<Value>(body) else {
        return Observation::OutcomeUnknown {
            response_reference: None,
        };
    };
    observe_value(&response)
}

/// The observation of one Responses object, from a body or from a terminal stream event.
pub(crate) fn observe_value(response: &Value) -> Observation {
    let Some(status) = response_state(response) else {
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
    Observation::Terminal(ModelResponse {
        state: status,
        items,
        usage: usage(response),
        cache_use: cache_use(response),
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

pub(crate) fn native_item(
    index: usize,
    item: &Value,
    response_state: StreamItemState,
) -> NativeStreamItem {
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
        native_type: None,
        name: None,
    };
    match kind {
        "message" => {
            native.text = text_of("content", "output_text", "text");
            native.refusal = text_of("content", "refusal", "refusal");
        }
        "reasoning" => native.summary = text_of("summary", "summary_text", "text"),
        // A request asks for no other native item (ADR 0084). Such an item is a Tool item, so
        // the response has no final text and no Agent Decision. Its native type stays as evidence.
        _ => {
            native.role = StreamItemRole::Tool;
            native.call_id = string("call_id");
            native.arguments = string("arguments");
            native.native_type = Some(if kind.is_empty() { "unknown" } else { kind }.to_owned());
            native.name = string("name");
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

/// The cached input tokens that the destination reports. Absent cache use is unknown, never zero.
fn cache_use(response: &Value) -> CacheUse {
    match response
        .pointer("/usage/input_tokens_details/cached_tokens")
        .and_then(Value::as_u64)
    {
        Some(cached_input_tokens) => CacheUse::Reported {
            cached_input_tokens,
        },
        None => CacheUse::Unknown,
    }
}
