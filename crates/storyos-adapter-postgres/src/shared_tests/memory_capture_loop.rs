//! The shared test support from `memory_capture_loop_tests.rs`.

use crate::shared_tests::json_row::json_of;
use crate::statement_cache::Client;
use serde_json::{Value, json};
use storyos_application::ProjectScope;

/// The input of a capture: one script line for each call, in RunStep order.
pub(crate) fn script(calls: &[(&str, Value)]) -> String {
    let lines: Vec<String> = calls
        .iter()
        .map(|(name, arguments)| format!("SCRIPT:capture {name} {arguments}"))
        .collect();
    format!("林舟在雨夜撑船。\n{}", lines.join("\n"))
}

/// The `record_memory_observations` call with `observations`. An item with no key `kind` is an
/// observation of a fact, and it gets the null entry and the null reason of the strict schema.
pub(crate) fn record(observations: Value) -> (&'static str, Value) {
    let items = observations.as_array().into_iter().flatten().map(|item| {
        let mut item = item.clone();
        if item.get("kind").is_none() {
            let fact = json!({ "kind": "observation", "entry": null, "reason": null });
            item.as_object_mut()
                .unwrap()
                .extend(fact.as_object().unwrap().clone());
        }
        item
    });
    let arguments = json!({ "observations": items.collect::<Vec<_>>() });
    ("record_memory_observations", arguments)
}

/// The Run status, the capture state, outcome, and failure reason, and each observation of the
/// Run `run_id`.
pub(crate) async fn settlement(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT jsonb_build_array(run.status, capture.state, capture.outcome,
                      capture.failure_reason,
                      COALESCE((SELECT jsonb_agg(jsonb_build_array(item.position, item.entry_type,
                                  item.title, item.text, item.state) ORDER BY item.position)
                                  FROM storyos.memory_observations AS item
                                 WHERE item.capture_id = capture.capture_id), '[]'))::text
               FROM storyos.agent_runs AS run
               JOIN storyos.memory_captures AS capture USING (owner_user_id, project_id, run_id)
              WHERE run.run_id = $1::text::uuid",
        &[&run_id],
    )
    .await
}

/// The count of the Memory entry versions, the Proposals, the Research Artifacts, and the
/// Authoritative Revisions of the Project of `scope`.
pub(crate) async fn other_records(admin: &Client, scope: &ProjectScope) -> Value {
    let tables = [
        "memory_entry_versions",
        "proposals",
        "research_artifacts",
        "authoritative_revisions",
    ];
    let mut counts = Vec::new();
    for table in tables {
        let query = format!(
            "SELECT count(*) FROM storyos.{table}
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid"
        );
        let values = [scope.owner_user_id.as_ref(), scope.project_id.as_ref()];
        let row = admin.query_one(&query, &[&values[0], &values[1]]).await;
        counts.push(row.unwrap().get::<_, i64>(/*idx*/ 0));
    }
    json!(counts)
}

/// For each decision request of the Run: its purpose, its `tool_choice`, and its Tools.
pub(crate) async fn requests(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT jsonb_agg(jsonb_build_array(wire ->> 'purpose', wire ->> 'tool_choice',
                      wire -> 'tools') ORDER BY decision_position)::text
               FROM storyos.model_attempts,
                    LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'",
        &[&run_id],
    )
    .await
}
