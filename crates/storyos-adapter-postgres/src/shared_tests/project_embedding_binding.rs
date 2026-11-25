//! The shared test support from `project_embedding_binding_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::model_gateway::{Admission, queued_run_offering};
use crate::shared_tests::project_research_binding::park_queued_runs;
use crate::statement_cache::Client;
use storyos_application::ProjectScope;
use storyos_core::{
    DeploymentDestination, EmbeddingRegistration, HOST_FAKE_EMBEDDING_REGISTRATION,
};

pub(crate) const FAKE: Option<&EmbeddingRegistration> = Some(&HOST_FAKE_EMBEDDING_REGISTRATION);

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
    let run_id = Admission {
        embedding,
        ..Default::default()
    }
    .admit(store, &scope, &chapter_id, prefixes[1])
    .await;
    (scope, chapter_id, run_id)
}
