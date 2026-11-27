//! Migration 0003 on a database with the rows of before ADR 0107 (#1933). The fixture holds each
//! row of one Project that the code of before ADR 0107 wrote, through its HTTP test of a retained
//! stale-writer refusal: an Author Edit Admission with a writer generation, two writer takeovers
//! with their Admissions, Receipts, and Activity, and a `stale_writer` Acceptance refusal. The
//! writer table and the writer column go, and each other row of the past, its Activity replay, and
//! its Proposal read stay.

use serde_json::{Map, Value};
use storyos_application::{
    ProjectId, ProjectScope, ProposalReader, SnapshotLookup, SnapshotStore, UserId,
};

use crate::recovery_visibility_proof_tests::open_isolated_proof;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::statement_cache::Client;

const MIGRATION: &str = "0003_each_editor_session_writes.sql";
/// Recorded with the base commit `fbfa2a6a` of #1933: `acceptance-refusal-http.integration.test.ts`
/// › "Acceptance retains a stale-writer refusal", read before its Project Export step.
const FIXTURE: &str =
    include_str!("../../../fixtures/writer-history/project-rows-before-adr-0107.json");

fn fixture() -> Map<String, Value> {
    serde_json::from_str(FIXTURE).unwrap()
}

fn only<'a>(tables: &'a Map<String, Value>, table: &str) -> &'a Map<String, Value> {
    let [row] = tables[table].as_array().unwrap().as_slice() else {
        panic!("the fixture has more than one row in {table}");
    };
    row.as_object().unwrap()
}

/// Inserts each row of the fixture with no trigger and no foreign key check, as a restore does.
async fn load(admin: &Client, tables: &Map<String, Value>) {
    admin
        .batch_execute("SET session_replication_role = replica")
        .await
        .unwrap();
    for (table, rows) in tables {
        assert!(table.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_'));
        let inserted = admin
            .execute(
                &format!(
                    "INSERT INTO storyos.{table}
                     SELECT * FROM jsonb_populate_recordset(NULL::storyos.{table}, $1::text::jsonb)"
                ),
                &[&rows.to_string()],
            )
            .await
            .unwrap();
        assert_eq!(inserted, rows.as_array().unwrap().len() as u64, "{table}");
    }
    admin
        .batch_execute("SET session_replication_role = DEFAULT")
        .await
        .unwrap();
}

/// The rows of the past of each table of the fixture as JSON text, with no writer column.
async fn history(admin: &Client, tables: &Map<String, Value>) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    for table in tables.keys() {
        if table == "project_writer_generations" {
            continue;
        }
        let row = admin
            .query_one(
                &format!(
                    "SELECT jsonb_agg(to_jsonb(row) - 'writer_generation'
                                      ORDER BY (to_jsonb(row) - 'writer_generation')::text)::text
                       FROM storyos.{table} AS row"
                ),
                &[],
            )
            .await
            .unwrap();
        rows.push((table.clone(), row.get(/*idx*/ 0)));
    }
    rows
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn migration_0003_drops_the_writer_and_keeps_the_rows_of_the_past_and_their_replay() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (_isolated, store, admin) =
        open_isolated_proof("storyos_es_writer_history", Some(MIGRATION)).await;
    let tables = fixture();
    let project = only(&tables, "projects");
    let scope = ProjectScope::new(
        UserId::new(project["owner_user_id"].as_str().unwrap()),
        ProjectId::new(project["project_id"].as_str().unwrap()),
    );
    let kinds = |table: &str, field: &str| -> Vec<String> {
        tables[table]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row[field].as_str().unwrap().to_owned())
            .collect()
    };
    // The fixture has the writer history that the migration must keep.
    let admissions = kinds("author_command_admissions", "command_kind");
    assert_eq!(
        admissions
            .iter()
            .filter(|kind| *kind == "takeOverProjectWriter")
            .count(),
        2
    );
    assert!(
        tables["author_command_admissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["command_kind"] == "applyAuthorEdit" && row["writer_generation"] == 1)
    );
    assert_eq!(
        tables["project_writer_generations"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(kinds("acceptance_refusals", "reason").contains(&"stale_writer".to_owned()));
    load(&admin, &tables).await;

    let lookup = SnapshotLookup {
        project_scope: scope.clone(),
        snapshot_id: admin
            .query_one(
                "SELECT snapshot_id::text FROM storyos.project_snapshots
                  ORDER BY project_activity_position DESC, created_at DESC LIMIT 1",
                &[],
            )
            .await
            .unwrap()
            .get(0),
    };
    let proposal_id = only(&tables, "proposals")["proposal_id"].as_str().unwrap();
    let read = || async {
        (
            history(&admin, &tables).await,
            store.list_project_activity(&lookup, 0).await.unwrap(),
            store.read_proposal(&scope, proposal_id).await.unwrap(),
        )
    };
    let before = read().await;
    assert_eq!(
        before
            .1
            .iter()
            .filter(|event| event.event_schema == "storyos.event.writer-takeover-applied.v1")
            .count(),
        2
    );
    assert!(before.2.is_some());

    let migration = crate::storage_activation::EMBEDDED_SOURCES
        .iter()
        .find(|source| source.path.ends_with(MIGRATION))
        .unwrap();
    admin
        .batch_execute(&format!("BEGIN;\n{}\nCOMMIT;", migration.sql))
        .await
        .unwrap();

    let dropped = admin
        .query_one(
            "SELECT to_regclass('storyos.project_writer_generations') IS NULL,
                    NOT EXISTS (SELECT 1 FROM information_schema.columns
                                 WHERE table_schema = 'storyos'
                                   AND table_name = 'author_command_admissions'
                                   AND column_name = 'writer_generation')",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        (dropped.get::<_, bool>(0), dropped.get::<_, bool>(1)),
        (true, true)
    );
    assert_eq!(read().await, before);
    // Each retained refusal reason, `stale_writer` also, still decodes.
    for reason in kinds("acceptance_refusals", "reason") {
        crate::acceptance_refusal::parse_reason(&reason).unwrap();
    }
}
