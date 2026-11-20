//! The shared test support from `agent_run_write_memory_tests.rs`.

use crate::shared_tests::json_row::json_of;
use serde_json::{Value, json};
use storyos_application::ProjectScope;
use storyos_core::MemoryWriteRefusal;
use tokio_postgres::Client;

pub(crate) fn refused(refusal: MemoryWriteRefusal) -> Value {
    json!([
        "write_memory",
        "refused",
        refusal.label(),
        refusal.result().text,
        true
    ])
}

/// Each version row of the Project of `scope` in the order of its Memory revision, as one line:
/// the number, the version, the revision, the state, the type, the origin, the title, the
/// description, the text, and the kind of the cause.
pub(crate) async fn versions(admin: &Client, scope: &ProjectScope) -> Vec<Value> {
    admin
        .query(
            "SELECT concat_ws(' ', entry_number, version, memory_revision, state, entry_type,
                      origin, title, description, text, cause ->> 'kind')
               FROM storyos.memory_entry_versions
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              ORDER BY memory_revision",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| json!(row.get::<_, String>(/*idx*/ 0)))
        .collect()
}

/// The Memory revision and the next entry number of the head of the Project of `scope`.
pub(crate) async fn memory_head(admin: &Client, scope: &ProjectScope) -> Option<(i64, i64)> {
    admin
        .query_opt(
            "SELECT memory_revision::bigint, next_entry_number::bigint
               FROM storyos.project_memory_heads
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .map(|row| (row.get(/*idx*/ 0), row.get(/*idx*/ 1)))
}

/// The text of each `memory_index` item of the decision Context Assembly at the RunStep 0 of the
/// Run.
pub(crate) async fn first_index(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT COALESCE(jsonb_path_query_array(payload,
                      '$.selected[*] ? (@.source_class == \"memory_index\").content'), '[]')::text
               FROM storyos.context_assembly_manifests
              WHERE run_id = $1::text::uuid AND manifest_role = 'decision'
                AND decision_position = 0",
        &[&run_id],
    )
    .await
}
