use serde_json::json;
use storyos_application::{
    CacheUse, ModelProviderAdapter, ModelResponse, ModelStreamSink, ModelUsage, Observation,
    PROVISIONAL_STREAM_EVENT_BOUND, ReferenceRetrieval, ReportedBinding, RequestAttempt,
    RequestRoute, ResponseReference, StreamControl,
};
use storyos_core::{DecisionCandidate, NativeStreamItem, StreamItemRole, StreamItemState};

use crate::AgentPlanResponses;
use crate::tests::{
    CanaryResolver, Reply, create, destination, final_answer, item, message, route,
};

const ADVISORY: &str = r#"{"kind":"advisory","text":"Keep it."}"#;

/// Records each committed batch. It tells the adapter to stop after `stop_after` batches.
struct BatchSink {
    batches: Vec<Vec<NativeStreamItem>>,
    stop_after: Option<usize>,
}

impl ModelStreamSink for BatchSink {
    async fn append(&mut self, events: &[NativeStreamItem]) -> StreamControl {
        self.batches.push(events.to_vec());
        match self.stop_after {
            Some(limit) if self.batches.len() >= limit => StreamControl::Stop,
            Some(_) | None => StreamControl::Continue,
        }
    }
}

fn streaming_route(base: &str) -> RequestRoute {
    RequestRoute {
        mapping_revision: storyos_core::AGENT_PLAN_STREAMING_MAPPING_REVISION.to_owned(),
        bounds: Some(storyos_application::RequestBounds {
            max_output_tokens: 8192,
            timeout: std::time::Duration::from_secs(/*secs*/ 60),
        }),
        ..route(base)
    }
}

fn started(reference: &str) -> Vec<serde_json::Value> {
    vec![
        json!({"type": "response.created", "sequence_number": 0,
               "response": {"id": reference, "status": "in_progress", "output": []}}),
        json!({"type": "response.output_item.added", "sequence_number": 1, "output_index": 0,
               "item": {"id": "msg_01", "type": "message", "role": "assistant",
                        "status": "in_progress", "content": []}}),
        json!({"type": "response.output_text.delta", "sequence_number": 2, "output_index": 0,
               "item_id": "msg_01", "content_index": 0, "delta": &ADVISORY[..10]}),
        json!({"type": "response.output_text.delta", "sequence_number": 3, "output_index": 0,
               "item_id": "msg_01", "content_index": 0, "delta": &ADVISORY[10..]}),
    ]
}

/// Sends one streamed request to a destination that replies with `events`.
async fn stream(
    events: Vec<serde_json::Value>,
    stop_after: Option<usize>,
) -> (Observation, Vec<Vec<NativeStreamItem>>, String) {
    let (observation, batches, served) = exchange(Reply::Events(events), stop_after).await;
    let (_, _, body) = served.await.unwrap();
    (observation, batches, body)
}

async fn exchange(
    reply: Reply,
    stop_after: Option<usize>,
) -> (
    Observation,
    Vec<Vec<NativeStreamItem>>,
    tokio::task::JoinHandle<crate::tests::Received>,
) {
    let (base, served) = destination(reply).await;
    let adapter = AgentPlanResponses {
        resolver: CanaryResolver::default(),
    };
    let prepared = adapter
        .prepare(&create(streaming_route(&base), RequestAttempt::New))
        .await
        .ok()
        .unwrap();
    let mut sink = BatchSink {
        batches: Vec::new(),
        stop_after,
    };
    let observation = adapter.exchange(prepared.prepared, &mut sink).await;
    (observation, sink.batches, served)
}

fn provisional(text: Option<&str>) -> NativeStreamItem {
    NativeStreamItem {
        text: text.map(str::to_owned),
        ..item(
            "msg_01",
            StreamItemRole::Assistant,
            StreamItemState::Provisional,
        )
    }
}

fn unknown(reference: &str) -> Observation {
    Observation::OutcomeUnknown {
        response_reference: Some(ResponseReference {
            reference_id: reference.to_owned(),
            retrieval: ReferenceRetrieval::Supported {
                bounds: storyos_core::RetrievalBounds::DeclaredReadOnly,
            },
            reported_binding: ReportedBinding {
                owner_user_id: None,
                conversation_id: None,
                destination_identity: None,
                mapping_revision: storyos_core::AGENT_PLAN_STREAMING_MAPPING_REVISION.to_owned(),
            },
        }),
    }
}

#[tokio::test]
async fn a_streamed_response_commits_ordered_events_and_maps_its_terminal_response() {
    let complete = message(ADVISORY);
    let mut events = started("resp_s1");
    events.push(
        json!({"type": "response.output_item.done", "sequence_number": 4,
                       "output_index": 0, "item": complete}),
    );
    events.push(
        json!({"type": "response.completed", "sequence_number": 5, "response": {
        "id": "resp_s1", "status": "completed", "output": [complete],
        "usage": {"input_tokens": 50, "output_tokens": 9,
                  "input_tokens_details": {"cached_tokens": 0}}}}),
    );
    let (observation, batches, body) = stream(events, /*stop_after*/ None).await;
    let done = NativeStreamItem {
        text: Some(ADVISORY.to_owned()),
        ..item(
            "msg_01",
            StreamItemRole::Assistant,
            StreamItemState::Complete,
        )
    };
    let sent: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert_eq!(
        (
            observation,
            batches.concat(),
            batches
                .iter()
                .all(|batch| batch.len() <= PROVISIONAL_STREAM_EVENT_BOUND
                    || batch.last() == Some(&done)),
            sent["stream"].clone(),
        ),
        (
            Observation::Terminal(ModelResponse {
                items: vec![done.clone()],
                output: Some(final_answer(
                    DecisionCandidate::Advisory {
                        text: "Keep it.".to_owned()
                    },
                    None
                )),
                usage: ModelUsage::Reported {
                    input_tokens: 50,
                    output_tokens: 9,
                },
                cache_use: CacheUse::Reported {
                    cached_input_tokens: 0,
                },
                response_reference: Some("resp_s1".to_owned()),
            }),
            vec![
                provisional(None),
                provisional(Some(&ADVISORY[..10])),
                provisional(Some(ADVISORY)),
                done,
            ],
            true,
            json!(true),
        )
    );
}

#[tokio::test]
async fn a_stream_that_ends_before_its_terminal_event_is_unknown_with_its_reference() {
    let (observation, batches, _) = stream(started("resp_s2"), /*stop_after*/ None).await;

    assert_eq!(
        (observation, batches.concat().last().cloned()),
        (unknown("resp_s2"), Some(provisional(Some(ADVISORY)))),
    );
}

#[tokio::test]
async fn a_stop_from_the_sink_ends_the_exchange_before_the_next_commit() {
    let (observation, batches, _) = stream(started("resp_s3"), Some(1)).await;

    assert_eq!((observation, batches.len()), (unknown("resp_s3"), 1));
}

#[tokio::test]
async fn a_stop_at_the_terminal_response_event_is_unknown_with_its_reference() {
    let mut events = started("resp_s5");
    events.push(
        json!({"type": "response.completed", "sequence_number": 4, "response": {
        "id": "resp_s5", "status": "completed", "output": [message(ADVISORY)]}}),
    );
    let (observation, batches, _) = stream(events, Some(1)).await;

    assert_eq!((observation, batches.len()), (unknown("resp_s5"), 1));
}

#[tokio::test]
async fn a_stalled_stream_commits_its_pending_events_and_learns_a_stop_after_the_interval() {
    let (base, _served) = destination(Reply::Stall(started("resp_s4"))).await;
    let response = reqwest::Client::new()
        .post(format!("{base}/responses"))
        .body("{}")
        .send()
        .await
        .unwrap();
    let mut sink = BatchSink {
        batches: Vec::new(),
        stop_after: Some(2),
    };
    // The destination sends nothing more, so only the interval can start each commit.
    let policy = super::CommitPolicy {
        bound: PROVISIONAL_STREAM_EVENT_BOUND,
        interval: std::time::Duration::from_millis(/*millis*/ 20),
    };
    let observation = super::observe(
        response,
        &[],
        storyos_core::AGENT_PLAN_STREAMING_MAPPING_REVISION,
        policy,
        &mut sink,
    )
    .await;

    assert_eq!(
        (observation, sink.batches),
        (
            unknown("resp_s4"),
            vec![
                vec![
                    provisional(None),
                    provisional(Some(&ADVISORY[..10])),
                    provisional(Some(ADVISORY)),
                ],
                Vec::new(),
            ],
        )
    );
}
