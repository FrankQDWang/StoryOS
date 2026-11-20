//! The shared test support from `memory_capture_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_steering::queued;
use storyos_application::{
    MemoryCaptureSource, MemoryCaptureStartRefusal, ProjectScope, StartedMemoryCapture,
};
use storyos_core::ProjectionMode;
use tokio_postgres::Client;

/// A capture gives way to each queued Run of the author, and earlier tests leave queued Runs in
/// the shared database. Park them, so that only the Runs of this test are queued.
pub(crate) async fn park_queued_author_runs(admin: &Client) {
    admin
        .execute(
            "UPDATE storyos.agent_runs
                SET status = 'paused', lease_expires_at = NULL, wakeup_pending = false
              WHERE run_kind = 'author' AND status = 'queued'",
            &[],
        )
        .await
        .unwrap();
}

/// A Project of `prefix` with one completed author Run, its Chapter, and the source of that Run.
pub(crate) async fn project(
    store: &PostgresProjectReader,
    admin: &Client,
    prefix: &str,
) -> (ProjectScope, String, MemoryCaptureSource) {
    park_queued_author_runs(admin).await;
    let (scope, chapter_id, run_id) = queued(store, prefix).await;
    complete(store, admin, &scope, &run_id).await;
    let conversation_id: String = admin
        .query_one(
            "SELECT conversation_id::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let source = MemoryCaptureSource::Conversation {
        conversation_id,
        first_run_id: run_id.clone(),
        last_run_id: run_id,
    };
    (scope, chapter_id, source)
}

/// Completes the capture Run `run_id`. A capture that gave its claim back to a queued Run of the
/// author in another Project gets a new claim, until its Run is not queued.
pub(crate) async fn run_capture(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    run_id: &str,
) {
    for _ in 0..=storyos_core::MEMORY_CAPTURE_MAX_RUN_STEPS {
        complete(store, admin, scope, run_id).await;
        let status: String = admin
            .query_one(
                "SELECT status FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
                &[&run_id],
            )
            .await
            .unwrap()
            .get(/*idx*/ 0);
        if status != "queued" {
            return;
        }
    }
    panic!("the capture {run_id} did not end");
}

/// Starts one capture of `source` with the input `input` in one transaction, as a trigger does.
pub(crate) async fn start(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    source: &MemoryCaptureSource,
    input: &str,
) -> Result<StartedMemoryCapture, MemoryCaptureStartRefusal> {
    let transaction = store
        .begin_serializable_project_command_transaction(scope)
        .await
        .unwrap();
    let started = crate::memory_capture::start(
        &transaction.client,
        scope,
        source,
        input,
        ProjectionMode::ExactRequired,
    )
    .await
    .unwrap();
    match started {
        Ok(_) => transaction.commit().await.unwrap(),
        Err(_) => transaction.rollback().await.unwrap(),
    }
    started
}
