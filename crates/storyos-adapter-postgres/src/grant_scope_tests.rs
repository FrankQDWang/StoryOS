use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    DestinationRequest, ModelProviderAdapter, ModelStreamSink, NoContractFaults, Observation,
    PreDispatchRefusal, PreparedRequest, ProjectScope, complete_agent_run, open_project_assistance,
};
use storyos_core::DeploymentDestination;
use tokio_postgres::Client;

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::grant_scope::current_grant;
use crate::shared_tests::model_gateway::RuntimeClock;
use crate::shared_tests::model_gateway::{
    DispatchEvidence, admit_run, claim_run, dispatch_evidence, queued_run, stores,
};

const OUTSIDE_GRANT: &str =
    r#"{"kind": "execution_refused", "capability": "disclosure_outside_grant"}"#;

/// The dispatch evidence of a request that refused before the dispatch claim.
fn refused_before_claim() -> DispatchEvidence {
    DispatchEvidence {
        status: "refused".to_owned(),
        settlement: Some(OUTSIDE_GRANT.to_owned()),
        model_attempts: 0,
        disclosure_events: 0,
        destination_manifests: 0,
        items: None,
        decision_id: None,
    }
}

/// Claims and settles `run_id` with `adapter`, and returns its dispatch evidence.
async fn settle(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    adapter: &impl ModelProviderAdapter,
    scope: &ProjectScope,
    run_id: &str,
) -> DispatchEvidence {
    let claim = claim_run(admin, scope.clone(), run_id.to_owned()).await;
    complete_agent_run(store, adapter, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    dispatch_evidence(admin, run_id).await
}

/// The status of a run, the grant evidence of its first decision Model Attempt, and the grant that
/// its first decision manifest names.
async fn grant_evidence(
    admin: &Client,
    run_id: &str,
) -> (String, Option<serde_json::Value>, serde_json::Value) {
    let row = admin
        .query_one(
            "SELECT run.status,
                    (SELECT (attempt.payload->'disclosure_grant')::text
                       FROM storyos.model_attempts AS attempt
                      WHERE attempt.run_id = run.run_id AND attempt.attempt_role = 'decision'
                        AND attempt.decision_position = 0),
                    (SELECT jsonb_build_object('grant_id', manifest.disclosure_grant_id,
                                               'grant_scope', manifest.disclosure_grant_scope)::text
                       FROM storyos.context_assembly_manifests AS manifest
                      WHERE manifest.run_id = run.run_id AND manifest.manifest_role = 'decision'
                        AND manifest.decision_position = 0)
               FROM storyos.agent_runs AS run
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    let grant: Option<String> = row.get(/*idx*/ 1);
    (
        row.get(/*idx*/ 0),
        grant.map(|text| serde_json::from_str(&text).unwrap()),
        serde_json::from_str(&row.get::<_, String>(/*idx*/ 2)).unwrap(),
    )
}

/// Removes the scope of the current grant, as a row from before migration 0090 has none.
async fn remove_grant_scope(admin: &Client, grant_id: &str) {
    admin
        .execute(
            "UPDATE storyos.project_destination_grants SET grant_scope = NULL
              WHERE grant_id = $1::text::uuid",
            &[&grant_id],
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_ordinary_request_dispatches_with_no_prompt_and_records_its_grant_scope() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, run_id) =
        queued_run(&store, "d5a", DeploymentDestination::HostFakeToolLoop).await;
    let (grant_id, grant_scope) = current_grant(&store, &admin, &scope).await;

    let evidence = settle(&store, &admin, &FakeDestination, &scope, &run_id).await;

    assert_eq!(
        grant_scope.as_deref(),
        Some("project_writing_assistance_v5")
    );
    // The `finish` RunStep and the closing RunStep.
    assert_eq!(
        (evidence.model_attempts, evidence.disclosure_events),
        (2, 2)
    );
    let admitted = serde_json::json!({
        "grant_id": grant_id,
        "grant_scope": "project_writing_assistance_v5"
    });
    assert_eq!(
        grant_evidence(&admin, &run_id).await,
        ("completed".to_owned(), Some(admitted.clone()), admitted)
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_grant_without_a_scope_refuses_before_dispatch_and_the_next_run_rebinds() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, unscoped_run) =
        queued_run(&store, "d5b", DeploymentDestination::HostFakeToolLoop).await;
    let pinned_run = admit_run(
        &store,
        &scope,
        &chapter_id,
        "d5c",
        DeploymentDestination::HostFakeToolLoop,
        "Help with this passage.",
    )
    .await;
    let (old_grant, _) = current_grant(&store, &admin, &scope).await;
    remove_grant_scope(&admin, &old_grant).await;

    let unscoped = settle(&store, &admin, &FakeDestination, &scope, &unscoped_run).await;
    let rebound_run = admit_run(
        &store,
        &scope,
        &chapter_id,
        "d5d",
        DeploymentDestination::HostFakeToolLoop,
        "Help with this passage.",
    )
    .await;
    let (new_grant, new_scope) = current_grant(&store, &admin, &scope).await;
    let pinned = settle(&store, &admin, &FakeDestination, &scope, &pinned_run).await;
    settle(&store, &admin, &FakeDestination, &scope, &rebound_run).await;

    assert_eq!(unscoped, refused_before_claim());
    assert_eq!(
        pinned,
        DispatchEvidence {
            settlement: Some(
                r#"{"kind": "execution_refused", "capability": "model_use_binding_stale"}"#
                    .to_owned()
            ),
            ..refused_before_claim()
        }
    );
    assert_ne!(new_grant, old_grant);
    assert_eq!(new_scope.as_deref(), Some("project_writing_assistance_v5"));
    let admitted = serde_json::json!({
        "grant_id": new_grant,
        "grant_scope": "project_writing_assistance_v5"
    });
    assert_eq!(
        grant_evidence(&admin, &rebound_run).await,
        ("completed".to_owned(), Some(admitted.clone()), admitted)
    );
}

/// The identity, grant, binding, and Decision rows that `record` names, as JSON.
async fn binding_rows(
    admin: &Client,
    record: &storyos_application::ProjectAssistanceRecord,
) -> serde_json::Value {
    let text: String = admin
        .query_one(
            "SELECT jsonb_build_array(
                      (SELECT to_jsonb(row) FROM storyos.processing_destination_identities AS row
                        WHERE row.processing_destination_identity = $1::text::uuid),
                      (SELECT to_jsonb(row) FROM storyos.project_destination_grants AS row
                        WHERE row.grant_id = $2::text::uuid),
                      (SELECT to_jsonb(row) FROM storyos.project_external_use_binding_revisions
                         AS row WHERE row.project_model_use_binding_revision = $3::text::uuid),
                      (SELECT to_jsonb(row)
                         FROM storyos.external_contract_compatibility_decisions AS row
                        WHERE row.external_compatibility_decision = $4::text::uuid))::text",
            &[
                &record.processing_destination_identity,
                &record.grant_id,
                &record.project_model_use_binding_revision,
                &record.external_compatibility_decision,
            ],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    serde_json::from_str(&text).unwrap()
}

/// ADR 0094, ADR 0095, and ADR 0051: a grant with the fourth scope gets new binding records with
/// the fifth scope at the next `createAgentRun`, with no action of the author. The grant of
/// another Project does not change.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_grant_with_the_fourth_scope_gets_new_records_with_the_fifth_scope_at_the_next_run() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, pinned_run) =
        queued_run(&store, "b01", DeploymentDestination::HostFakeToolLoop).await;
    let (other_scope, _, other_run) =
        queued_run(&store, "b07", DeploymentDestination::HostFakeToolLoop).await;
    let mut earlier_records = Vec::new();
    for scope in [&scope, &other_scope] {
        let earlier = open_project_assistance(&store, scope)
            .await
            .unwrap()
            .unwrap();
        admin
            .execute(
                "UPDATE storyos.project_destination_grants
                    SET grant_scope = 'project_writing_assistance_v4'
                  WHERE grant_id = $1::text::uuid",
                &[&earlier.grant_id],
            )
            .await
            .unwrap();
        earlier_records.push(earlier);
    }
    let [earlier, other] = <[_; 2]>::try_from(earlier_records).unwrap();
    let earlier_rows = binding_rows(&admin, &earlier).await;
    let other_rows = binding_rows(&admin, &other).await;

    let rebound_run = admit_run(
        &store,
        &scope,
        &chapter_id,
        "b02",
        DeploymentDestination::HostFakeToolLoop,
        "Help with this passage.",
    )
    .await;
    let current = open_project_assistance(&store, &scope)
        .await
        .unwrap()
        .unwrap();
    let pinned: (String, String) = {
        let row = admin
            .query_one(
                "SELECT grant_id::text, project_model_use_binding_revision::text
                   FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
                &[&rebound_run],
            )
            .await
            .unwrap();
        (row.get(/*idx*/ 0), row.get(/*idx*/ 1))
    };
    let stale = settle(&store, &admin, &FakeDestination, &scope, &pinned_run).await;
    settle(&store, &admin, &FakeDestination, &scope, &rebound_run).await;
    let other_current = open_project_assistance(&store, &other_scope)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        (
            current_grant(&store, &admin, &scope).await.1,
            pinned,
            binding_rows(&admin, &earlier).await,
            stale.settlement,
            (other_current, binding_rows(&admin, &other).await),
        ),
        (
            Some("project_writing_assistance_v5".to_owned()),
            (
                current.grant_id.clone(),
                current.project_model_use_binding_revision.clone()
            ),
            earlier_rows,
            Some(
                r#"{"kind": "execution_refused", "capability": "model_use_binding_stale"}"#
                    .to_owned()
            ),
            (other.clone(), other_rows),
        )
    );
    assert!(
        [
            current.processing_destination_identity != earlier.processing_destination_identity,
            current.grant_id != earlier.grant_id,
            current.project_model_use_binding_revision
                != earlier.project_model_use_binding_revision,
            current.external_compatibility_decision != earlier.external_compatibility_decision,
        ]
        .iter()
        .all(|changed| *changed)
    );
    let admitted = serde_json::json!({
        "grant_id": current.grant_id,
        "grant_scope": "project_writing_assistance_v5"
    });
    assert_eq!(
        grant_evidence(&admin, &rebound_run).await,
        ("completed".to_owned(), Some(admitted.clone()), admitted)
    );
    // The HTTP phases drain the queued Runs of the shared database, so the test parks its Run.
    admin
        .execute(
            "UPDATE storyos.agent_runs
                SET status = 'paused', lease_expires_at = NULL, wakeup_pending = false
              WHERE run_id = $1::text::uuid AND status = 'queued'",
            &[&other_run],
        )
        .await
        .unwrap();
}

/// Changes the pinned grant when the request of `stage` is prepared, after its admission and
/// before its dispatch claim.
struct ChangeGrantAtPrepare<'a> {
    admin: &'a Client,
    statement: &'static str,
    grant_id: String,
    stage: Stage,
}

#[derive(PartialEq)]
enum Stage {
    Create,
    Retrieve,
}

impl ModelProviderAdapter for ChangeGrantAtPrepare<'_> {
    const ADAPTERS: &'static [storyos_core::ModelAdapter] = FakeDestination::ADAPTERS;
    type Prepared = <FakeDestination as ModelProviderAdapter>::Prepared;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal> {
        let stage = match request {
            DestinationRequest::Create(_) => Some(Stage::Create),
            DestinationRequest::Retrieve(_) => Some(Stage::Retrieve),
            DestinationRequest::Abort(_) => None,
        };
        if stage.as_ref() == Some(&self.stage) {
            self.admin
                .batch_execute(&self.statement.replace("{grant_id}", &self.grant_id))
                .await
                .unwrap();
        }
        FakeDestination.prepare(request).await
    }

    async fn exchange(
        &self,
        prepared: Self::Prepared,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        FakeDestination.exchange(prepared, sink).await
    }
}

const REMOVE_SCOPE: &str = "UPDATE storyos.project_destination_grants SET grant_scope = NULL
  WHERE grant_id = '{grant_id}'";

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_dispatch_claim_checks_the_current_grant_again_and_sends_nothing() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let changes = [
        REMOVE_SCOPE,
        // A second identity of the same Project that the use binding does not name.
        "WITH grant_row AS (
           SELECT owner_user_id, project_id FROM storyos.project_destination_grants
            WHERE grant_id = '{grant_id}'
         ), identity AS (
           INSERT INTO storyos.processing_destination_identities
             (owner_user_id, project_id, processing_destination_identity, destination_kind,
              control_classification)
           SELECT owner_user_id, project_id, gen_random_uuid(), 'host_fake', 'host_internal'
             FROM grant_row
           RETURNING processing_destination_identity
         )
         UPDATE storyos.project_destination_grants
            SET processing_destination_identity = (SELECT processing_destination_identity
                                                     FROM identity)
          WHERE grant_id = '{grant_id}'",
    ];
    let mut observed = Vec::new();
    let mut changed = Vec::new();
    for (prefix, statement) in ["d5e", "d6a"].into_iter().zip(changes) {
        let (scope, chapter_id, run_id) =
            queued_run(&store, prefix, DeploymentDestination::HostFakeToolLoop).await;
        let adapter = ChangeGrantAtPrepare {
            admin: &admin,
            statement,
            grant_id: current_grant(&store, &admin, &scope).await.0,
            stage: Stage::Create,
        };
        observed.push(settle(&store, &admin, &adapter, &scope, &run_id).await);
        changed.push((scope, chapter_id, adapter.grant_id));
    }
    // The next createAgentRun replaces a binding whose grant names another identity.
    let (scope, chapter_id, mismatched_grant) = &changed[1];
    let rebound_run = admit_run(
        &store,
        scope,
        chapter_id,
        "d6f",
        DeploymentDestination::HostFakeToolLoop,
        "Help with this passage.",
    )
    .await;
    let rebound = settle(&store, &admin, &FakeDestination, scope, &rebound_run).await;

    assert_eq!(observed, [refused_before_claim(), refused_before_claim()]);
    assert_ne!(
        &current_grant(&store, &admin, scope).await.0,
        mismatched_grant
    );
    assert_eq!(
        (rebound.status.as_str(), rebound.model_attempts),
        ("completed", 2)
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_project_dispatches_only_under_its_own_grant() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (granted_scope, _, granted_run) =
        queued_run(&store, "d6b", DeploymentDestination::HostFakeToolLoop).await;
    let (other_scope, _, other_run) =
        queued_run(&store, "d6c", DeploymentDestination::HostFakeToolLoop).await;
    remove_grant_scope(&admin, &current_grant(&store, &admin, &other_scope).await.0).await;

    let other = settle(&store, &admin, &FakeDestination, &other_scope, &other_run).await;
    let granted = settle(
        &store,
        &admin,
        &FakeDestination,
        &granted_scope,
        &granted_run,
    )
    .await;

    assert_eq!(other, refused_before_claim());
    assert_eq!(
        (granted.status.as_str(), granted.model_attempts),
        ("completed", 2)
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_retrieve_after_an_unknown_create_checks_the_current_grant_and_sends_nothing() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, _) =
        queued_run(&store, "d6d", DeploymentDestination::HostFakeToolLoop).await;
    let run_id = admit_run(
        &store,
        &scope,
        &chapter_id,
        "d6e",
        DeploymentDestination::HostFakeToolLoop,
        "SCRIPT:retrieve-complete",
    )
    .await;
    let adapter = ChangeGrantAtPrepare {
        admin: &admin,
        statement: REMOVE_SCOPE,
        grant_id: current_grant(&store, &admin, &scope).await.0,
        stage: Stage::Retrieve,
    };

    let evidence = settle(&store, &admin, &adapter, &scope, &run_id).await;
    let row = admin
        .query_one(
            "SELECT attempt.payload->'original_result_retrieval'->>'disposition',
                    attempt.payload->'original_result_retrieval'->>'keep_reason',
                    (SELECT count(*) FROM storyos.model_attempts AS retrieval
                      WHERE retrieval.run_id = attempt.run_id
                        AND retrieval.attempt_role = 'retrieval')
               FROM storyos.model_attempts AS attempt
              WHERE attempt.run_id = $1::text::uuid AND attempt.attempt_role = 'decision'",
            &[&run_id],
        )
        .await
        .unwrap();

    assert_eq!(
        (
            evidence.status.as_str(),
            evidence.model_attempts,
            evidence.disclosure_events,
        ),
        ("waiting", 1, 1)
    );
    assert_eq!(
        (
            row.get::<_, Option<String>>(/*idx*/ 0),
            row.get::<_, Option<String>>(/*idx*/ 1),
            row.get::<_, i64>(/*idx*/ 2),
        ),
        (
            Some("kept_unknown".to_owned()),
            Some("destination_mismatch".to_owned()),
            0
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_recorded_purpose_outside_the_scope_refuses_before_dispatch() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, run_id) =
        queued_run(&store, "d7a", DeploymentDestination::HostFakeToolLoop).await;
    admin
        .execute(
            "UPDATE storyos.context_assembly_manifests
                SET payload = jsonb_set(payload, '{operation_requirement,purpose}', '\"research\"')
              WHERE run_id = $1::text::uuid AND manifest_role = 'decision'",
            &[&run_id],
        )
        .await
        .unwrap();

    let evidence = settle(&store, &admin, &FakeDestination, &scope, &run_id).await;

    assert_eq!(evidence, refused_before_claim());
}
