//! The shared test support from `memory_work_schedule_tests.rs`.

use crate::PostgresProjectReader;
use storyos_application::{
    MemoryWorkOutcome, MemoryWorkScheduleStore, MemoryWorkSource, ProjectScope,
};
use tokio_postgres::Client;

/// Whether the row of the Conversation is due in the quiet time from now: `None` when the
/// Conversation has no row, `Some(None)` when the due time is empty.
pub(crate) async fn due(admin: &Client, conversation_id: &str) -> Option<Option<bool>> {
    admin
        .query_opt(
            "SELECT due_at > clock_timestamp() + interval '29 minutes'
                AND due_at <= clock_timestamp() + interval '30 minutes'
               FROM storyos.memory_work_schedules WHERE conversation_id = $1::text::uuid",
            &[&conversation_id],
        )
        .await
        .unwrap()
        .map(|row| row.get(/*idx*/ 0))
}

/// Makes the row of the Conversation due now, and each other row not due, so that the claim
/// takes this row.
pub(crate) async fn make_due(admin: &Client, conversation_id: &str) {
    admin
        .execute(
            "UPDATE storyos.memory_work_schedules
                SET due_at = CASE WHEN conversation_id = $1::text::uuid
                                  THEN clock_timestamp() - interval '1 second' END
              WHERE conversation_id = $1::text::uuid OR due_at IS NOT NULL",
            &[&conversation_id],
        )
        .await
        .unwrap();
}

/// Claims the due row of the Conversation, and returns its outcome. The due time is then empty.
pub(crate) async fn claim_due(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    conversation_id: &str,
) -> MemoryWorkOutcome {
    make_due(admin, conversation_id).await;
    let claimed = store.claim_due_memory_work().await.unwrap().unwrap();
    assert_eq!(
        (&claimed.project_scope, &claimed.source),
        (
            scope,
            &MemoryWorkSource::Conversation(conversation_id.to_owned())
        )
    );
    assert_eq!(due(admin, conversation_id).await, Some(None));
    claimed.outcome
}
