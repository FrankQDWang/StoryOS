//! The shared test support from `runtime_qualification_tests.rs`.

use crate::shared_tests::agent_run_finish_rule::finish_before;
use crate::shared_tests::model_gateway::RuntimeClock;
use crate::shared_tests::model_gateway::claim_run;
use std::sync::Mutex;
use storyos_application::{
    CreatePurpose, CreateRequest, DestinationRequest, ModelProviderAdapter, ModelStreamSink,
    NoContractFaults, Observation, PreDispatchRefusal, PreparedRequest, ProjectScope,
    ToolProbeRequest, WirePayloadProjection, complete_agent_run,
};
use storyos_core::{DeploymentDestination, ModelAdapter, ToolChoice};
use tokio_postgres::Client;

pub(crate) const AUTHORIZATION: &str =
    "https://github.com/FrankQDWang/StoryOS/issues/393#issuecomment-6098554693";

pub(crate) fn authorized() -> DeploymentDestination {
    DeploymentDestination::VolcengineAgentPlan {
        qualification_authorization: Some(AUTHORIZATION.to_owned()),
    }
}

/// An Agent Plan adapter that records each Create request and reports one scripted observation.
pub(crate) struct ScriptedAgentPlan {
    observation: Observation,
    pub(crate) seen: Mutex<Vec<CreateRequest>>,
}

impl ScriptedAgentPlan {
    pub(crate) fn new(observation: Observation) -> Self {
        Self {
            observation,
            seen: Mutex::default(),
        }
    }
}

/// The scripted observation answers the closing request. A request before a `finish` call that
/// counted gets the `finish` call first.
impl ModelProviderAdapter for ScriptedAgentPlan {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = ToolChoice;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<ToolChoice>, PreDispatchRefusal> {
        let DestinationRequest::Create(create) = request else {
            return Err(PreDispatchRefusal::UnsupportedRequest);
        };
        // As the real adapter does, only the current mapping prepares a request (ADR 0097).
        if create.route.mapping_revision != storyos_core::AGENT_PLAN_RUN_PLAN_MAPPING_REVISION {
            return Err(PreDispatchRefusal::UnsupportedRequest);
        }
        self.seen.lock().unwrap().push(create.clone());
        // As the real adapter does, a function output probe without a call ID has no body, so it
        // refuses before dispatch (ADR 0049). The test still reads the request that the Host gave.
        if let CreatePurpose::ToolQualification(ToolProbeRequest::FunctionCallOutput {
            call_id: None,
        }) = create.purpose
        {
            return Err(PreDispatchRefusal::UnsupportedRequest);
        }
        Ok(PreparedRequest {
            projection: WirePayloadProjection {
                execution_profile: "storyos.volcengine-agent-plan.execution.v1".to_owned(),
                mapping_revision: create.route.mapping_revision.clone(),
                digest: "sha256:scripted".to_owned(),
                serialized_payload: None,
                cache_setting: None,
            },
            prepared: create.tool_choice,
        })
    }

    async fn exchange(
        &self,
        tool_choice: ToolChoice,
        _sink: &mut impl ModelStreamSink,
    ) -> Observation {
        match (&self.observation, tool_choice) {
            (Observation::Terminal(response), ToolChoice::Required) => {
                Observation::Terminal(finish_before(response))
            }
            (observation, _) => observation.clone(),
        }
    }
}

pub(crate) async fn settle(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    run_id: &str,
    adapter: &ScriptedAgentPlan,
) {
    let claim = claim_run(admin, scope.clone(), run_id.to_owned()).await;
    complete_agent_run(store, adapter, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
}
