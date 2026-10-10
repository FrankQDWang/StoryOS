use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    NoContractFaults, ProjectAssistanceRecord, ProjectScope, complete_agent_run,
    open_project_assistance,
};
use storyos_core::{
    AGENT_PLAN_REGISTRATION, AssistanceAvailability, DeploymentDestination, DestinationKind,
    RuntimeQualification,
};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::{admit_run, claim_run, queued_run, stores};
use crate::model_registration::tests::agent_plan;

/// The non-secret binding facts that the Agent Plan binding of a Project records.
#[derive(Debug, PartialEq)]
struct BindingFacts {
    endpoint: Option<String>,
    account_boundary: Option<String>,
    eligibility_evidence: Option<String>,
    credential_source: String,
    credential_reference: Option<String>,
    budget_bounds: String,
    runtime_qualification: String,
}

async fn binding_facts(admin: &Client, decision: &str) -> BindingFacts {
    let row = admin
        .query_one(
            "SELECT evidence.endpoint, evidence.account_boundary, evidence.eligibility_evidence,
                    binding.credential_source, binding.credential_reference,
                    binding.budget_bounds, decision.runtime_qualification
               FROM storyos.external_contract_compatibility_decisions AS decision
               JOIN storyos.project_external_use_binding_revisions AS binding
                 USING (owner_user_id, project_id, project_model_use_binding_revision)
               JOIN storyos.processing_destination_identity_evidence_revisions AS evidence
                 USING (owner_user_id, project_id, processing_destination_identity,
                        evidence_revision)
              WHERE decision.external_compatibility_decision = $1::text::uuid",
            &[&decision],
        )
        .await
        .unwrap();
    BindingFacts {
        endpoint: row.get(/*idx*/ 0),
        account_boundary: row.get(/*idx*/ 1),
        eligibility_evidence: row.get(/*idx*/ 2),
        credential_source: row.get(/*idx*/ 3),
        credential_reference: row.get(/*idx*/ 4),
        budget_bounds: row.get(/*idx*/ 5),
        runtime_qualification: row.get(/*idx*/ 6),
    }
}

/// Claims and settles the run, and returns its recorded refusal.
async fn refusal(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Option<String> {
    let claim = claim_run(admin, scope.clone(), run_id.to_owned()).await;
    // The fake adapter is never reached: each of these runs is refused before preparation.
    complete_agent_run(store, &FakeDestination, &NoContractFaults, &claim)
        .await
        .unwrap();
    admin
        .query_one(
            "SELECT settlement->>'capability' FROM storyos.agent_runs
              WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_new_deployment_destination_rebinds_the_project_and_refuses_stale_and_drifted_runs() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, fake_run) =
        queued_run(&store, "e7a", DeploymentDestination::HostFake).await;
    let fake_binding = open_project_assistance(&store, &scope)
        .await
        .unwrap()
        .unwrap();

    let agent_plan_run = admit_run(&store, &scope, &chapter_id, "e7b", agent_plan()).await;
    let current = open_project_assistance(&store, &scope)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        current,
        ProjectAssistanceRecord {
            availability: AssistanceAvailability::Available,
            revision: 2,
            model_registration_revision: AGENT_PLAN_REGISTRATION.revision.to_owned(),
            destination: DestinationKind::VolcengineAgentPlan,
            runtime_qualification: RuntimeQualification::Pending,
            ..current.clone()
        }
    );
    assert_ne!(
        (
            &current.processing_destination_identity,
            &current.grant_id,
            &current.project_model_use_binding_revision,
            &current.external_compatibility_decision,
        ),
        (
            &fake_binding.processing_destination_identity,
            &fake_binding.grant_id,
            &fake_binding.project_model_use_binding_revision,
            &fake_binding.external_compatibility_decision,
        )
    );
    assert_eq!(
        binding_facts(&admin, &current.external_compatibility_decision).await,
        BindingFacts {
            endpoint: Some("https://ark.cn-beijing.volces.com/api/plan/v3".to_owned()),
            account_boundary: Some(storyos_core::AGENT_PLAN_ACCOUNT_BOUNDARY.to_owned()),
            eligibility_evidence: Some(storyos_core::AGENT_PLAN_ELIGIBILITY_EVIDENCE.to_owned()),
            credential_source: "operator_quota".to_owned(),
            credential_reference: Some(
                "macos-keychain:storyos-volcengine-agent-plan/frankqdwang".to_owned()
            ),
            budget_bounds: "unqualified".to_owned(),
            runtime_qualification: "pending".to_owned(),
        }
    );
    assert_eq!(
        binding_facts(&admin, &fake_binding.external_compatibility_decision).await,
        BindingFacts {
            endpoint: None,
            account_boundary: None,
            eligibility_evidence: None,
            credential_source: "none".to_owned(),
            credential_reference: None,
            budget_bounds: "not_required".to_owned(),
            runtime_qualification: "qualified".to_owned(),
        }
    );

    let stale = refusal(&store, &admin, &scope, &fake_run).await;
    admin
        .batch_execute(&format!(
            "INSERT INTO storyos.model_registration_revisions
               (model_registration_revision, model_kind, api_surface, provider_model_id,
                capability_profile_revision)
             SELECT '018f0000-0000-7001-8000-0000000b2fa0', model_kind, api_surface,
                    provider_model_id, capability_profile_revision
               FROM storyos.model_registration_revisions
              WHERE model_registration_revision = '{agent_plan}';
             UPDATE storyos.model_registration_heads
                SET model_registration_revision = '018f0000-0000-7001-8000-0000000b2fa0'
              WHERE model_kind = 'volcengine_agent_plan_responses';",
            agent_plan = AGENT_PLAN_REGISTRATION.revision,
        ))
        .await
        .unwrap();
    let drifted = refusal(&store, &admin, &scope, &agent_plan_run).await;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.model_registration_heads
                SET model_registration_revision = '{agent_plan}'
              WHERE model_kind = 'volcengine_agent_plan_responses'",
            agent_plan = AGENT_PLAN_REGISTRATION.revision,
        ))
        .await
        .unwrap();

    assert_eq!(
        (stale.as_deref(), drifted.as_deref()),
        (
            Some("model_use_binding_stale"),
            Some("model_registration_drift")
        )
    );
}
