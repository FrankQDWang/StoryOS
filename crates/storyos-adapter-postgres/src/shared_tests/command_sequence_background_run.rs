//! The shared test support from `command_sequence_tests/background_run_tests.rs`.

use crate::PostgresProjectReader;
use storyos_application::{AgentRunWorkStore, ProjectScope};
use storyos_core::ModelAdapter;
use tokio_postgres::NoTls;

/// Claims as another Worker. An admin transaction locks each AgentRun outside the Project, so
/// that the claim can take only a Run of this Project and changes no other test data.
pub(crate) async fn claim_in_project(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
) -> Option<storyos_application::ClaimedAgentRun> {
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL").unwrap();
    let (others, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(connection);
    others.batch_execute("BEGIN").await.unwrap();
    others
        .execute(
            "SELECT 1 FROM storyos.agent_runs WHERE project_id <> $1::text::uuid FOR UPDATE",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let claimed = store
        .claim_next_agent_run(&[
            ModelAdapter::HostFake,
            ModelAdapter::VolcengineAgentPlanResponses,
        ])
        .await
        .unwrap();
    others.batch_execute("ROLLBACK").await.unwrap();
    claimed
}
