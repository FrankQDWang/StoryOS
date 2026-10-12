use storyos_adapter_fake_destination::{FakeDestination, FakeExchange};
use storyos_adapter_volcengine_responses::{AgentPlanExchange, AgentPlanResponses};
use storyos_application::{
    CredentialResolver, DestinationRequest, ModelProviderAdapter, ModelStreamSink, Observation,
    PreDispatchRefusal, PreparedRequest,
};
use storyos_core::ModelAdapter;

/// Serves each request with the adapter that the Model Registration of its AgentRun binds.
/// It never uses the fake adapter in place of another adapter (ADR 0039).
pub(crate) struct RegisteredAdapters<R> {
    pub(crate) fake: FakeDestination,
    pub(crate) agent_plan: AgentPlanResponses<R>,
}

pub(crate) enum RegisteredExchange {
    Fake(FakeExchange),
    AgentPlan(AgentPlanExchange),
}

impl<R: CredentialResolver> ModelProviderAdapter for RegisteredAdapters<R> {
    const ADAPTERS: &'static [ModelAdapter] = &[
        ModelAdapter::HostFake,
        ModelAdapter::VolcengineAgentPlanResponses,
    ];
    type Prepared = RegisteredExchange;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<RegisteredExchange>, PreDispatchRefusal> {
        match request.route().adapter {
            ModelAdapter::HostFake => {
                let prepared = self.fake.prepare(request).await?;
                Ok(PreparedRequest {
                    projection: prepared.projection,
                    prepared: RegisteredExchange::Fake(prepared.prepared),
                })
            }
            ModelAdapter::VolcengineAgentPlanResponses => {
                let prepared = self.agent_plan.prepare(request).await?;
                Ok(PreparedRequest {
                    projection: prepared.projection,
                    prepared: RegisteredExchange::AgentPlan(prepared.prepared),
                })
            }
        }
    }

    async fn exchange(
        &self,
        prepared: RegisteredExchange,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        match prepared {
            RegisteredExchange::Fake(exchange) => self.fake.exchange(exchange, sink).await,
            RegisteredExchange::AgentPlan(exchange) => {
                self.agent_plan.exchange(exchange, sink).await
            }
        }
    }
}

#[cfg(test)]
#[path = "registered_adapters_tests.rs"]
mod tests;
