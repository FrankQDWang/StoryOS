//! The shared test support from `memory_consolidation_merge_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::memory_consolidation::{capture, run_pass, scripted};
use serde_json::{Value, json};
use storyos_application::{MemoryCaptureSource, ProjectScope, StartedMemoryConsolidation};
use tokio_postgres::Client;

/// One entry for `seed`: the number, the type, the origin, and the title.
pub(crate) type Seed<'a> = (u64, &'a str, &'a str, &'a str);

/// Writes version 1 of each entry of `rows` with SQL, at the revision of its number, and moves
/// the head of the Project to the last row. A background entry has one source of its origin. The
/// origin `author_delete` gives a deleted entry of the author, which is a delete mark. The cause
/// kind `seed` is a test value.
pub(crate) async fn seed(admin: &Client, scope: &ProjectScope, rows: &[Seed<'_>]) {
    let rows: Vec<Value> = rows
        .iter()
        .map(|(number, entry_type, origin, title)| json!([number, entry_type, origin, title]))
        .collect();
    admin
        .batch_execute(&format!(
            "WITH rows AS (
                 SELECT (row ->> 0)::numeric AS number, row ->> 1 AS entry_type,
                        row ->> 2 AS origin, row ->> 3 AS title, md5('{project}:' || (row ->> 0))
                        AS id
                   FROM jsonb_array_elements('{rows}'::jsonb) AS row),
             entries AS (
                 INSERT INTO storyos.memory_entry_versions
                     (owner_user_id, project_id, memory_entry_id, entry_number, version,
                      memory_revision, state, entry_type, origin, title, description, text,
                      cause, source_references)
                 SELECT '{owner}', '{project}', id::uuid, number, 1, number,
                        CASE origin WHEN 'author_delete' THEN 'deleted' ELSE 'current' END,
                        entry_type,
                        CASE origin WHEN 'author_delete' THEN 'author_request' ELSE origin END,
                        title, '述。', '文。',
                        jsonb_build_object('kind', CASE origin WHEN 'author_delete'
                                                   THEN 'author_delete' ELSE 'seed' END),
                        CASE origin
                          WHEN 'chapter' THEN jsonb_build_array(jsonb_build_object(
                               'kind', 'chapter', 'chapter_id', id::uuid))
                          WHEN 'conversation' THEN jsonb_build_array(jsonb_build_object(
                               'kind', 'conversation', 'conversation_id', id::uuid))
                          ELSE '[]' END
                   FROM rows)
             INSERT INTO storyos.project_memory_heads
             SELECT '{owner}', '{project}', max(number), max(number) + 1 FROM rows",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            rows = Value::from(rows),
        ))
        .await
        .unwrap();
}

/// A merge of the entries `entries` into a result with the title `title`.
pub(crate) fn merge(entries: Value, title: &str) -> Value {
    json!({
        "action": "merge", "entry": null, "entries": entries, "type": null, "title": title,
        "description": "合述。", "text": "合文。", "observations": [1], "reason": null
    })
}

/// A conflict flag of the entry `entry` with the reason `reason` and the observation 1.
pub(crate) fn flag(entry: u64, reason: &str) -> Value {
    json!({
        "action": "flag_conflict", "entry": entry, "entries": null, "type": null, "title": null,
        "description": null, "text": null, "observations": [1], "reason": reason
    })
}

/// Each version of each entry, in the order of the numbers and the versions: the number, the
/// version, the Memory revision, the state, the origin, the title, the description, the text, the
/// cause kind, the reason of the flag, and the source kind of the flag.
pub(crate) async fn flag_lines(admin: &Client, scope: &ProjectScope) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_agg(jsonb_build_array(entry_number, version, memory_revision, state,
                      origin, title, description, text, cause ->> 'kind', flag_reason,
                      flag_source ->> 'kind') ORDER BY entry_number, version)::text
               FROM storyos.memory_entry_versions WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    version_lines(&serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
}

/// Captures one observation with the script lines of `operations` and runs one pass.
pub(crate) async fn pass_on(
    (store, admin): (&PostgresProjectReader, &Client),
    scope: &ProjectScope,
    source: &MemoryCaptureSource,
    operations: &[Value],
) -> StartedMemoryConsolidation {
    let observations = json!([scripted("脚本", operations)]);
    capture(store, admin, scope, source, observations).await;
    run_pass(store, admin, scope).await
}

/// Each version of `versions` as one line of its values, with the source kinds joined by commas.
pub(crate) fn version_lines(versions: &Value) -> Value {
    let text = |value: &Value| match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(|item| item.as_str().unwrap())
            .collect::<Vec<_>>()
            .join(","),
        value => value.to_string(),
    };
    let rows = versions.as_array().unwrap().iter();
    let lines = rows.map(|row| {
        row.as_array()
            .unwrap()
            .iter()
            .map(text)
            .collect::<Vec<_>>()
            .join(" ")
    });
    lines.collect()
}
