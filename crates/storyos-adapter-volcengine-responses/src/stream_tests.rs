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
    references: Vec<ResponseReference>,
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

    async fn retain_reference(&mut self, reference: &ResponseReference) -> StreamControl {
        self.references.push(reference.clone());
        StreamControl::Continue
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
        references: Vec::new(),
        stop_after,
    };
    let observation = adapter.exchange(prepared.prepared, &mut sink).await;
    // Each stream reports its reference once, before its first batch.
    let reported = match &observation {
        Observation::Terminal(response) => response.response_reference.clone(),
        Observation::OutcomeUnknown { response_reference } => response_reference
            .as_ref()
            .map(|reference| reference.reference_id.clone()),
        Observation::NotSubmitted | Observation::Rejected { .. } => None,
    };
    assert_eq!(
        sink.references
            .iter()
            .map(|reference| reference.reference_id.clone())
            .collect::<Vec<_>>(),
        Vec::from_iter(reported)
    );
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
        references: Vec::new(),
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

/// The data of each complete event when the stream arrives in `chunks`.
///
/// After each search, the next search starts at the next event, or at most 3 bytes before the end
/// of the received bytes. A blank line has at most 4 bytes, so each byte is in at most 4 searches.
fn event_data_of(chunks: &[&[u8]]) -> Vec<Option<String>> {
    let mut buffer = super::EventBuffer::default();
    let mut data = Vec::new();
    for chunk in chunks {
        buffer.push(chunk);
        while let Some(event) = buffer
            .next_event()
            .map(|event| super::event_data(event).map(std::borrow::Cow::into_owned))
        {
            assert_eq!(buffer.resume, buffer.start);
            data.push(event);
        }
        assert!(buffer.resume >= buffer.start);
        assert!(buffer.resume + 3 >= buffer.bytes.len());
    }
    data
}

#[test]
fn each_chunking_of_a_stream_gives_the_same_ordered_event_data() {
    let stream: &[u8] = b"event: a\ndata: {\"n\":1}\n\n\
        data: {\"n\":2}\r\n\r\n\
        data: first\ndata:second\n\n\
        event: ping\n\n\
        data: [DONE]\n\n\
        data: mixed\r\n\n\
        data: crlf\n\r\n\r\n\
        data: {\"n\":9}";
    let expected = vec![
        Some(r#"{"n":1}"#.to_owned()),
        Some(r#"{"n":2}"#.to_owned()),
        Some("first\nsecond".to_owned()),
        None,
        None,
        Some("mixed\r".to_owned()),
        Some("crlf".to_owned()),
    ];

    let bytes: Vec<&[u8]> = stream.chunks(/*chunk_size*/ 1).collect();
    assert_eq!(event_data_of(&bytes), expected);
    for split in 0..=stream.len() {
        let (head, tail) = stream.split_at(split);
        assert_eq!(event_data_of(&[head, tail]), expected, "split at {split}");
    }
}

#[test]
fn each_split_of_a_boundary_across_three_chunks_gives_the_same_events() {
    let stream: &[u8] = b"data: a\n\ndata: b\r\n\r\ndata: c\r\n\ndata: d\n\r\n\r\n";
    let expected = vec![
        Some("a".to_owned()),
        Some("b".to_owned()),
        Some("c\r".to_owned()),
        Some("d".to_owned()),
    ];

    for first in 0..=stream.len() {
        for second in first..=stream.len() {
            let chunks = [&stream[..first], &stream[first..second], &stream[second..]];
            assert_eq!(
                event_data_of(&chunks),
                expected,
                "split at {first} and {second}"
            );
        }
    }
}

#[test]
fn a_long_event_in_small_chunks_gives_its_data() {
    let text = "x".repeat(/*n*/ 4096);
    let stream = format!("data: {text}\r\n\r\n");

    for chunk_size in [1, 16] {
        let chunks: Vec<&[u8]> = stream.as_bytes().chunks(chunk_size).collect();
        assert_eq!(event_data_of(&chunks), vec![Some(text.clone())]);
    }
}
