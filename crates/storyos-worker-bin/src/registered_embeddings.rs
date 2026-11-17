use storyos_adapter_fake_destination::{FakeEmbeddingExchange, FakeEmbeddings};
use storyos_adapter_volcengine_responses::{AgentPlanEmbeddingExchange, AgentPlanEmbeddings};
use storyos_application::{
    CredentialResolver, EmbeddingObservation, EmbeddingProviderAdapter, EmbeddingRoute,
    PreDispatchRefusal, PreparedEmbeddingRequest,
};
use storyos_core::{EmbeddingAdapter, EmbeddingInput, EmbeddingRegistration};

/// Serves each embedding request with the adapter of its Embedding Registration. It never uses
/// the fake adapter in place of another adapter (ADR 0039, ADR 0095 section 3).
pub(crate) struct RegisteredEmbeddings<R> {
    pub(crate) fake: FakeEmbeddings,
    pub(crate) agent_plan: AgentPlanEmbeddings<R>,
}

pub(crate) enum RegisteredEmbeddingExchange {
    Fake(FakeEmbeddingExchange),
    AgentPlan(AgentPlanEmbeddingExchange),
}

impl<R: CredentialResolver> EmbeddingProviderAdapter for RegisteredEmbeddings<R> {
    type Prepared = RegisteredEmbeddingExchange;

    async fn prepare(
        &self,
        registration: &'static EmbeddingRegistration,
        route: &EmbeddingRoute,
        input: &EmbeddingInput,
    ) -> Result<PreparedEmbeddingRequest<RegisteredEmbeddingExchange>, PreDispatchRefusal> {
        match registration.adapter {
            EmbeddingAdapter::HostFake => {
                let prepared = self.fake.prepare(registration, route, input).await?;
                Ok(PreparedEmbeddingRequest {
                    projection: prepared.projection,
                    prepared: RegisteredEmbeddingExchange::Fake(prepared.prepared),
                })
            }
            EmbeddingAdapter::VolcengineAgentPlanEmbeddings => {
                let prepared = self.agent_plan.prepare(registration, route, input).await?;
                Ok(PreparedEmbeddingRequest {
                    projection: prepared.projection,
                    prepared: RegisteredEmbeddingExchange::AgentPlan(prepared.prepared),
                })
            }
        }
    }

    async fn exchange(&self, prepared: RegisteredEmbeddingExchange) -> EmbeddingObservation {
        match prepared {
            RegisteredEmbeddingExchange::Fake(exchange) => self.fake.exchange(exchange).await,
            RegisteredEmbeddingExchange::AgentPlan(exchange) => {
                self.agent_plan.exchange(exchange).await
            }
        }
    }
}

#[cfg(test)]
#[path = "registered_embeddings_tests.rs"]
mod tests;
