//! The shared test support from `passage_index_work_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::project_embedding_binding::FAKE;
use std::time::Duration;
use storyos_adapter_fake_destination::FakeEmbeddings;
use storyos_application::{
    ClaimedPassageIndex, EmbeddingProviderAdapter, PassageIndexStep, ProjectScope,
    claim_next_passage_index_work, step_passage_index_work,
};
use storyos_core::{EmbeddingRegistration, HOST_FAKE_EMBEDDING_REGISTRATION};
use tokio_postgres::Client;

pub(crate) const FAKE_ONLY: &[&EmbeddingRegistration] = &[&HOST_FAKE_EMBEDDING_REGISTRATION];

/// The stores of a deployment with the fake embedding configuration.
pub(crate) async fn configured() -> (PostgresProjectReader, Client) {
    let (store, admin) = stores().await;
    (store.with_embedding_registration(FAKE), admin)
}

/// Gives the open transaction of `admin` the fake embedding configuration of the deployment.
pub(crate) async fn deploy(admin: &Client) {
    admin
        .execute(
            "SELECT set_config('storyos.embedding_registration', $1, true)",
            &[&HOST_FAKE_EMBEDDING_REGISTRATION.revision],
        )
        .await
        .unwrap();
}

/// The work rows of the Project in Chapter order: the Chapter, its order, its mark generation,
/// its completed generation, and whether its quiet time runs.
pub(crate) async fn work(
    admin: &Client,
    scope: &ProjectScope,
) -> Vec<(String, i64, i64, i64, bool)> {
    admin
        .query(
            "SELECT chapter_id::text, chapter_order, mark_generation, completed_generation,
                    not_before > clock_timestamp()
               FROM storyos.passage_index_work WHERE project_id = $1::text::uuid
              ORDER BY chapter_order",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4)))
        .collect()
}

/// Ends the quiet time of each work row of the Project.
pub(crate) async fn wake(admin: &Client, scope: &ProjectScope) {
    admin
        .execute(
            "UPDATE storyos.passage_index_work
                SET not_before = clock_timestamp() - interval '1 second'
              WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
}

/// Moves the work of each other Project one day later, so that a claim takes only this Project.
pub(crate) async fn park_other_work(admin: &Client, scope: &ProjectScope) {
    admin
        .execute(
            "UPDATE storyos.passage_index_work
                SET not_before = clock_timestamp() + interval '1 day'
              WHERE project_id <> $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
}

pub(crate) async fn claim(
    store: &PostgresProjectReader,
    registrations: &[&'static EmbeddingRegistration],
) -> Option<ClaimedPassageIndex> {
    claim_next_passage_index_work(store, registrations)
        .await
        .unwrap()
}

pub(crate) async fn step(
    store: &PostgresProjectReader,
    claim: &ClaimedPassageIndex,
) -> PassageIndexStep {
    step_with(store, &FakeEmbeddings, claim).await
}

/// One step of the claim with `adapter`, with a bounded wait: an action of the author that waits
/// for the work ends the test.
pub(crate) async fn step_with(
    store: &PostgresProjectReader,
    adapter: &impl EmbeddingProviderAdapter,
    claim: &ClaimedPassageIndex,
) -> PassageIndexStep {
    let step = step_passage_index_work(store, adapter, FAKE_ONLY, claim);
    tokio::time::timeout(Duration::from_secs(/*secs*/ 60), step)
        .await
        .expect("the step ends")
        .unwrap()
}

/// The steps of one claim until a step does not write.
pub(crate) async fn steps(
    store: &PostgresProjectReader,
    claim: &ClaimedPassageIndex,
) -> Vec<&'static str> {
    let mut steps = Vec::new();
    for _ in 0..8 {
        let result = step(store, claim).await;
        steps.push(match result {
            PassageIndexStep::Written => "written",
            PassageIndexStep::Completed => "completed",
            PassageIndexStep::Released(_) => "released",
            PassageIndexStep::StaleFence => "stale_fence",
        });
        if result != PassageIndexStep::Written {
            break;
        }
    }
    steps
}
