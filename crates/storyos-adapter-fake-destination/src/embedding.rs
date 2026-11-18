//! The embedding adapter of the Contract-Faithful Fake Destination (ADR 0095 section 3). The
//! vector depends only on the text, so a restart loses nothing.

use serde_json::json;
use storyos_application::{
    EmbeddingObservation, EmbeddingProjection, EmbeddingProviderAdapter, EmbeddingRoute,
    EmbeddingUsage, PreDispatchRefusal, PreparedEmbeddingRequest,
};
use storyos_core::{
    EMBEDDING_DIMENSIONS, EMBEDDING_TEXT_TOKEN_LIMIT, EmbeddingInput, EmbeddingRegistration,
};

/// The text markers that select an observation other than a vector.
pub const EMBEDDING_REJECTION_MARKER: &str = "SCRIPT:embedding-rejected";
pub const EMBEDDING_NOT_SUBMITTED_MARKER: &str = "SCRIPT:embedding-not-submitted";
pub const EMBEDDING_UNKNOWN_MARKER: &str = "SCRIPT:embedding-unknown";

/// The rejection of a text above the input limit. It is the native reason that the Agent Plan
/// adapter keeps for the reply of the record of S7-C1: HTTP 400 `InvalidParameter` with the
/// message of an input above the model window.
pub const EMBEDDING_INPUT_TOO_LONG: &str = "input_too_long:http_400:InvalidParameter";

/// The fake embedding destination. It needs no credential and no endpoint.
pub struct FakeEmbeddings;

/// The single-use prepared exchange of one fake embedding request.
pub struct FakeEmbeddingExchange {
    text: String,
    dimensions: usize,
}

impl EmbeddingProviderAdapter for FakeEmbeddings {
    const REGISTRATIONS: &'static [&'static EmbeddingRegistration] =
        &[&storyos_core::HOST_FAKE_EMBEDDING_REGISTRATION];
    type Prepared = FakeEmbeddingExchange;

    #[tracing::instrument(skip_all, level = "debug")]
    async fn prepare(
        &self,
        registration: &'static EmbeddingRegistration,
        _route: &EmbeddingRoute,
        input: &EmbeddingInput,
    ) -> Result<PreparedEmbeddingRequest<Self::Prepared>, PreDispatchRefusal> {
        let body = json!({
            "model": registration.provider_model_id,
            "instructions": input.cause().instructions(),
            "input": [{"type": "text", "text": input.text()}],
            "dimensions": registration.dimensions,
        });
        Ok(PreparedEmbeddingRequest {
            projection: EmbeddingProjection {
                mapping_revision: registration.mapping_revision,
                digest: format!(
                    "sha256:{}",
                    storyos_core::hex_sha256(storyos_core::canonical_json(&body).as_bytes())
                ),
            },
            prepared: FakeEmbeddingExchange {
                text: input.text().to_owned(),
                dimensions: registration.dimensions,
            },
        })
    }

    #[tracing::instrument(skip_all, level = "debug")]
    async fn exchange(&self, prepared: FakeEmbeddingExchange) -> EmbeddingObservation {
        let FakeEmbeddingExchange { text, dimensions } = prepared;
        let rejected = |reason: &str| EmbeddingObservation::Rejected {
            reason: reason.to_owned(),
        };
        if text.contains(EMBEDDING_NOT_SUBMITTED_MARKER) {
            EmbeddingObservation::NotSubmitted
        } else if text.contains(EMBEDDING_UNKNOWN_MARKER) {
            EmbeddingObservation::OutcomeUnknown
        } else if text.contains(EMBEDDING_REJECTION_MARKER) {
            rejected("fake_rejection")
        } else if text.is_empty() {
            rejected("empty_text")
        } else if storyos_core::count_context_item_tokens(&text) > EMBEDDING_TEXT_TOKEN_LIMIT {
            rejected(EMBEDDING_INPUT_TOO_LONG)
        } else if dimensions != EMBEDDING_DIMENSIONS {
            rejected("dimensions_not_offered")
        } else {
            EmbeddingObservation::Vector {
                values: fake_vector(&text),
                usage: EmbeddingUsage::Reported {
                    input_tokens: storyos_core::count_context_item_tokens(&text),
                },
            }
        }
    }
}

/// A unit vector of the characters and the character pairs of `text`. Each item adds one to the
/// bucket of its FNV-1a hash, so texts with many common characters give near vectors.
pub fn fake_vector(text: &str) -> Vec<f32> {
    let mut vector = vec![0.0_f32; EMBEDDING_DIMENSIONS];
    let characters: Vec<char> = text.chars().collect();
    let pairs = characters
        .windows(/*size*/ 2)
        .map(|pair| pair.iter().collect::<String>());
    for item in characters.iter().map(char::to_string).chain(pairs) {
        let hash = item.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
        vector[(hash % EMBEDDING_DIMENSIONS as u64) as usize] += 1.0;
    }
    let length = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if length > 0.0 {
        vector.iter_mut().for_each(|value| *value /= length);
    }
    vector
}

#[cfg(test)]
#[path = "embedding_tests.rs"]
mod tests;
