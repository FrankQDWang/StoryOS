use std::sync::Mutex;
use std::time::Duration;

use storyos_application::{
    CreateRequest, CredentialReference, CredentialResolver, DeclaredTarget, DestinationRequest,
    DispatchClaim, ModelProviderAdapter, ModelResponse, ModelStreamSink, ModelUsage, Observation,
    PreDispatchRefusal, PreparedRequest, RequestAttempt, RequestBounds, RequestContextItem,
    RequestRoute, ResolvedCredential, RetrievePurpose, RetrieveRequest, StreamControl,
};
use storyos_core::{
    ContextSourceClass, DecisionCandidate, ModelAdapter, ModelOutput, NativeStreamItem,
    OutputPhase, ProseChangeCandidate, StreamItemRole, StreamItemState,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::{AgentPlanExchange, AgentPlanResponses};

const CREDENTIAL_CANARY: &str = "storyos-credential-canary-5e21";
const REFERENCE: &str = "macos-keychain:storyos-volcengine-agent-plan/frankqdwang";

/// Returns a test value that is not a real credential, and counts each resolution.
#[derive(Default)]
struct CanaryResolver {
    resolved: Mutex<usize>,
}

impl CredentialResolver for CanaryResolver {
    async fn resolve(&self, _reference: &CredentialReference) -> Option<ResolvedCredential> {
        *self.resolved.lock().unwrap() += 1;
        Some(ResolvedCredential::new(CREDENTIAL_CANARY.to_owned()))
    }
}

#[derive(Default)]
struct RecordingSink(Vec<NativeStreamItem>);

impl ModelStreamSink for RecordingSink {
    async fn append(&mut self, events: &[NativeStreamItem]) -> StreamControl {
        self.0.extend_from_slice(events);
        StreamControl::Continue
    }
}

/// What the scripted destination does after it reads one complete request.
enum Reply {
    Http(u16, &'static str),
    Drop,
    Hold,
}

/// The request that the scripted destination received.
#[derive(Debug, PartialEq)]
struct Received {
    request_line: String,
    authorization: Option<String>,
    body: String,
}

/// Serves one connection with `reply`, and returns its base URL and the received request.
async fn destination(reply: Reply) -> (String, tokio::task::JoinHandle<Received>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/api/plan/v3", listener.local_addr().unwrap());
    let served = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        let received = loop {
            let mut chunk = [0_u8; 4096];
            let read = stream.read(&mut chunk).await.unwrap();
            bytes.extend_from_slice(&chunk[..read]);
            if let Some(received) = complete_request(&bytes) {
                break received;
            }
            assert_ne!(read, 0, "the request ended before its body");
        };
        match reply {
            Reply::Http(status, body) => {
                let response = format!(
                    "HTTP/1.1 {status} Scripted\r\ncontent-type: application/json\r\n\
                     content-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
            Reply::Drop => drop(stream),
            Reply::Hold => std::future::pending::<()>().await,
        }
        received
    });
    (base, served)
}

fn complete_request(bytes: &[u8]) -> Option<Received> {
    let text = std::str::from_utf8(bytes).ok()?;
    let (head, body) = text.split_once("\r\n\r\n")?;
    let header = |name: &str| {
        head.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name)
                .then(|| value.trim().to_owned())
        })
    };
    let length: usize = header("content-length")?.parse().ok()?;
    (body.len() >= length).then(|| Received {
        request_line: head.lines().next().unwrap_or_default().to_owned(),
        authorization: header("authorization"),
        body: body[..length].to_owned(),
    })
}

fn target() -> DeclaredTarget {
    DeclaredTarget {
        chapter_id: "018f0000-0000-7001-8000-0000000000c1".to_owned(),
        block_id: "018f0000-0000-7001-8000-0000000000b1".to_owned(),
        base_revision_id: "018f0000-0000-7001-8000-0000000000a1".to_owned(),
        collection: false,
        block_text: "The rain fell hard on the old roof.".to_owned(),
    }
}

fn route(endpoint: &str, timeout: Duration) -> RequestRoute {
    RequestRoute {
        adapter: ModelAdapter::VolcengineAgentPlanResponses,
        provider_model_id: "doubao-seed-2.1-pro".to_owned(),
        endpoint: Some(endpoint.to_owned()),
        credential_reference: Some(CredentialReference(REFERENCE.to_owned())),
        bounds: Some(RequestBounds {
            max_output_tokens: 8192,
            timeout,
        }),
    }
}

fn create(route: RequestRoute, attempt: RequestAttempt) -> DestinationRequest {
    DestinationRequest::Create(CreateRequest {
        attempt,
        route,
        author_message: "Tighten this paragraph.".to_owned(),
        chapter_id: target().chapter_id,
        passage_resolution: None,
        passage_input: None,
        context: vec![
            RequestContextItem {
                source_class: ContextSourceClass::AuthorInstruction,
                content: "Tighten this paragraph.".to_owned(),
            },
            RequestContextItem {
                source_class: ContextSourceClass::WorkingTarget,
                content: "The rain fell hard on the old roof.".to_owned(),
            },
        ],
        declared_targets: vec![target()],
        candidate_revision: None,
        previous_response_reference: None,
        successor_of: None,
    })
}

async fn prepare(
    adapter: &AgentPlanResponses<CanaryResolver>,
    request: &DestinationRequest,
) -> PreparedRequest<AgentPlanExchange> {
    adapter.prepare(request).await.ok().unwrap()
}

/// Sends one Create request to a scripted destination, with a one-second bound.
async fn exchange(reply: Reply) -> (Observation, Vec<NativeStreamItem>) {
    let (base, _served) = destination(reply).await;
    exchange_at(&base, Duration::from_secs(1)).await
}

async fn exchange_at(base: &str, timeout: Duration) -> (Observation, Vec<NativeStreamItem>) {
    let adapter = AgentPlanResponses {
        resolver: CanaryResolver::default(),
    };
    let request = create(route(base, timeout), RequestAttempt::New);
    let prepared = prepare(&adapter, &request).await;
    let mut sink = RecordingSink::default();
    let observation = adapter.exchange(prepared.prepared, &mut sink).await;
    (observation, sink.0)
}

const COMPLETED_PROSE_CHANGE: &str = r#"{
  "id": "resp_0217000000000000000000000000000000000000000000001",
  "object": "response",
  "status": "completed",
  "output": [
    {"id": "rs_01", "type": "reasoning", "status": "completed",
     "summary": [{"type": "summary_text", "text": "The author wants a tighter line."}]},
    {"id": "msg_01", "type": "message", "role": "assistant", "status": "completed",
     "content": [{"type": "output_text", "text": "{\"kind\":\"prose_change\",\"summary\":\"I made the line shorter.\",\"changes\":[{\"block_id\":\"018f0000-0000-7001-8000-0000000000b1\",\"candidate_text\":\"Rain hammered the old roof.\",\"explanation\":\"A stronger verb replaces two words.\"}]}"}]}
  ],
  "usage": {"input_tokens": 412, "input_tokens_details": {"cached_tokens": 0},
            "output_tokens": 133, "output_tokens_details": {"reasoning_tokens": 61},
            "total_tokens": 545},
  "store": true
}"#;

#[tokio::test]
async fn a_completed_response_maps_to_its_native_items_and_a_validated_candidate() {
    let (base, served) = destination(Reply::Http(200, COMPLETED_PROSE_CHANGE)).await;
    let adapter = AgentPlanResponses {
        resolver: CanaryResolver::default(),
    };
    let request = create(route(&base, Duration::from_secs(1)), RequestAttempt::New);
    let prepared = prepare(&adapter, &request).await;
    let projection = prepared.projection.clone();
    let mut sink = RecordingSink::default();
    let observation = adapter.exchange(prepared.prepared, &mut sink).await;
    let received = served.await.unwrap();

    let items = vec![
        NativeStreamItem {
            item_id: "rs_01".to_owned(),
            role: StreamItemRole::Assistant,
            state: StreamItemState::Complete,
            text: None,
            summary: Some("The author wants a tighter line.".to_owned()),
            call_id: None,
            arguments: None,
            refusal: None,
            hosted_report: None,
        },
        NativeStreamItem {
            item_id: "msg_01".to_owned(),
            role: StreamItemRole::Assistant,
            state: StreamItemState::Complete,
            text: Some(
                r#"{"kind":"prose_change","summary":"I made the line shorter.","changes":[{"block_id":"018f0000-0000-7001-8000-0000000000b1","candidate_text":"Rain hammered the old roof.","explanation":"A stronger verb replaces two words."}]}"#
                    .to_owned(),
            ),
            summary: None,
            call_id: None,
            arguments: None,
            refusal: None,
            hosted_report: None,
        },
    ];
    assert_eq!(
        (observation, sink.0),
        (
            Observation::Terminal(ModelResponse {
                items: items.clone(),
                output: Some(ModelOutput {
                    phase: OutputPhase::FinalAnswer,
                    candidate: DecisionCandidate::ProseChange {
                        text: "I made the line shorter.".to_owned(),
                    },
                    prose_changes: Some(vec![ProseChangeCandidate {
                        chapter_id: target().chapter_id,
                        manuscript_block_id: target().block_id,
                        base_authoritative_revision_id: target().base_revision_id,
                        candidate_text: "Rain hammered the old roof.".to_owned(),
                        explanation: "A stronger verb replaces two words.".to_owned(),
                    }]),
                }),
                usage: ModelUsage::Reported {
                    input_tokens: 412,
                    output_tokens: 133,
                },
                response_reference: Some(
                    "resp_0217000000000000000000000000000000000000000000001".to_owned()
                ),
            }),
            items,
        )
    );
    let mut body: serde_json::Value = serde_json::from_str(&received.body).unwrap();
    let instructions = body
        .as_object_mut()
        .unwrap()
        .remove("instructions")
        .unwrap();
    assert_eq!(
        (
            received.request_line,
            received.authorization,
            Some(received.body.clone()),
            projection.digest,
            body,
        ),
        (
            "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
            Some(format!("Bearer {CREDENTIAL_CANARY}")),
            projection.serialized_payload,
            format!(
                "sha256:{}",
                storyos_core::hex_sha256(received.body.as_bytes())
            ),
            serde_json::json!({
                "model": "doubao-seed-2.1-pro",
                "input": [{"role": "user", "content": [{"type": "input_text", "text":
                    "Author request:\nTighten this paragraph.\n\n\
                     Working text:\nThe rain fell hard on the old roof.\n\n\
                     Target blocks:\n[block_id: 018f0000-0000-7001-8000-0000000000b1]\n\
                     The rain fell hard on the old roof."}]}],
                "max_output_tokens": 8192,
                "stream": false,
                "store": true,
            }),
        )
    );
    assert!(
        instructions
            .as_str()
            .is_some_and(|text| text.contains("prose_change"))
    );
    assert!(!received.body.contains(CREDENTIAL_CANARY));
}

fn completed_text(text: &str) -> String {
    serde_json::json!({
        "id": "resp_text",
        "status": "completed",
        "output": [{"id": "msg_01", "type": "message", "role": "assistant", "status": "completed",
                    "content": [{"type": "output_text", "text": text}]}],
    })
    .to_string()
}

fn output_of(observation: Observation) -> Option<ModelOutput> {
    match observation {
        Observation::Terminal(response) => response.output,
        other => panic!("not a terminal response: {other:?}"),
    }
}

fn final_answer(
    candidate: DecisionCandidate,
    prose_changes: Option<Vec<ProseChangeCandidate>>,
) -> Option<ModelOutput> {
    Some(ModelOutput {
        phase: OutputPhase::FinalAnswer,
        candidate,
        prose_changes,
    })
}

#[test]
fn each_complete_text_maps_to_its_contract_candidate_or_to_an_advisory() {
    let observe = |text: &str| {
        output_of(super::response::observe(
            completed_text(text).as_bytes(),
            &[target()],
        ))
    };
    let undeclared = r#"{"kind":"prose_change","summary":"Changed.","changes":[{"block_id":"other","candidate_text":"New.","explanation":"Why."}]}"#;

    assert_eq!(
        [
            observe(r#"{"kind":"advisory","text":"Keep the second sentence."}"#),
            observe("```json\n{\"kind\":\"clarification\",\"question\":\"Which scene?\"}\n```"),
            observe("Keep the second sentence."),
            observe(r#"{"kind":"prose_change","summary":"Changed."}"#),
            observe(undeclared),
        ],
        [
            final_answer(
                DecisionCandidate::Advisory {
                    text: "Keep the second sentence.".to_owned()
                },
                None
            ),
            final_answer(
                DecisionCandidate::Clarification {
                    question: "Which scene?".to_owned()
                },
                None
            ),
            final_answer(
                DecisionCandidate::Advisory {
                    text: "Keep the second sentence.".to_owned()
                },
                None
            ),
            final_answer(
                DecisionCandidate::Advisory {
                    text: r#"{"kind":"prose_change","summary":"Changed."}"#.to_owned()
                },
                None
            ),
            final_answer(
                DecisionCandidate::ProseChange {
                    text: "Changed.".to_owned()
                },
                Some(vec![ProseChangeCandidate {
                    chapter_id: String::new(),
                    manuscript_block_id: "other".to_owned(),
                    base_authoritative_revision_id: String::new(),
                    candidate_text: "New.".to_owned(),
                    explanation: "Why.".to_owned(),
                }])
            ),
        ]
    );
    assert_eq!(
        output_of(super::response::observe(
            completed_text(r#"{"kind":"prose_change","summary":"Changed.","changes":[]}"#)
                .as_bytes(),
            &[]
        )),
        final_answer(
            DecisionCandidate::Advisory {
                text: r#"{"kind":"prose_change","summary":"Changed.","changes":[]}"#.to_owned()
            },
            None
        )
    );
}

#[test]
fn a_refusal_a_function_call_or_an_incomplete_response_supplies_no_candidate() {
    let observe = |body: serde_json::Value| {
        super::response::observe(body.to_string().as_bytes(), &[target()])
    };
    let refusal = observe(serde_json::json!({
        "id": "resp_refusal", "status": "completed",
        "output": [{"id": "msg_01", "type": "message", "role": "assistant", "status": "completed",
                    "content": [{"type": "refusal", "refusal": "I cannot help with that."}]}],
    }));
    let function_call = observe(serde_json::json!({
        "id": "resp_call", "status": "completed",
        "output": [{"id": "fc_01", "type": "function_call", "status": "completed",
                    "call_id": "call_01", "name": "search", "arguments": "{\"q\":\"x\"}"}],
    }));
    let incomplete = observe(serde_json::json!({
        "id": "resp_incomplete", "status": "incomplete",
        "incomplete_details": {"reason": "max_output_tokens"},
        "output": [{"id": "msg_01", "type": "message", "role": "assistant",
                    "content": [{"type": "output_text", "text": "{\"kind\":\"adv"}]}],
        "usage": {"input_tokens": 10},
    }));
    let item = |item_id: &str, role, state| NativeStreamItem {
        item_id: item_id.to_owned(),
        role,
        state,
        text: None,
        summary: None,
        call_id: None,
        arguments: None,
        refusal: None,
        hosted_report: None,
    };

    assert_eq!(
        [refusal, function_call, incomplete],
        [
            Observation::Terminal(ModelResponse {
                items: vec![NativeStreamItem {
                    refusal: Some("I cannot help with that.".to_owned()),
                    ..item(
                        "msg_01",
                        StreamItemRole::Assistant,
                        StreamItemState::Complete
                    )
                }],
                output: None,
                usage: ModelUsage::Unknown,
                response_reference: Some("resp_refusal".to_owned()),
            }),
            Observation::Terminal(ModelResponse {
                items: vec![NativeStreamItem {
                    call_id: Some("call_01".to_owned()),
                    arguments: Some("{\"q\":\"x\"}".to_owned()),
                    ..item("fc_01", StreamItemRole::Tool, StreamItemState::Complete)
                }],
                output: None,
                usage: ModelUsage::Unknown,
                response_reference: Some("resp_call".to_owned()),
            }),
            Observation::Terminal(ModelResponse {
                items: vec![NativeStreamItem {
                    text: Some("{\"kind\":\"adv".to_owned()),
                    ..item(
                        "msg_01",
                        StreamItemRole::Assistant,
                        StreamItemState::Incomplete
                    )
                }],
                output: None,
                usage: ModelUsage::Unknown,
                response_reference: Some("resp_incomplete".to_owned()),
            }),
        ]
    );
}

#[tokio::test]
async fn each_transport_outcome_maps_to_rejected_not_submitted_or_outcome_unknown() {
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        format!("http://{}/api/plan/v3", listener.local_addr().unwrap())
    };
    let unknown = Observation::OutcomeUnknown {
        response_reference: None,
    };
    let rate_limited = r#"{"error":{"code":"RateLimitExceeded.EndpointRPM","message":"echo: Tighten this paragraph.","type":"TooManyRequests"}}"#;

    assert_eq!(
        [
            exchange(Reply::Http(429, rate_limited)).await.0,
            exchange(Reply::Http(
                401,
                r#"{"error":{"code":"Invalid key with spaces"}}"#
            ))
            .await
            .0,
            exchange(Reply::Http(500, "{}")).await.0,
            exchange(Reply::Http(200, "not json")).await.0,
            exchange(Reply::Drop).await.0,
            exchange(Reply::Hold).await.0,
            exchange_at(&closed, Duration::from_secs(1)).await.0,
        ],
        [
            Observation::Rejected {
                reason: "http_429:RateLimitExceeded.EndpointRPM".to_owned()
            },
            Observation::Rejected {
                reason: "http_401".to_owned()
            },
            unknown.clone(),
            unknown.clone(),
            unknown.clone(),
            unknown,
            Observation::NotSubmitted,
        ]
    );
}

#[tokio::test]
async fn a_claimed_attempt_is_observed_again_without_a_second_request() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/api/plan/v3", listener.local_addr().unwrap());
    let adapter = AgentPlanResponses {
        resolver: CanaryResolver::default(),
    };
    let claimed = RequestAttempt::Claimed(DispatchClaim {
        model_attempt_id: "018f0000-0000-7001-8000-0000000000d1".to_owned(),
    });
    let prepared = prepare(
        &adapter,
        &create(route(&base, Duration::from_secs(1)), claimed),
    )
    .await;
    let mut sink = RecordingSink::default();
    let observation = adapter.exchange(prepared.prepared, &mut sink).await;
    let connected = tokio::select! {
        biased;
        accepted = listener.accept() => accepted.is_ok(),
        () = std::future::ready(()) => false,
    };

    assert_eq!(
        (
            observation,
            sink.0,
            connected,
            *adapter.resolver.resolved.lock().unwrap()
        ),
        (
            Observation::OutcomeUnknown {
                response_reference: None
            },
            Vec::new(),
            false,
            0
        )
    );
}

#[tokio::test]
async fn retrieval_and_an_unknown_bound_refuse_before_the_dispatch_claim() {
    let adapter = AgentPlanResponses {
        resolver: CanaryResolver::default(),
    };
    let base = "http://127.0.0.1:9/api/plan/v3";
    let retrieve = DestinationRequest::Retrieve(RetrieveRequest {
        attempt: RequestAttempt::New,
        route: route(base, Duration::from_secs(1)),
        purpose: RetrievePurpose::OriginalResult,
        original_model_attempt_id: "018f0000-0000-7001-8000-0000000000d1".to_owned(),
        response_reference: "resp_text".to_owned(),
    });
    let unbounded = create(
        RequestRoute {
            bounds: None,
            ..route(base, Duration::from_secs(1))
        },
        RequestAttempt::New,
    );
    let mut refusals = Vec::new();
    for request in [retrieve, unbounded] {
        refusals.push(adapter.prepare(&request).await.err());
    }

    assert_eq!(
        (refusals, *adapter.resolver.resolved.lock().unwrap()),
        (vec![Some(PreDispatchRefusal::UnsupportedRequest); 2], 0)
    );
}
