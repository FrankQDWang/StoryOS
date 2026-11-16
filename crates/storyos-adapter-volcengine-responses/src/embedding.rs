//! One text embedding request of the multimodal embedding path of the Agent Plan route, for the
//! embedding probe of ADR 0095 section 5 (ADR 0049 amendment of 2026-11-16). It has no Model
//! Gateway admission. The probe observation keeps the native reply, because the probe records it.

use std::time::Duration;

use serde_json::{Value, json};
use storyos_application::{CredentialReference, CredentialResolver, PreDispatchRefusal};

/// The path of the multimodal embedding operation below the endpoint of the route.
const EMBEDDING_PATH: &str = "/embeddings/multimodal";

/// The maximum number of characters of a reply that is not JSON that the observation keeps.
const TEXT_REPLY_LIMIT: usize = 2048;

/// The text items of one request. The route returns the vectors of the items in one reply.
pub struct EmbeddingRequest {
    pub endpoint: String,
    pub model: String,
    pub instructions: String,
    pub texts: Vec<String>,
    /// `None` sends no `dimensions` member, so the route uses its default.
    pub dimensions: Option<u32>,
    pub timeout: Duration,
}

/// Sends each prepared embedding request once, with no retry and no redirect. It resolves the
/// Credential Reference through `resolver` in `prepare`.
pub struct AgentPlanEmbeddings<R> {
    pub resolver: R,
}

/// The single-use prepared embedding request.
pub struct PreparedEmbedding {
    url: String,
    body: String,
    credential: storyos_application::ResolvedCredential,
    timeout: Duration,
}

impl PreparedEmbedding {
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The canonical JSON body that `exchange` sends.
    pub fn body(&self) -> &str {
        &self.body
    }
}

/// The result of one embedding exchange. It has no error channel.
#[derive(Debug, PartialEq)]
pub enum EmbeddingObservation {
    /// The request did not leave this process.
    NotSubmitted,
    /// The request possibly left this process, and no status came back.
    OutcomeUnknown,
    /// The route answered. A body that is not JSON is kept as bounded text.
    Answered {
        status: u16,
        request_id: Option<String>,
        body: Value,
    },
}

impl<R: CredentialResolver> AgentPlanEmbeddings<R> {
    /// Makes the canonical body and resolves the credential. An empty text list is unsupported.
    pub async fn prepare(
        &self,
        request: &EmbeddingRequest,
        reference: &CredentialReference,
    ) -> Result<PreparedEmbedding, PreDispatchRefusal> {
        if request.texts.is_empty() {
            return Err(PreDispatchRefusal::UnsupportedRequest);
        }
        let input = request
            .texts
            .iter()
            .map(|text| json!({"type": "text", "text": text}))
            .collect::<Vec<_>>();
        let mut body = json!({
            "model": request.model,
            "instructions": request.instructions,
            "input": input,
            "encoding_format": "float",
        });
        if let Some(dimensions) = request.dimensions {
            body["dimensions"] = json!(dimensions);
        }
        let credential = self
            .resolver
            .resolve(reference)
            .await
            .ok_or(PreDispatchRefusal::CredentialUnavailable)?;
        Ok(PreparedEmbedding {
            url: format!("{}{EMBEDDING_PATH}", request.endpoint.trim_end_matches('/')),
            body: storyos_core::canonical_json(&body),
            credential,
            timeout: request.timeout,
        })
    }

    /// One POST of the prepared body.
    pub async fn exchange(&self, prepared: PreparedEmbedding) -> EmbeddingObservation {
        let Some(client) = crate::single_request_client(prepared.timeout) else {
            return EmbeddingObservation::NotSubmitted;
        };
        let sent = client
            .post(&prepared.url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .bearer_auth(prepared.credential.expose())
            .body(prepared.body)
            .send()
            .await;
        let response = match sent {
            Ok(response) => response,
            Err(error) if error.is_builder() || error.is_connect() => {
                return EmbeddingObservation::NotSubmitted;
            }
            Err(_) => return EmbeddingObservation::OutcomeUnknown,
        };
        let status = response.status().as_u16();
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let Ok(bytes) = response.bytes().await else {
            return EmbeddingObservation::OutcomeUnknown;
        };
        let body = serde_json::from_slice(&bytes).unwrap_or_else(|_| {
            Value::String(
                String::from_utf8_lossy(&bytes)
                    .chars()
                    .take(TEXT_REPLY_LIMIT)
                    .collect(),
            )
        });
        EmbeddingObservation::Answered {
            status,
            request_id,
            body,
        }
    }
}

#[cfg(test)]
#[path = "embedding_tests.rs"]
mod tests;
