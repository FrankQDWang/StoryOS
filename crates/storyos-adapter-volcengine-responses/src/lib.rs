//! The Model Provider Adapter of the Volcengine Agent Plan Responses route (ADR 0033, ADR 0039).

use std::convert::Infallible;

use storyos_application::{
    CredentialResolver, DestinationRequest, ModelProviderAdapter, ModelStreamSink, Observation,
    PreDispatchRefusal, PreparedRequest,
};
use storyos_core::ModelAdapter;

/// Resolves the Credential Reference of each request through `resolver` before the dispatch
/// claim. S4-02 owns the wire mapping and the exchange, so each request is unsupported for now.
pub struct AgentPlanResponses<R> {
    pub resolver: R,
}

impl<R: CredentialResolver> ModelProviderAdapter for AgentPlanResponses<R> {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = Infallible;

    #[tracing::instrument(skip_all, level = "debug")]
    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<Infallible>, PreDispatchRefusal> {
        let Some(reference) = &request.route().credential_reference else {
            return Err(PreDispatchRefusal::CredentialUnavailable);
        };
        match self.resolver.resolve(reference).await {
            None => Err(PreDispatchRefusal::CredentialUnavailable),
            Some(_credential) => Err(PreDispatchRefusal::UnsupportedRequest),
        }
    }

    async fn exchange(
        &self,
        prepared: Infallible,
        _sink: &mut impl ModelStreamSink,
    ) -> Observation {
        match prepared {}
    }
}
