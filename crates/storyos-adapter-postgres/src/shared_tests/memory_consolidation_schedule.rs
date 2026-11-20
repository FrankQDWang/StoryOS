//! The shared test support from `memory_consolidation_schedule_tests.rs`.

use crate::PostgresProjectReader;
use serde_json::{Value, json};
use storyos_application::{
    MemoryWorkOutcome, MemoryWorkScheduleStore, MemoryWorkSource, ProjectScope,
};
use tokio_postgres::Client;

/// The consolidation row of the Project: whether it names no Conversation and the Project as its
/// source, and its due time. The due time is `null` when it is empty, `"now"` when it is in the
/// past, and else its distance from the start of the last pass. `None`: the Project has no row.
pub(crate) async fn pass_due(admin: &Client, scope: &ProjectScope) -> Option<Value> {
    let row = admin
        .query_opt(
            "SELECT jsonb_build_array(work.conversation_id IS NULL,
                      work.source_id = work.project_id,
                      CASE WHEN work.due_at <= clock_timestamp() THEN 'now'
                           ELSE (work.due_at - (SELECT max(pass.started_at)
                                                  FROM storyos.memory_consolidation_passes AS pass
                                                 WHERE pass.project_id = work.project_id))::text
                      END)::text
               FROM storyos.memory_work_schedules AS work
              WHERE work.project_id = $1::text::uuid AND work.work_kind = 'consolidation'",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    row.map(|row| serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
}

/// Makes the consolidation row of the Project due now, and each other row not due, so that the
/// claim takes this row.
pub(crate) async fn make_pass_due(admin: &Client, scope: &ProjectScope) {
    admin
        .execute(
            "UPDATE storyos.memory_work_schedules
                SET due_at = CASE WHEN project_id = $1::text::uuid AND work_kind = 'consolidation'
                                  THEN clock_timestamp() - interval '1 second' END
              WHERE (project_id = $1::text::uuid AND work_kind = 'consolidation')
                 OR due_at IS NOT NULL",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
}

/// Claims the due consolidation row of the Project, and returns its outcome. The due time is then
/// empty.
pub(crate) async fn claim_pass(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
) -> MemoryWorkOutcome {
    make_pass_due(admin, scope).await;
    let claimed = store.claim_due_memory_work().await.unwrap().unwrap();
    assert_eq!(
        (&claimed.project_scope, &claimed.source),
        (scope, &MemoryWorkSource::Project)
    );
    assert_eq!(
        pass_due(admin, scope).await,
        Some(json!([true, true, null]))
    );
    claimed.outcome
}
