//! The shared test support from `project_research_binding_tests.rs`.

use storyos_application::ProjectScope;
use tokio_postgres::Client;

/// Pauses the queued Runs of the Project, because later HTTP files drain all queued work.
pub(crate) async fn park_queued_runs(admin: &Client, scope: &ProjectScope) {
    admin
        .execute(
            "UPDATE storyos.agent_runs
                SET status = 'paused', lease_expires_at = NULL, wakeup_pending = false
              WHERE project_id = $1::text::uuid AND status = 'queued'",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
}
