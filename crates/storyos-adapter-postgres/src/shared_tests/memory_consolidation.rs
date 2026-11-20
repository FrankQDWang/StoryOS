//! The shared test support from `memory_consolidation_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::memory_capture::{run_capture, start};
use crate::shared_tests::memory_capture_loop::{record, script};
use serde_json::{Value, json};
use storyos_application::{MemoryCaptureSource, ProjectScope, StartedMemoryConsolidation};
use storyos_core::CaptureRefusal;
use tokio_postgres::Client;

/// Settles one capture of `source` with the observations `observations`.
pub(crate) async fn capture(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    source: &MemoryCaptureSource,
    observations: Value,
) {
    let started = start(store, scope, source, &script(&[record(observations)]))
        .await
        .unwrap();
    run_capture(store, admin, scope, &started.run_id).await;
}

/// One observation of the type `character` whose text is the script lines of `operations`.
pub(crate) fn scripted(title: &str, operations: &[Value]) -> Value {
    let lines: Vec<String> = operations
        .iter()
        .map(|operation| format!("SCRIPT:operation {operation}"))
        .collect();
    json!({ "type": "character", "title": title, "text": format!("脚本。\n{}", lines.join("\n")) })
}

pub(crate) fn create(entry_type: &str, title: &str, observations: Value) -> Value {
    json!({
        "action": "create", "entry": null, "entries": null, "type": entry_type, "title": title,
        "description": "述。", "text": "文。", "observations": observations, "reason": null
    })
}

pub(crate) fn update(entry: u64, text: &str) -> Value {
    json!({
        "action": "update", "entry": entry, "entries": null, "type": null, "title": "林舟",
        "description": "船夫。", "text": text, "observations": [1], "reason": null
    })
}

pub(crate) fn delete(entry: u64) -> Value {
    json!({
        "action": "delete", "entry": entry, "entries": null, "type": null, "title": null,
        "description": null, "text": null, "observations": [], "reason": "灯已灭。"
    })
}

/// Starts one pass of the observations that wait in the Project of `scope` in one transaction, as
/// the claim of a schedule row does.
pub(crate) async fn start_pass(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
) -> Result<StartedMemoryConsolidation, CaptureRefusal> {
    let transaction = store
        .begin_serializable_project_command_transaction(scope)
        .await
        .unwrap();
    let started = crate::memory_consolidation::start(&transaction.client, scope)
        .await
        .unwrap();
    match started {
        Ok(_) => transaction.commit().await.unwrap(),
        Err(_) => transaction.rollback().await.unwrap(),
    }
    started
}

/// Starts a pass of the Project of `scope` with a direct call and completes its Run.
pub(crate) async fn run_pass(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
) -> StartedMemoryConsolidation {
    let started = start_pass(store, scope).await.unwrap();
    run_capture(store, admin, scope, &started.run_id).await;
    started
}

/// The Run status, the outcome, the failure reason, and the result of each operation of the pass
/// of the Run `run_id`, and the state of each observation of its Project.
pub(crate) async fn settled(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_build_array(run.status, pass.outcome, pass.failure_reason,
                      COALESCE((SELECT jsonb_agg(jsonb_build_array(item ->> 'result',
                                  item ->> 'refusal', item -> 'entry'))
                                  FROM jsonb_array_elements(pass.operations) AS item), '[]'),
                      (SELECT jsonb_agg(item.state ORDER BY capture.started_at, item.position)
                         FROM storyos.memory_observations AS item
                         JOIN storyos.memory_captures AS capture USING (capture_id)
                        WHERE item.project_id = run.project_id))::text
               FROM storyos.agent_runs AS run
               JOIN storyos.memory_consolidation_passes AS pass USING (run_id)
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get(/*idx*/ 0)).unwrap()
}

/// Each version that a pass wrote in the Project of `scope`, in the order of the Memory revisions:
/// the number, the version, the revision, the state, the type, the origin, the title, the text,
/// the cause kind, and the source kinds.
pub(crate) async fn versions(admin: &Client, scope: &ProjectScope) -> Value {
    let row = admin
        .query_one(
            "SELECT COALESCE(jsonb_agg(jsonb_build_array(entry_number, version, memory_revision,
                      state, entry_type, origin, title, text, cause ->> 'kind',
                      jsonb_path_query_array(source_references, '$[*].kind'))
                      ORDER BY memory_revision), '[]')::text
               FROM storyos.memory_entry_versions
              WHERE project_id = $1::text::uuid AND cause ->> 'kind' = 'consolidation'",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get(/*idx*/ 0)).unwrap()
}

/// The count of the rows of each table that a pass must not write (threat model, AP-22): the
/// Proposals, the manuscript, the Project Instruction, the Skill records, the grants, and the
/// bindings.
pub(crate) async fn guarded(admin: &Client, scope: &ProjectScope) -> Vec<i64> {
    let tables = [
        "proposals",
        "authoritative_revisions",
        "project_instruction_revisions",
        "agent_run_skill_catalog_entries",
        "project_destination_grants",
        "project_policy_revisions",
        "project_external_use_binding_revisions",
    ];
    let mut counts = Vec::new();
    for table in tables {
        let query =
            format!("SELECT count(*) FROM storyos.{table} WHERE project_id = $1::text::uuid");
        let row = admin.query_one(&query, &[&scope.project_id.as_ref()]).await;
        counts.push(row.unwrap().get(/*idx*/ 0));
    }
    counts
}
