//! The shared test support from `memory_consolidation_conflict_tests.rs`.

use crate::statement_cache::Client;
use serde_json::{Value, json};
use storyos_application::ProjectScope;

/// A conflict item of a capture with the entry `entry` and the reason `reason`.
pub(crate) fn conflict(entry: u64, reason: &str) -> Value {
    json!({
        "kind": "conflict", "type": null, "title": null, "text": null, "entry": entry,
        "reason": reason
    })
}

/// The identity of the entry `number` of the Project.
pub(crate) async fn entry_id(admin: &Client, scope: &ProjectScope, number: u64) -> String {
    let row = admin
        .query_one(
            "SELECT memory_entry_id::text FROM storyos.memory_entry_versions
              WHERE project_id = $1::text::uuid AND entry_number = $2::text::numeric LIMIT 1",
            &[&scope.project_id.as_ref(), &number.to_string()],
        )
        .await
        .unwrap();
    row.get(/*idx*/ 0)
}
