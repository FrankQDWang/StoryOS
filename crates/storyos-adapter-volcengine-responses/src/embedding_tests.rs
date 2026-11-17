use std::time::Duration;

use serde_json::json;
use storyos_application::{
    CredentialReference, CredentialResolver, EmbeddingObservation, EmbeddingProviderAdapter,
    EmbeddingRoute, EmbeddingUsage, PreDispatchRefusal, ResolvedCredential,
};
use storyos_core::{AGENT_PLAN_EMBEDDING_REGISTRATION, EmbeddingInput, EmbeddingRegistration};

use super::AgentPlanEmbeddings;
use crate::tests::{CanaryResolver, Reply, destination};

fn route(endpoint: &str) -> EmbeddingRoute {
    EmbeddingRoute {
        registration_revision: AGENT_PLAN_EMBEDDING_REGISTRATION.revision.to_owned(),
        endpoint: Some(endpoint.to_owned()),
        credential_reference: Some(CredentialReference(
            "settings:STORYOS_AGENT_PLAN_API_KEY".to_owned(),
        )),
    }
}

fn passage(text: &str) -> EmbeddingInput {
    EmbeddingInput::Passage {
        chapter_id: "ch-1".to_owned(),
        revision_id: "rev-1".to_owned(),
        passage_digest: "sha256:p".to_owned(),
        text: text.to_owned(),
    }
}

fn adapter() -> AgentPlanEmbeddings<CanaryResolver> {
    AgentPlanEmbeddings {
        resolver: CanaryResolver::default(),
    }
}

/// The reply form of the record of S7-C1, with a vector of `dimensions` values.
fn vector_reply(dimensions: usize) -> String {
    json!({
        "id": "021700000000000",
        "object": "list",
        "data": {"object": "embedding", "embedding": vec![0.25; dimensions]},
        "usage": {"prompt_tokens": 9, "total_tokens": 9},
    })
    .to_string()
}

/// Sends `input` to a destination that answers with `reply`, and returns the observation and
/// the task that gives the received request.
async fn exchange(
    reply: Reply,
    input: &EmbeddingInput,
    timeout: Duration,
) -> (
    EmbeddingObservation,
    tokio::task::JoinHandle<crate::tests::Received>,
) {
    let (base, served) = destination(reply).await;
    let embeddings = adapter();
    let mut prepared = embeddings
        .prepare(&AGENT_PLAN_EMBEDDING_REGISTRATION, &route(&base), input)
        .await
        .unwrap();
    prepared.prepared.timeout = timeout;
    (embeddings.exchange(prepared.prepared).await, served)
}

#[tokio::test]
async fn each_cause_sends_one_text_in_the_recorded_form_with_the_resolved_credential() {
    let question = EmbeddingInput::Question {
        tool_call_id: "call-1".to_owned(),
        text: "Where is the lantern?".to_owned(),
    };
    for input in [passage("The tide turned."), question] {
        let (observation, served) = exchange(
            Reply::Http(200, vector_reply(/*dimensions*/ 1_024)),
            &input,
            Duration::from_secs(/*secs*/ 5),
        )
        .await;
        let (request_line, authorization, body) = served.await.unwrap();

        assert_eq!(
            request_line,
            "POST /api/plan/v3/embeddings/multimodal HTTP/1.1"
        );
        assert_eq!(
            authorization.as_deref(),
            Some("Bearer storyos-credential-canary-5e21")
        );
        assert_eq!(
            body,
            storyos_core::canonical_json(&json!({
                "model": "doubao-embedding-vision-251215",
                "instructions": input.cause().instructions(),
                "input": [{"type": "text", "text": input.text()}],
                "encoding_format": "float",
                "dimensions": 1_024,
            }))
        );
        assert_eq!(
            observation,
            EmbeddingObservation::Vector {
                values: vec![0.25; 1_024],
                usage: EmbeddingUsage::Reported { input_tokens: 9 },
            }
        );
    }
}

#[tokio::test]
async fn the_projection_names_the_mapping_and_the_digest_of_the_body_and_no_text() {
    let embeddings = adapter();
    let input = passage("The tide turned.");

    let prepared = embeddings
        .prepare(
            &AGENT_PLAN_EMBEDDING_REGISTRATION,
            &route("http://127.0.0.1:1/api/plan/v3/"),
            &input,
        )
        .await
        .unwrap();

    assert_eq!(
        prepared.projection.mapping_revision,
        AGENT_PLAN_EMBEDDING_REGISTRATION.mapping_revision
    );
    assert_eq!(
        prepared.projection.digest,
        format!(
            "sha256:{}",
            storyos_core::hex_sha256(prepared.prepared.body.as_bytes())
        )
    );
    assert_eq!(
        prepared.prepared.url,
        "http://127.0.0.1:1/api/plan/v3/embeddings/multimodal"
    );
    assert_eq!(
        prepared.prepared.timeout,
        storyos_core::EMBEDDING_REQUEST_TIMEOUT
    );
}

/// A resolver that has no value for each reference.
struct EmptyResolver;

impl CredentialResolver for EmptyResolver {
    async fn resolve(&self, _reference: &CredentialReference) -> Option<ResolvedCredential> {
        None
    }
}

/// A resolver that a refusal before the resolution never calls.
struct UncalledResolver;

impl CredentialResolver for UncalledResolver {
    async fn resolve(&self, _reference: &CredentialReference) -> Option<ResolvedCredential> {
        panic!("the adapter resolved a credential for a refused request")
    }
}

static OTHER_BOUNDS: EmbeddingRegistration = EmbeddingRegistration {
    request_bounds: "embedding_request_v0",
    ..AGENT_PLAN_EMBEDDING_REGISTRATION
};
static OTHER_MAPPING: EmbeddingRegistration = EmbeddingRegistration {
    mapping_revision: "storyos.volcengine-agent-plan.embedding.v0",
    ..AGENT_PLAN_EMBEDDING_REGISTRATION
};

#[tokio::test]
async fn the_adapter_refuses_before_the_claim_without_the_mapping_the_endpoint_or_a_credential() {
    let input = passage("The tide turned.");
    let base = "http://127.0.0.1:1/api/plan/v3";
    let no_endpoint = EmbeddingRoute {
        endpoint: None,
        ..route(base)
    };
    let no_reference = EmbeddingRoute {
        credential_reference: None,
        ..route(base)
    };
    let embeddings = AgentPlanEmbeddings {
        resolver: UncalledResolver,
    };
    let rows: [(
        &'static EmbeddingRegistration,
        EmbeddingRoute,
        PreDispatchRefusal,
    ); 4] = [
        (
            &OTHER_BOUNDS,
            route(base),
            PreDispatchRefusal::UnsupportedRequest,
        ),
        (
            &OTHER_MAPPING,
            route(base),
            PreDispatchRefusal::UnsupportedRequest,
        ),
        (
            &AGENT_PLAN_EMBEDDING_REGISTRATION,
            no_endpoint,
            PreDispatchRefusal::UnsupportedRequest,
        ),
        (
            &AGENT_PLAN_EMBEDDING_REGISTRATION,
            no_reference,
            PreDispatchRefusal::CredentialUnavailable,
        ),
    ];

    for (registration, route, expected) in rows {
        let refusal = embeddings
            .prepare(registration, &route, &input)
            .await
            .err()
            .unwrap();
        assert_eq!(refusal, expected);
    }
    let unresolved = AgentPlanEmbeddings {
        resolver: EmptyResolver,
    }
    .prepare(&AGENT_PLAN_EMBEDDING_REGISTRATION, &route(base), &input)
    .await
    .err()
    .unwrap();
    assert_eq!(unresolved, PreDispatchRefusal::CredentialUnavailable);
}

#[tokio::test]
async fn each_other_reply_form_gives_one_observation() {
    let input = passage("The tide turned.");
    let short = Duration::from_millis(/*millis*/ 300);
    let rows = [
        (
            Reply::Http(200, json!({"data": [{"embedding": [0.25]}]}).to_string()),
            EmbeddingObservation::InvalidResult,
        ),
        (
            Reply::Http(200, "not json".to_owned()),
            EmbeddingObservation::InvalidResult,
        ),
        (
            Reply::Http(200, json!({"data": {"embedding": [0.25, "x"]}}).to_string()),
            EmbeddingObservation::InvalidResult,
        ),
        (
            Reply::Http(200, json!({"data": {"embedding": [0.5]}}).to_string()),
            EmbeddingObservation::Vector {
                values: vec![0.5],
                usage: EmbeddingUsage::Unknown,
            },
        ),
        (Reply::Drop, EmbeddingObservation::OutcomeUnknown),
        (Reply::Hold, EmbeddingObservation::OutcomeUnknown),
    ];

    for (reply, expected) in rows {
        let (observation, _) = exchange(reply, &input, short).await;
        assert_eq!(observation, expected);
    }
}

#[tokio::test]
async fn the_text_of_the_probe_above_the_limit_keeps_the_native_reason_and_no_provider_text() {
    // The text of request E7 of the record of S7-C1, the same text as the fake test
    // `the_text_of_the_probe_above_the_limit_gets_the_native_reason_of_the_route`. The scripted
    // reply is the reply of the route to E7.
    let text = "The probe repeats one constant sentence. ".repeat(25_000);
    let over_limit = json!({"error": {
        "code": "InvalidParameter",
        "message": "Total tokens of image and text exceed max message tokens. Request id: 0217",
        "param": "",
        "type": "BadRequest",
    }})
    .to_string();

    let (observation, served) = exchange(
        Reply::Http(400, over_limit),
        &passage(&text),
        Duration::from_secs(/*secs*/ 5),
    )
    .await;

    let (_, _, body) = served.await.unwrap();
    assert!(body.contains(&text));
    assert_eq!(
        observation,
        EmbeddingObservation::Rejected {
            reason: "input_too_long:http_400:InvalidParameter".to_owned(),
        }
    );
}

#[tokio::test]
async fn a_closed_port_is_not_submitted() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/api/plan/v3", listener.local_addr().unwrap());
    drop(listener);
    let embeddings = adapter();
    let prepared = embeddings
        .prepare(
            &AGENT_PLAN_EMBEDDING_REGISTRATION,
            &route(&base),
            &passage("The tide turned."),
        )
        .await
        .unwrap();

    assert_eq!(
        embeddings.exchange(prepared.prepared).await,
        EmbeddingObservation::NotSubmitted
    );
}
