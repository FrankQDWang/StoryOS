use std::sync::Mutex;
use std::time::Duration;

use serde_json::json;
use storyos_application::{
    CreateRequest, CredentialReference, CredentialResolver, DeclaredTarget, DestinationRequest,
    DispatchClaim, ModelProviderAdapter, ModelResponse, ModelStreamSink, ModelUsage, Observation,
    PreDispatchRefusal, RequestAttempt, RequestBounds, RequestContextItem, RequestRoute,
    ResolvedCredential, RetrievePurpose, RetrieveRequest, StreamControl,
};
use storyos_core::{
    ContextSourceClass, DecisionCandidate, ModelAdapter, ModelOutput, NativeStreamItem,
    OutputPhase, ProseChangeCandidate, StreamItemRole, StreamItemState,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::AgentPlanResponses;

const CREDENTIAL_CANARY: &str = "storyos-credential-canary-5e21";
const BLOCK: &str = "018f0000-0000-7001-8000-0000000000b1";

/// Returns a test value that is not a real credential, and counts each resolution.
#[derive(Default)]
struct CanaryResolver(Mutex<usize>);

impl CredentialResolver for CanaryResolver {
    async fn resolve(&self, _reference: &CredentialReference) -> Option<ResolvedCredential> {
        *self.0.lock().unwrap() += 1;
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
    Http(u16, String),
    Drop,
    Hold,
}

/// The request line, the `Authorization` header, and the body that the destination received.
type Received = (String, Option<String>, String);

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
            assert_ne!(read, 0, "the request ended before its body");
            bytes.extend_from_slice(&chunk[..read]);
            if let Some(received) = complete_request(&bytes) {
                break received;
            }
        };
        match reply {
            Reply::Http(status, body) => {
                let head = format!(
                    "HTTP/1.1 {status} Scripted\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all((head + &body).as_bytes()).await.unwrap();
            }
            Reply::Drop => drop(stream),
            Reply::Hold => std::future::pending::<()>().await,
        }
        received
    });
    (base, served)
}

fn complete_request(bytes: &[u8]) -> Option<Received> {
    let (head, body) = std::str::from_utf8(bytes).ok()?.split_once("\r\n\r\n")?;
    let header = |name: &str| {
        head.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name)
                .then(|| value.trim().to_owned())
        })
    };
    let length: usize = header("content-length")?.parse().ok()?;
    (body.len() >= length).then(|| {
        let line = head.lines().next().unwrap_or_default().to_owned();
        (line, header("authorization"), body[..length].to_owned())
    })
}

fn target() -> DeclaredTarget {
    DeclaredTarget {
        chapter_id: "018f0000-0000-7001-8000-0000000000c1".to_owned(),
        block_id: BLOCK.to_owned(),
        base_revision_id: "018f0000-0000-7001-8000-0000000000a1".to_owned(),
        collection: false,
        block_text: "The rain fell hard on the old roof.".to_owned(),
    }
}

fn route(endpoint: &str) -> RequestRoute {
    RequestRoute {
        adapter: ModelAdapter::VolcengineAgentPlanResponses,
        provider_model_id: "doubao-seed-2.1-pro".to_owned(),
        endpoint: Some(endpoint.to_owned()),
        credential_reference: Some(CredentialReference("macos-keychain:s/a".to_owned())),
        bounds: Some(RequestBounds {
            max_output_tokens: 8192,
            timeout: Duration::from_secs(1),
        }),
    }
}

fn create(route: RequestRoute, attempt: RequestAttempt) -> DestinationRequest {
    let item = |source_class, content: &str| RequestContextItem {
        source_class,
        content: content.to_owned(),
    };
    DestinationRequest::Create(CreateRequest {
        attempt,
        route,
        author_message: "Tighten this paragraph.".to_owned(),
        chapter_id: target().chapter_id,
        passage_resolution: None,
        passage_input: None,
        context: vec![
            item(
                ContextSourceClass::AuthorInstruction,
                "Tighten this paragraph.",
            ),
            item(ContextSourceClass::WorkingTarget, "The rain fell hard."),
        ],
        declared_targets: vec![target()],
        candidate_revision: None,
        previous_response_reference: None,
        successor_of: None,
    })
}

/// Prepares and sends one request to `base`, and returns the observation, the committed stream
/// events, and the Wire Payload Projection.
async fn exchange_at(base: &str) -> (Observation, Vec<NativeStreamItem>, Option<String>) {
    let adapter = AgentPlanResponses {
        resolver: CanaryResolver::default(),
    };
    let prepared = adapter
        .prepare(&create(route(base), RequestAttempt::New))
        .await
        .ok()
        .unwrap();
    let mut sink = RecordingSink::default();
    let observation = adapter.exchange(prepared.prepared, &mut sink).await;
    (observation, sink.0, prepared.projection.serialized_payload)
}

async fn exchange(reply: Reply) -> Observation {
    let (base, _served) = destination(reply).await;
    exchange_at(&base).await.0
}

fn item(item_id: &str, role: StreamItemRole, state: StreamItemState) -> NativeStreamItem {
    NativeStreamItem {
        item_id: item_id.to_owned(),
        role,
        state,
        text: None,
        summary: None,
        call_id: None,
        arguments: None,
        refusal: None,
        hosted_report: None,
    }
}

fn message(text: &str) -> serde_json::Value {
    json!({"id": "msg_01", "type": "message", "role": "assistant", "status": "completed",
           "content": [{"type": "output_text", "text": text}]})
}

fn final_answer(
    candidate: DecisionCandidate,
    changes: Option<Vec<ProseChangeCandidate>>,
) -> ModelOutput {
    ModelOutput {
        phase: OutputPhase::FinalAnswer,
        candidate,
        prose_changes: changes,
    }
}

#[tokio::test]
async fn a_completed_response_maps_to_native_items_and_the_sent_body_is_the_projection() {
    let reply = json!({"id": "resp_01", "status": "completed", "output": [
        {"id": "rs_01", "type": "reasoning", "summary": [{"type": "summary_text", "text": "Plan."}]},
        message(r#"{"kind":"advisory","text":"Keep the second sentence."}"#),
    ], "usage": {"input_tokens": 412, "output_tokens": 133, "total_tokens": 545}});
    let (base, served) = destination(Reply::Http(200, reply.to_string())).await;
    let (observation, events, projection) = exchange_at(&base).await;
    let (line, authorization, body) = served.await.unwrap();
    let items = vec![
        NativeStreamItem {
            summary: Some("Plan.".to_owned()),
            ..item(
                "rs_01",
                StreamItemRole::Assistant,
                StreamItemState::Complete,
            )
        },
        NativeStreamItem {
            text: Some(r#"{"kind":"advisory","text":"Keep the second sentence."}"#.to_owned()),
            ..item(
                "msg_01",
                StreamItemRole::Assistant,
                StreamItemState::Complete,
            )
        },
    ];
    let mut sent: serde_json::Value = serde_json::from_str(&body).unwrap();
    let instructions = sent.as_object_mut().unwrap().remove("instructions");

    assert_eq!(
        (observation, events, line, authorization, projection, sent),
        (
            Observation::Terminal(ModelResponse {
                items: items.clone(),
                output: Some(final_answer(
                    DecisionCandidate::Advisory {
                        text: "Keep the second sentence.".to_owned()
                    },
                    None
                )),
                usage: ModelUsage::Reported {
                    input_tokens: 412,
                    output_tokens: 133,
                },
                response_reference: Some("resp_01".to_owned()),
            }),
            items,
            "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
            Some(format!("Bearer {CREDENTIAL_CANARY}")),
            Some(body.clone()),
            json!({
                "model": "doubao-seed-2.1-pro",
                "input": [{"role": "user", "content": [{"type": "input_text", "text": format!(
                    "Author request:\nTighten this paragraph.\n\nWorking text:\nThe rain fell \
                     hard.\n\nTarget blocks:\n[block_id: {BLOCK}]\nThe rain fell hard on the old roof."
                )}]}],
                "max_output_tokens": 8192,
                "stream": false,
                "store": true,
            }),
        )
    );
    assert!(instructions.is_some_and(|text| text.as_str().is_some_and(|text| !text.is_empty())));
    assert!(!body.contains(CREDENTIAL_CANARY));
}

#[test]
fn complete_text_maps_to_its_contract_candidate_and_other_text_to_an_advisory() {
    let advisory = |text: &str| {
        final_answer(
            DecisionCandidate::Advisory {
                text: text.to_owned(),
            },
            None,
        )
    };
    let change = |block: &str, chapter: &str, base: &str| ProseChangeCandidate {
        chapter_id: chapter.to_owned(),
        manuscript_block_id: block.to_owned(),
        base_authoritative_revision_id: base.to_owned(),
        candidate_text: "Rain hammered the roof.".to_owned(),
        explanation: "A stronger verb.".to_owned(),
    };
    let prose = |block: &str| {
        format!(
            r#"{{"kind":"prose_change","summary":"Shorter.","changes":[{{"block_id":"{block}","candidate_text":"Rain hammered the roof.","explanation":"A stronger verb."}}]}}"#
        )
    };
    let map = |text: &str| super::decision::map_output(text, &[target()]);
    let shorter = DecisionCandidate::ProseChange {
        text: "Shorter.".to_owned(),
    };

    assert_eq!(
        [
            map(&prose(BLOCK)),
            map(&prose("undeclared")),
            map("```json\n{\"kind\":\"clarification\",\"question\":\"Which scene?\"}\n```"),
            map("Keep the second sentence."),
            map(r#"{"kind":"prose_change","summary":"Shorter."}"#),
            super::decision::map_output(&prose(BLOCK), &[]),
        ],
        [
            final_answer(
                shorter.clone(),
                Some(vec![change(
                    BLOCK,
                    &target().chapter_id,
                    &target().base_revision_id
                )])
            ),
            final_answer(shorter, Some(vec![change("undeclared", "", "")])),
            final_answer(
                DecisionCandidate::Clarification {
                    question: "Which scene?".to_owned()
                },
                None
            ),
            advisory("Keep the second sentence."),
            advisory(r#"{"kind":"prose_change","summary":"Shorter."}"#),
            advisory(&prose(BLOCK)),
        ]
    );
}

#[test]
fn a_refusal_a_function_call_or_an_incomplete_response_supplies_no_candidate() {
    let observe = |status: &str, output: serde_json::Value| {
        let body = json!({"id": "resp_01", "status": status, "output": [output]});
        match super::response::observe(body.to_string().as_bytes(), &[target()]) {
            Observation::Terminal(response) => (response.items, response.output),
            other => panic!("not a terminal response: {other:?}"),
        }
    };
    let refusal = json!({"id": "msg_01", "type": "message", "status": "completed",
                         "content": [{"type": "refusal", "refusal": "No."}]});
    let call = json!({"id": "fc_01", "type": "function_call", "status": "completed",
                      "call_id": "call_01", "arguments": "{\"q\""});
    let partial = json!({"id": "msg_01", "type": "message",
                         "content": [{"type": "output_text", "text": "{\"kind\""}]});
    let (assistant, complete) = (StreamItemRole::Assistant, StreamItemState::Complete);

    assert_eq!(
        [
            observe("completed", refusal),
            observe("completed", call),
            observe("incomplete", partial),
        ],
        [
            (
                vec![NativeStreamItem {
                    refusal: Some("No.".to_owned()),
                    ..item("msg_01", assistant, complete)
                }],
                None
            ),
            (
                vec![NativeStreamItem {
                    call_id: Some("call_01".to_owned()),
                    arguments: Some("{\"q\"".to_owned()),
                    ..item("fc_01", StreamItemRole::Tool, complete)
                }],
                None
            ),
            (
                vec![NativeStreamItem {
                    text: Some("{\"kind\"".to_owned()),
                    ..item("msg_01", assistant, StreamItemState::Incomplete)
                }],
                None
            ),
        ]
    );
}

#[tokio::test]
async fn each_transport_outcome_maps_to_rejected_not_submitted_or_outcome_unknown() {
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        format!("http://{}/api/plan/v3", listener.local_addr().unwrap())
    };
    let rejected = |reason: &str| Observation::Rejected {
        reason: reason.to_owned(),
    };
    let unknown = Observation::OutcomeUnknown {
        response_reference: None,
    };
    let limited = r#"{"error":{"code":"RateLimitExceeded.EndpointRPM","message":"echo: Tighten"}}"#;

    assert_eq!(
        [
            exchange(Reply::Http(429, limited.to_owned())).await,
            exchange(Reply::Http(401, r#"{"error":{"code":"a b"}}"#.to_owned())).await,
            exchange(Reply::Http(500, "{}".to_owned())).await,
            exchange(Reply::Http(200, "not json".to_owned())).await,
            exchange(Reply::Drop).await,
            exchange(Reply::Hold).await,
            exchange_at(&closed).await.0,
        ],
        [
            rejected("http_429:RateLimitExceeded.EndpointRPM"),
            rejected("http_401"),
            unknown.clone(),
            unknown.clone(),
            unknown.clone(),
            unknown,
            Observation::NotSubmitted,
        ]
    );
}

#[tokio::test]
async fn a_claimed_attempt_retrieval_and_an_unknown_bound_send_nothing() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/api/plan/v3", listener.local_addr().unwrap());
    let adapter = AgentPlanResponses {
        resolver: CanaryResolver::default(),
    };
    let claimed = RequestAttempt::Claimed(DispatchClaim {
        model_attempt_id: "018f0000-0000-7001-8000-0000000000d1".to_owned(),
    });
    let prepared = adapter
        .prepare(&create(route(&base), claimed))
        .await
        .ok()
        .unwrap();
    let observation = adapter
        .exchange(prepared.prepared, &mut RecordingSink::default())
        .await;
    let retrieve = DestinationRequest::Retrieve(RetrieveRequest {
        attempt: RequestAttempt::New,
        route: route(&base),
        purpose: RetrievePurpose::OriginalResult,
        original_model_attempt_id: "018f0000-0000-7001-8000-0000000000d1".to_owned(),
        response_reference: "resp_01".to_owned(),
    });
    let unbounded = create(
        RequestRoute {
            bounds: None,
            ..route(&base)
        },
        RequestAttempt::New,
    );
    let refusals = [
        adapter.prepare(&retrieve).await.err(),
        adapter.prepare(&unbounded).await.err(),
    ];
    let connected = tokio::select! {
        biased;
        accepted = listener.accept() => accepted.is_ok(),
        () = std::future::ready(()) => false,
    };

    assert_eq!(
        (
            observation,
            refusals,
            connected,
            *adapter.resolver.0.lock().unwrap()
        ),
        (
            Observation::OutcomeUnknown {
                response_reference: None
            },
            [Some(PreDispatchRefusal::UnsupportedRequest); 2],
            false,
            0
        )
    );
}
