//! The embedding grant and use binding of `createAgentRun` (ADR 0095 section 3). Test prefix lead
//! `ab`.

use serde_json::{Value, json};
use storyos_application::ProjectScope;
use storyos_core::{
    DeploymentDestination, EmbeddingRegistration, HOST_FAKE_EMBEDDING_REGISTRATION,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{admit_embedding_run, queued_run_offering, stores};
use crate::project_research_binding::tests::park_queued_runs;

pub(crate) const FAKE: Option<&EmbeddingRegistration> = Some(&HOST_FAKE_EMBEDDING_REGISTRATION);

/// The embedding grant and use binding rows of the Project.
async fn embedding_records(admin: &Client, scope: &ProjectScope) -> Value {
    let row = admin
        .query_one(
            "SELECT COALESCE(jsonb_agg(jsonb_build_array(
                      grant_row.grant_id, grant_row.grant_scope, identity.destination_kind,
                      binding.embedding_registration_revision, binding.credential_source,
                      binding.budget_bounds) ORDER BY grant_row.grant_id), '[]'::jsonb)::text
               FROM storyos.project_destination_grants AS grant_row
               JOIN storyos.processing_destination_identities AS identity
                 USING (owner_user_id, project_id, processing_destination_identity)
               JOIN storyos.project_external_use_binding_revisions AS binding
                 USING (owner_user_id, project_id, grant_id)
              WHERE grant_row.project_id = $1::text::uuid
                AND grant_row.grant_kind = 'manuscript_embedding_use'",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}

/// A new Project with one Chapter, one paused Run, and one queued Run of a deployment with the
/// embedding configuration `embedding`. Returns the scope, the Chapter, and the queued Run.
pub(crate) async fn embedding_project(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    embedding: Option<&'static EmbeddingRegistration>,
) -> (ProjectScope, String, String) {
    let (scope, chapter_id, _) = queued_run_offering(
        store,
        prefixes[0],
        DeploymentDestination::HostFakeToolLoop,
        None,
    )
    .await;
    park_queued_runs(admin, &scope).await;
    let run_id = admit_embedding_run(store, &scope, &chapter_id, prefixes[1], embedding).await;
    (scope, chapter_id, run_id)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn create_agent_run_binds_embedding_only_with_a_configuration_and_in_its_project() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (first, chapter_id, _) = embedding_project(&store, &admin, ["ab1", "ab2"], None).await;
    let unconfigured = embedding_records(&admin, &first).await;
    admit_embedding_run(&store, &first, &chapter_id, "ab3", FAKE).await;
    let bound = embedding_records(&admin, &first).await;
    admit_embedding_run(&store, &first, &chapter_id, "ab4", FAKE).await;
    let again = embedding_records(&admin, &first).await;
    let (second, ..) = embedding_project(&store, &admin, ["ab5", "ab6"], FAKE).await;
    park_queued_runs(&admin, &first).await;
    park_queued_runs(&admin, &second).await;

    assert_eq!(unconfigured, json!([]));
    let grant_id = bound[0][0].clone();
    assert_eq!(
        bound,
        json!([[
            grant_id,
            "manuscript_embedding_v1",
            "host_fake",
            HOST_FAKE_EMBEDDING_REGISTRATION.revision,
            "none",
            "embedding_request_v1"
        ]])
    );
    // The current records stay, and the grant of the other Project does not change them.
    assert_eq!(again, bound);
    assert_eq!(embedding_records(&admin, &first).await, bound);
    let other = embedding_records(&admin, &second).await;
    assert_eq!(
        (other.as_array().unwrap().len(), other[0][1].clone()),
        (1, json!("manuscript_embedding_v1"))
    );
    assert_ne!(other[0][0], grant_id);
}
