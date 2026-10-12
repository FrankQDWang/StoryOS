//! The Model Provider Adapter of the Volcengine Agent Plan Responses route (ADR 0033, ADR 0039).

mod decision;
mod response;
mod stream;
mod wire;

use std::time::Duration;

use storyos_application::{
    CacheSetting, CredentialResolver, DeclaredTarget, DestinationRequest, ModelProviderAdapter,
    ModelStreamSink, Observation, PreDispatchRefusal, PreparedRequest, RequestAttempt,
    ResolvedCredential, WirePayloadProjection,
};
use storyos_core::ModelAdapter;

const EXECUTION_PROFILE: &str = "storyos.volcengine-agent-plan.execution.v1";

/// The rejection of a prior response reference that the destination does not have
/// (live observation, 2026-10-12). It does not tell an expired reference from another one.
const PREVIOUS_RESPONSE_NOT_FOUND: &str = "http_400:InvalidParameter.PreviousResponseNotFound";

/// Sends each admitted Create request once, with no retry, redirect, or fallback. It asks for
/// no explicit cache. The streaming mapping reads typed server-sent events.
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
        continues: bool,
        /// The mapping revision of a streamed request, `None` for a request that is not streamed.
        stream: Option<String>,
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
        let streams = route.mapping_revision == storyos_core::AGENT_PLAN_STREAMING_MAPPING_REVISION;
        // Only a continuation mapping can send a prior response reference.
        if create.previous_response_reference.is_some()
            && !streams
            && route.mapping_revision != storyos_core::AGENT_PLAN_CONTINUATION_MAPPING_REVISION
        {
            return Err(PreDispatchRefusal::UnsupportedRequest);
        }
        let body = wire::create_body(create, &route.provider_model_id, bounds, streams);
        let projection = WirePayloadProjection {
            execution_profile: EXECUTION_PROFILE.to_owned(),
            mapping_revision: route.mapping_revision.clone(),
            digest: format!("sha256:{}", storyos_core::hex_sha256(body.as_bytes())),
            serialized_payload: Some(body.clone()),
            cache_setting: Some(CacheSetting::DestinationDefault),
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
                continues: create.previous_response_reference.is_some(),
                stream: streams.then(|| route.mapping_revision.clone()),
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
            continues,
            stream,
        } = prepared.0
        else {
            return Observation::OutcomeUnknown {
                response_reference: None,
            };
        };
        let mut observation = match send(&url, body, &credential, timeout).await {
            Sent::Settled(observation) => observation,
            Sent::Accepted(response) => match stream {
                Some(mapping_revision) => {
                    return stream::observe(
                        response,
                        &targets,
                        &mapping_revision,
                        stream::CommitPolicy {
                            bound: storyos_application::PROVISIONAL_STREAM_EVENT_BOUND,
                            interval: storyos_application::PROVISIONAL_STREAM_COMMIT_INTERVAL,
                        },
                        sink,
                    )
                    .await;
                }
                None => match response.bytes().await {
                    Ok(bytes) => response::observe(&bytes, &targets),
                    Err(_) => Observation::OutcomeUnknown {
                        response_reference: None,
                    },
                },
            },
        };
        if let Observation::Rejected { reason } = &mut observation
            && continues
            && reason.as_str() == PREVIOUS_RESPONSE_NOT_FOUND
        {
            *reason = format!("continuation_unusable:{reason}");
        }
        if let Observation::Terminal(response) = &observation
            && !response.items.is_empty()
        {
            // The response is already complete, so a stop changes nothing that it reports.
            sink.append(&response.items).await;
        }
        observation
    }
}

/// The result of one POST: a successful status with its unread body, or a settled observation.
enum Sent {
    Accepted(reqwest::Response),
    Settled(Observation),
}

/// One POST with no retry. A failure before the request leaves this process is NotSubmitted.
async fn send(url: &str, body: String, credential: &ResolvedCredential, timeout: Duration) -> Sent {
    let unknown = Observation::OutcomeUnknown {
        response_reference: None,
    };
    let Ok(client) = reqwest::Client::builder()
        .http1_only()
        .pool_max_idle_per_host(/*max*/ 0)
        .retry(reqwest::retry::never())
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .timeout(timeout)
        .build()
    else {
        return Sent::Settled(Observation::NotSubmitted);
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
        Err(error) if error.is_connect() || error.is_builder() => {
            return Sent::Settled(Observation::NotSubmitted);
        }
        Err(_) => return Sent::Settled(unknown),
    };
    let status = response.status();
    if status.is_server_error() {
        return Sent::Settled(unknown);
    }
    if !status.is_success() {
        let bytes = response.bytes().await;
        return Sent::Settled(Observation::Rejected {
            reason: rejection_reason(status.as_u16(), bytes.as_deref().unwrap_or_default()),
        });
    }
    Sent::Accepted(response)
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
