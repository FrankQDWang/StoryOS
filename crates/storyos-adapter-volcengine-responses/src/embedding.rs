//! The embedding adapter of the multimodal embedding path of the Agent Plan route (ADR 0095
//! section 3). One request has one text, because the route gives one vector for each request.
//! The request form is the form of the record of S7-C1
//! (`docs/research/embedding-and-pgvector-probe-2026-11-16.md`).

use std::time::Duration;

use serde_json::{Value, json};
use storyos_application::{
    CredentialResolver, EmbeddingObservation, EmbeddingProjection, EmbeddingProviderAdapter,
    EmbeddingRoute, EmbeddingUsage, PreDispatchRefusal, PreparedEmbeddingRequest,
    ResolvedCredential,
};
use storyos_core::{
    AGENT_PLAN_EMBEDDING_REGISTRATION, EMBEDDING_REQUEST_BOUNDS, EMBEDDING_REQUEST_TIMEOUT,
    EmbeddingInput, EmbeddingRegistration,
};

/// The path of the multimodal embedding operation below the endpoint of the route.
const EMBEDDING_PATH: &str = "/embeddings/multimodal";

/// Sends each prepared embedding request once, with no retry and no redirect. It resolves the
/// Credential Reference of the route through `resolver` in `prepare`, before the dispatch claim.
pub struct AgentPlanEmbeddings<R> {
    pub resolver: R,
}

/// The single-use prepared exchange of one embedding request.
pub struct AgentPlanEmbeddingExchange {
    url: String,
    body: String,
    credential: ResolvedCredential,
    timeout: Duration,
}

impl<R: CredentialResolver> EmbeddingProviderAdapter for AgentPlanEmbeddings<R> {
    const REGISTRATIONS: &'static [&'static EmbeddingRegistration] =
        &[&storyos_core::AGENT_PLAN_EMBEDDING_REGISTRATION];
    type Prepared = AgentPlanEmbeddingExchange;

    /// Refuses a Registration with another mapping revision or other request bounds, and a route
    /// with no endpoint, before it resolves the credential.
    #[tracing::instrument(skip_all, level = "debug")]
    async fn prepare(
        &self,
        registration: &'static EmbeddingRegistration,
        route: &EmbeddingRoute,
        input: &EmbeddingInput,
    ) -> Result<PreparedEmbeddingRequest<Self::Prepared>, PreDispatchRefusal> {
        let known = registration.mapping_revision
            == AGENT_PLAN_EMBEDDING_REGISTRATION.mapping_revision
            && registration.request_bounds == EMBEDDING_REQUEST_BOUNDS;
        let (true, Some(endpoint)) = (known, route.endpoint.as_deref()) else {
            return Err(PreDispatchRefusal::UnsupportedRequest);
        };
        let body = storyos_core::canonical_json(&json!({
            "model": registration.provider_model_id,
            "instructions": input.cause().instructions(),
            "input": [{"type": "text", "text": input.text()}],
            "encoding_format": "float",
            "dimensions": registration.dimensions,
        }));
        let reference = route
            .credential_reference
            .as_ref()
            .ok_or(PreDispatchRefusal::CredentialUnavailable)?;
        let credential = self
            .resolver
            .resolve(reference)
            .await
            .ok_or(PreDispatchRefusal::CredentialUnavailable)?;
        Ok(PreparedEmbeddingRequest {
            projection: EmbeddingProjection {
                mapping_revision: registration.mapping_revision,
                digest: format!("sha256:{}", storyos_core::hex_sha256(body.as_bytes())),
            },
            prepared: AgentPlanEmbeddingExchange {
                url: format!("{}{EMBEDDING_PATH}", endpoint.trim_end_matches('/')),
                body,
                credential,
                timeout: EMBEDDING_REQUEST_TIMEOUT,
            },
        })
    }

    /// One POST of the prepared body.
    #[tracing::instrument(skip_all, level = "debug")]
    async fn exchange(&self, prepared: AgentPlanEmbeddingExchange) -> EmbeddingObservation {
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
        let Ok(bytes) = response.bytes().await else {
            return EmbeddingObservation::OutcomeUnknown;
        };
        observation(status, &bytes)
    }
}

/// The observation of one reply. A refusal keeps the status and the native code, and no text.
fn observation(status: u16, body: &[u8]) -> EmbeddingObservation {
    if !(200..300).contains(&status) {
        return EmbeddingObservation::Rejected {
            reason: crate::rejection_reason(status, body),
        };
    }
    let Ok(reply) = serde_json::from_slice::<Value>(body) else {
        return EmbeddingObservation::InvalidResult;
    };
    let values = reply
        .pointer("/data/embedding")
        .and_then(Value::as_array)
        .and_then(|values| {
            values
                .iter()
                .map(|value| value.as_f64().map(|value| value as f32))
                .collect::<Option<Vec<_>>>()
        });
    let Some(values) = values else {
        return EmbeddingObservation::InvalidResult;
    };
    let usage = reply
        .pointer("/usage/prompt_tokens")
        .and_then(Value::as_u64)
        .map_or(EmbeddingUsage::Unknown, |input_tokens| {
            EmbeddingUsage::Reported { input_tokens }
        });
    EmbeddingObservation::Vector { values, usage }
}

#[cfg(test)]
#[path = "embedding_tests.rs"]
mod tests;
