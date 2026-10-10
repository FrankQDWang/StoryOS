//! The Model Provider Adapter of the Volcengine Agent Plan Responses route (ADR 0033, ADR 0039).

mod decision;
mod response;
mod wire;

use std::time::Duration;

use storyos_application::{
    CredentialResolver, DeclaredTarget, DestinationRequest, ModelProviderAdapter, ModelStreamSink,
    Observation, PreDispatchRefusal, PreparedRequest, RequestAttempt, ResolvedCredential,
    WirePayloadProjection,
};
use storyos_core::ModelAdapter;

const EXECUTION_PROFILE: &str = "storyos.volcengine-agent-plan.execution.v1";
const MAPPING_REVISION: &str = "storyos.volcengine-agent-plan.responses.create.v1";

/// Sends each admitted Create request once, with no streaming, retry, redirect, or fallback.
/// It resolves the Credential Reference of each request through `resolver` before the dispatch
/// claim.
pub struct AgentPlanResponses<R> {
    pub resolver: R,
}

/// The single-use prepared exchange of one Agent Plan request.
pub struct AgentPlanExchange(Exchange);

enum Exchange {
    Send {
        url: String,
        body: String,
        credential: ResolvedCredential,
        timeout: Duration,
        targets: Vec<DeclaredTarget>,
    },
    /// A lost claim committed this Attempt. The request is not sent again.
    Reobserve,
}

impl<R: CredentialResolver> ModelProviderAdapter for AgentPlanResponses<R> {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = AgentPlanExchange;

    #[tracing::instrument(skip_all, level = "debug")]
    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<AgentPlanExchange>, PreDispatchRefusal> {
        // Retrieval and abort are not qualified for this route (S4-04).
        let DestinationRequest::Create(create) = request else {
            return Err(PreDispatchRefusal::UnsupportedRequest);
        };
        let route = &create.route;
        let (Some(endpoint), Some(bounds)) = (&route.endpoint, route.bounds) else {
            return Err(PreDispatchRefusal::UnsupportedRequest);
        };
        let body = wire::create_body(create, &route.provider_model_id, bounds);
        let projection = WirePayloadProjection {
            execution_profile: EXECUTION_PROFILE.to_owned(),
            mapping_revision: MAPPING_REVISION.to_owned(),
            digest: format!("sha256:{}", storyos_core::hex_sha256(body.as_bytes())),
            serialized_payload: Some(body.clone()),
        };
        if let RequestAttempt::Claimed(_) = create.attempt {
            return Ok(PreparedRequest {
                projection,
                prepared: AgentPlanExchange(Exchange::Reobserve),
            });
        }
        let Some(reference) = &route.credential_reference else {
            return Err(PreDispatchRefusal::CredentialUnavailable);
        };
        let Some(credential) = self.resolver.resolve(reference).await else {
            return Err(PreDispatchRefusal::CredentialUnavailable);
        };
        Ok(PreparedRequest {
            projection,
            prepared: AgentPlanExchange(Exchange::Send {
                url: format!("{}/responses", endpoint.trim_end_matches('/')),
                body,
                credential,
                timeout: bounds.timeout,
                targets: create.declared_targets.clone(),
            }),
        })
    }

    #[tracing::instrument(skip_all, level = "debug")]
    async fn exchange(
        &self,
        prepared: AgentPlanExchange,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        let Exchange::Send {
            url,
            body,
            credential,
            timeout,
            targets,
        } = prepared.0
        else {
            return Observation::OutcomeUnknown {
                response_reference: None,
            };
        };
        let observation = send(&url, body, &credential, timeout, &targets).await;
        if let Observation::Terminal(response) = &observation
            && !response.items.is_empty()
        {
            // The response is already complete, so a stop changes nothing that it reports.
            sink.append(&response.items).await;
        }
        observation
    }
}

/// One POST with no retry. A failure before the request leaves this process is NotSubmitted.
async fn send(
    url: &str,
    body: String,
    credential: &ResolvedCredential,
    timeout: Duration,
    targets: &[DeclaredTarget],
) -> Observation {
    let unknown = Observation::OutcomeUnknown {
        response_reference: None,
    };
    let Ok(client) = reqwest::Client::builder()
        .http1_only()
        .pool_max_idle_per_host(0)
        .retry(reqwest::retry::never())
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .timeout(timeout)
        .build()
    else {
        return Observation::NotSubmitted;
    };
    let sent = client
        .post(url)
        .bearer_auth(credential.expose())
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .await;
    let response = match sent {
        Ok(response) => response,
        Err(error) if error.is_connect() || error.is_builder() => return Observation::NotSubmitted,
        Err(_) => return unknown,
    };
    let status = response.status();
    if status.is_server_error() {
        return unknown;
    }
    let bytes = response.bytes().await;
    if !status.is_success() {
        return Observation::Rejected {
            reason: rejection_reason(status.as_u16(), bytes.as_deref().unwrap_or_default()),
        };
    }
    match bytes {
        Ok(bytes) => response::observe(&bytes, targets),
        Err(_) => unknown,
    }
}

/// The HTTP status and the native error code. The Provider message text is not kept.
fn rejection_reason(status: u16, body: &[u8]) -> String {
    let code = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|body| body.get("error")?.get("code")?.as_str().map(str::to_owned))
        .filter(|code| {
            (1..=64).contains(&code.len())
                && code
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
        });
    match code {
        Some(code) => format!("http_{status}:{code}"),
        None => format!("http_{status}"),
    }
}

#[cfg(test)]
#[path = "exchange_tests.rs"]
mod tests;
