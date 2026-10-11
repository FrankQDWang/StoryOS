use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AgentRunWorkStore, CompleteAgentRun, DestinationRequest, ModelProviderAdapter, ModelStreamSink,
    NoContractFaults, Observation, PreDispatchRefusal, PreparedRequest, complete_agent_run,
};
use storyos_core::{DeploymentDestination, ModelAdapter};

use crate::agent_run_dispatch::tests::{DispatchEvidence, dispatch_evidence, queued_run, stores};

pub(crate) fn agent_plan() -> DeploymentDestination {
    DeploymentDestination::VolcengineAgentPlan {
        qualification_authorization: None,
    }
}

/// An adapter of the Agent Plan Registration whose credential is unavailable.
struct UnreachableAgentPlan;

impl ModelProviderAdapter for UnreachableAgentPlan {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = ();

    async fn prepare(
        &self,
        _request: &DestinationRequest,
    ) -> Result<PreparedRequest<()>, PreDispatchRefusal> {
        Err(PreDispatchRefusal::CredentialUnavailable)
    }

    async fn exchange(&self, _prepared: (), _sink: &mut impl ModelStreamSink) -> Observation {
        panic!("a refused preparation sends nothing");
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn fake_worker_skips_an_agent_plan_run_that_only_the_agent_plan_worker_claims() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (_, _, agent_plan_run) = queued_run(&store, "0a1", agent_plan()).await;
    let (_, _, fake_run) = queued_run(&store, "0a2", DeploymentDestination::HostFake).await;

    let fake_claim = store
        .claim_next_agent_run(FakeDestination::ADAPTERS)
        .await
        .unwrap()
        .unwrap();
    let agent_plan_claim = store
        .claim_next_agent_run(UnreachableAgentPlan::ADAPTERS)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (&fake_claim.run_id, &agent_plan_claim.run_id),
        (&fake_run, &agent_plan_run)
    );

    let refused = complete_agent_run(
        &store,
        &UnreachableAgentPlan,
        &NoContractFaults,
        &agent_plan_claim,
    )
    .await;
    complete_agent_run(&store, &FakeDestination, &NoContractFaults, &fake_claim)
        .await
        .unwrap();

    assert_eq!(refused.ok(), Some(CompleteAgentRun::Settled));
    assert_eq!(
        dispatch_evidence(&admin, &agent_plan_run).await,
        DispatchEvidence {
            status: "refused".to_owned(),
            settlement: Some(
                r#"{"kind": "execution_refused", "capability": "destination_credential_unavailable"}"#
                    .to_owned()
            ),
            model_attempts: 0,
            disclosure_events: 0,
            destination_manifests: 0,
            items: None,
            decision_id: None,
        }
    );
}
