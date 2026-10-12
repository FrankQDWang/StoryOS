//! Read the typed server-sent events of one streamed Responses request (ADR 0033, ADR 0039).

use std::collections::BTreeMap;

use serde_json::Value;
use storyos_application::{
    DeclaredTarget, ModelStreamSink, Observation, ReferenceRetrieval, ReportedBinding,
    ResponseReference, StreamControl,
};
use storyos_core::{NativeStreamItem, StreamItemState};

/// The ordered items of one stream, and the provisional events that wait for their commit.
struct StreamState {
    reference: Option<String>,
    /// The current state of each item, by its output index.
    items: BTreeMap<usize, NativeStreamItem>,
    pending: Vec<NativeStreamItem>,
}

/// Commits each terminal item event at once, and provisional events in batches of `bound`. A
/// stream that stops before its terminal response event is OutcomeUnknown.
pub(crate) async fn observe(
    mut response: reqwest::Response,
    targets: &[DeclaredTarget],
    mapping_revision: &str,
    bound: usize,
    sink: &mut impl ModelStreamSink,
) -> Observation {
    let mut state = StreamState {
        reference: None,
        items: BTreeMap::new(),
        pending: Vec::new(),
    };
    let mut buffer = Vec::new();
    loop {
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) | Err(_) => break,
        };
        buffer.extend_from_slice(&chunk);
        while let Some((event, rest)) = split_event(&buffer) {
            let data = event_data(&event);
            buffer = rest;
            let Some(event) = data.and_then(|data| serde_json::from_str::<Value>(&data).ok())
            else {
                continue;
            };
            match state.apply(&event, targets) {
                Step::Continue => {}
                Step::Terminal(observation) => {
                    // The terminal observation carries each item, so a stop changes nothing.
                    state.flush(sink).await;
                    return observation;
                }
                Step::Commit => {
                    if state.flush(sink).await == StreamControl::Stop {
                        return state.unknown(mapping_revision);
                    }
                }
            }
            if state.pending.len() >= bound && state.flush(sink).await == StreamControl::Stop {
                return state.unknown(mapping_revision);
            }
        }
    }
    state.flush(sink).await;
    state.unknown(mapping_revision)
}

enum Step {
    Continue,
    Commit,
    Terminal(Observation),
}

impl StreamState {
    fn apply(&mut self, event: &Value, targets: &[DeclaredTarget]) -> Step {
        let text = |field: &str| event.get(field).and_then(Value::as_str).unwrap_or_default();
        let index = event
            .get("output_index")
            .and_then(Value::as_u64)
            .and_then(|index| usize::try_from(index).ok());
        match text("type") {
            "response.created" | "response.in_progress" => {
                if let Some(id) = event.pointer("/response/id").and_then(Value::as_str) {
                    self.reference = Some(id.to_owned());
                }
                Step::Continue
            }
            "response.completed" | "response.incomplete" | "response.failed" => {
                match event.get("response") {
                    Some(response) => {
                        Step::Terminal(crate::response::observe_value(response, targets))
                    }
                    None => Step::Continue,
                }
            }
            "response.output_item.added" | "response.output_item.done" => {
                let (Some(index), Some(item)) = (index, event.get("item")) else {
                    return Step::Continue;
                };
                let done = text("type") == "response.output_item.done";
                let state = if done {
                    StreamItemState::Complete
                } else {
                    StreamItemState::Provisional
                };
                self.put(index, crate::response::native_item(index, item, state));
                if done { Step::Commit } else { Step::Continue }
            }
            "response.output_text.delta" => {
                self.extend(index, text("delta"), |item| &mut item.text)
            }
            "response.refusal.delta" => self.extend(index, text("delta"), |item| &mut item.refusal),
            "response.reasoning_summary_text.delta" => {
                self.extend(index, text("delta"), |item| &mut item.summary)
            }
            "response.function_call_arguments.delta" => {
                self.extend(index, text("delta"), |item| &mut item.arguments)
            }
            _ => Step::Continue,
        }
    }

    fn put(&mut self, index: usize, item: NativeStreamItem) {
        self.items.insert(index, item.clone());
        self.pending.push(item);
    }

    /// Appends one delta to its item and records the provisional item as one event.
    fn extend(
        &mut self,
        index: Option<usize>,
        delta: &str,
        field: impl Fn(&mut NativeStreamItem) -> &mut Option<String>,
    ) -> Step {
        let Some(item) = index.and_then(|index| self.items.get_mut(&index)) else {
            return Step::Continue;
        };
        field(item).get_or_insert_with(String::new).push_str(delta);
        item.state = StreamItemState::Provisional;
        let event = item.clone();
        self.pending.push(event);
        Step::Continue
    }

    async fn flush(&mut self, sink: &mut impl ModelStreamSink) -> StreamControl {
        if self.pending.is_empty() {
            return StreamControl::Continue;
        }
        let events = std::mem::take(&mut self.pending);
        sink.append(&events).await
    }

    /// A dropped, stopped, or timed-out stream. The destination can still complete the response.
    fn unknown(&self, mapping_revision: &str) -> Observation {
        Observation::OutcomeUnknown {
            response_reference: self.reference.as_ref().map(|reference| ResponseReference {
                reference_id: reference.clone(),
                retrieval: ReferenceRetrieval::Unsupported,
                reported_binding: ReportedBinding {
                    owner_user_id: None,
                    conversation_id: None,
                    destination_identity: None,
                    mapping_revision: mapping_revision.to_owned(),
                },
            }),
        }
    }
}

/// The first complete event of `buffer` and the bytes after it. Events end with a blank line.
fn split_event(buffer: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let end = buffer
        .windows(/*size*/ 2)
        .position(|window| window == b"\n\n")
        .map(|position| (position, 2))
        .into_iter()
        .chain(
            buffer
                .windows(/*size*/ 4)
                .position(|window| window == b"\r\n\r\n")
                .map(|position| (position, 4)),
        )
        .min_by_key(|(position, _)| *position)?;
    Some((buffer[..end.0].to_vec(), buffer[end.0 + end.1..].to_vec()))
}

/// The joined `data` lines of one event, or `None` for an event without data.
fn event_data(event: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(event).ok()?;
    let lines: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(|line| line.strip_prefix(' ').unwrap_or(line))
        .collect();
    (!lines.is_empty() && lines != ["[DONE]"]).then(|| lines.join("\n"))
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
