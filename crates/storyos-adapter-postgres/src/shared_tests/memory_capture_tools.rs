//! The shared test support from `memory_capture_tools_tests.rs`.

use storyos_application::{MemoryCaptureSource, ProjectScope};
use tokio_postgres::Client;

/// The Conversation of the first Run of the author of the Project, as the source of a capture.
pub(crate) async fn conversation_source(
    admin: &Client,
    scope: &ProjectScope,
) -> MemoryCaptureSource {
    let row = admin
        .query_one(
            "SELECT run_id::text, conversation_id::text FROM storyos.agent_runs
              WHERE project_id = $1::text::uuid AND run_kind = 'author'
              ORDER BY run_id LIMIT 1",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    MemoryCaptureSource::Conversation {
        conversation_id: row.get(/*idx*/ 1),
        first_run_id: row.get(/*idx*/ 0),
        last_run_id: row.get(/*idx*/ 0),
    }
}
