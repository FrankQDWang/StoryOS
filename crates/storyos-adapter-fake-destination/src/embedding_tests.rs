use storyos_application::{
    EmbeddingObservation, EmbeddingProviderAdapter, EmbeddingRoute, EmbeddingUsage,
};
use storyos_core::{EmbeddingInput, EmbeddingRegistration, HOST_FAKE_EMBEDDING_REGISTRATION};

use super::*;

/// The fake binding has no endpoint and no Credential Reference.
const ROUTE: EmbeddingRoute = EmbeddingRoute {
    registration_revision: String::new(),
    endpoint: None,
    credential_reference: None,
};

fn question(text: &str) -> EmbeddingInput {
    EmbeddingInput::Question {
        tool_call_id: "call-1".to_owned(),
        text: text.to_owned(),
    }
}

async fn observe(registration: &'static EmbeddingRegistration, text: &str) -> EmbeddingObservation {
    let prepared = FakeEmbeddings
        .prepare(registration, &ROUTE, &question(text))
        .await
        .unwrap();
    FakeEmbeddings.exchange(prepared.prepared).await
}

fn vector(observation: EmbeddingObservation) -> Vec<f32> {
    match observation {
        EmbeddingObservation::Vector { values, .. } => values,
        other => panic!("not a vector: {other:?}"),
    }
}

fn cosine(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

#[tokio::test]
async fn the_vector_depends_only_on_the_text_and_near_texts_give_near_vectors() {
    let registration = &HOST_FAKE_EMBEDDING_REGISTRATION;
    let first = observe(registration, "The lantern crossed the river at night.").await;
    let usage = match &first {
        EmbeddingObservation::Vector { usage, .. } => *usage,
        other => panic!("not a vector: {other:?}"),
    };
    let first = vector(first);
    let again = vector(observe(registration, "The lantern crossed the river at night.").await);
    let near = vector(observe(registration, "The lantern crossed the river at dawn.").await);
    let far = vector(observe(registration, "雨下了一整夜。").await);

    assert_eq!(first.len(), 1_024);
    assert_eq!(first, again);
    assert_eq!(usage, EmbeddingUsage::Reported { input_tokens: 39 });
    assert!((cosine(&first, &first) - 1.0).abs() < 1e-5);
    assert!(cosine(&first, &near) > 0.8);
    assert!(cosine(&first, &far) < 0.2);
}

#[tokio::test]
async fn the_projection_holds_the_mapping_and_a_digest_and_no_text() {
    let passage = EmbeddingInput::Passage {
        chapter_id: "ch-1".to_owned(),
        revision_id: "rev-1".to_owned(),
        passage_digest: "sha256:p".to_owned(),
        text: "The tide turned.".to_owned(),
    };
    let registration = &HOST_FAKE_EMBEDDING_REGISTRATION;

    let passage = FakeEmbeddings
        .prepare(registration, &ROUTE, &passage)
        .await
        .unwrap();
    let question = FakeEmbeddings
        .prepare(registration, &ROUTE, &question("The tide turned."))
        .await
        .unwrap();

    assert_eq!(
        passage.projection.mapping_revision,
        "storyos.host-fake.embedding.v1"
    );
    assert!(passage.projection.digest.starts_with("sha256:"));
    assert!(!passage.projection.digest.contains("tide"));
    // The two causes send different instructions, so the bodies differ.
    assert_ne!(passage.projection.digest, question.projection.digest);
}

#[tokio::test]
async fn the_fake_refuses_bad_input_and_a_marker_selects_the_observation() {
    static NARROW: EmbeddingRegistration = EmbeddingRegistration {
        dimensions: 512,
        ..HOST_FAKE_EMBEDDING_REGISTRATION
    };
    let registration = &HOST_FAKE_EMBEDDING_REGISTRATION;
    let rejected = |reason: &str| EmbeddingObservation::Rejected {
        reason: reason.to_owned(),
    };

    assert_eq!(
        [
            observe(registration, "").await,
            observe(registration, &"潮".repeat(8_193)).await,
            observe(&NARROW, "The tide turned.").await,
            observe(registration, "a SCRIPT:embedding-rejected").await,
            observe(registration, "a SCRIPT:embedding-not-submitted").await,
            observe(registration, "a SCRIPT:embedding-unknown").await,
        ],
        [
            rejected("empty_text"),
            rejected(EMBEDDING_INPUT_TOO_LONG),
            rejected("dimensions_not_offered"),
            rejected("fake_rejection"),
            EmbeddingObservation::NotSubmitted,
            EmbeddingObservation::OutcomeUnknown,
        ]
    );
    assert_eq!(
        vector(observe(registration, &"潮".repeat(8_192)).await).len(),
        1_024
    );
}

#[tokio::test]
async fn the_text_of_the_probe_above_the_limit_gets_the_native_reason_of_the_route() {
    // The text of request E7 of the record of S7-C1. The Agent Plan adapter test of this text
    // keeps the same reason from the reply of the route.
    let text = "The probe repeats one constant sentence. ".repeat(25_000);

    assert_eq!(
        observe(&HOST_FAKE_EMBEDDING_REGISTRATION, &text).await,
        EmbeddingObservation::Rejected {
            reason: "input_too_long:http_400:InvalidParameter".to_owned(),
        }
    );
}
