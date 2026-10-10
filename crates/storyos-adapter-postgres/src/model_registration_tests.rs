use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AgentRunWorkStore, CompleteAgentRun, DestinationRequest, ModelProviderAdapter, ModelStreamSink,
    NoContractFaults, Observation, PreDispatchRefusal, PreparedRequest, ProjectScope,
    complete_agent_run,
};
use storyos_core::{AGENT_PLAN_REGISTRATION, ModelAdapter};
use tokio_postgres::Client;
use uuid::Uuid;

use super::insert_model_registration;
use crate::agent_run_dispatch::tests::{
    DispatchEvidence, ProjectBinding, dispatch_evidence, queued_run, stores,
};

/// Makes the Agent Plan binding current for `scope`, as a deployment that offers it does.
pub(crate) async fn bind_agent_plan(admin: &Client, scope: &ProjectScope) {
    insert_model_registration(admin, &AGENT_PLAN_REGISTRATION)
        .await
        .unwrap();
    let [identity, grant, binding, decision] = [(); 4].map(|()| Uuid::now_v7().to_string());
    admin
        .batch_execute(&format!(
            "BEGIN;
             INSERT INTO storyos.processing_destination_identities
               (owner_user_id, project_id, processing_destination_identity, destination_kind)
             VALUES ('{owner}', '{project}', '{identity}', 'volcengine_agent_plan');
             INSERT INTO storyos.processing_destination_identity_evidence_revisions
               (owner_user_id, project_id, processing_destination_identity, evidence_revision,
                evidence_kind)
             VALUES ('{owner}', '{project}', '{identity}', 1, 'volcengine_agent_plan_boundary');
             INSERT INTO storyos.project_destination_grants
               (owner_user_id, project_id, grant_id, processing_destination_identity, grant_kind)
             VALUES ('{owner}', '{project}', '{grant}', '{identity}', 'volcengine_agent_plan_use');
             INSERT INTO storyos.project_external_use_binding_revisions
               (owner_user_id, project_id, project_model_use_binding_revision,
                processing_destination_identity, evidence_revision, grant_id,
                model_registration_revision)
             VALUES ('{owner}', '{project}', '{binding}', '{identity}', 1, '{grant}',
                     '{registration}');
             INSERT INTO storyos.external_contract_compatibility_decisions
               (owner_user_id, project_id, external_compatibility_decision,
                project_model_use_binding_revision, decision_kind)
             VALUES ('{owner}', '{project}', '{decision}', '{binding}',
                     'volcengine_agent_plan_compatible');
             INSERT INTO storyos.project_policy_revisions
               (owner_user_id, project_id, policy_revision, availability, receipt_id,
                external_compatibility_decision)
             SELECT owner_user_id, project_id, 2, 'available', receipt_id, '{decision}'
               FROM storyos.project_policy_revisions
              WHERE (owner_user_id, project_id, policy_revision) = ('{owner}', '{project}', 1);
             COMMIT;",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            registration = AGENT_PLAN_REGISTRATION.revision,
        ))
        .await
        .unwrap();
}

/// An adapter of the Agent Plan Registration that admission must never reach.
struct UnreachableAgentPlan;

impl ModelProviderAdapter for UnreachableAgentPlan {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = ();

    async fn prepare(
        &self,
        _request: &DestinationRequest,
    ) -> Result<PreparedRequest<()>, PreDispatchRefusal> {
        panic!("Create admission must refuse a pending profile before preparation");
    }

    async fn exchange(&self, _prepared: (), _sink: &mut impl ModelStreamSink) -> Observation {
        panic!("Create admission must refuse a pending profile before the exchange");
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn fake_worker_skips_an_agent_plan_run_that_create_admission_refuses() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (_, agent_plan_run) = queued_run(&store, &admin, "0a1", ProjectBinding::AgentPlan).await;
    let (_, fake_run) = queued_run(&store, &admin, "0a2", ProjectBinding::HostFake).await;

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
                r#"{"kind": "execution_refused", "capability": "model_runtime_qualification_pending"}"#
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
