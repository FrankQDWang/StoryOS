//! Migration 0003 on a database with the rows of before ADR 0107 (#1933): the writer table and the
//! writer column go, and each other row of the past and its Activity replay stay.

use storyos_application::{ProjectId, ProjectScope, SnapshotLookup, SnapshotStore, UserId};

use crate::recovery_visibility_proof_tests::{create_empty_project, open_isolated_proof};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::statement_cache::Client;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const MIGRATION: &str = "0003_each_editor_session_writes.sql";
const SESSION: &str = "018f0000-0000-7001-8000-00000000d7a2";

/// The rows of the past of the Project as JSON text, with no writer column.
async fn history(admin: &Client, project_id: &str) -> String {
    admin
        .query_one(
            "SELECT jsonb_build_array(
               (SELECT jsonb_agg(to_jsonb(row) - 'writer_generation' ORDER BY row.author_command_admission_id)
                  FROM storyos.author_command_admissions AS row WHERE row.project_id = $1::text::uuid),
               (SELECT jsonb_agg(to_jsonb(row) ORDER BY row.receipt_id)
                  FROM storyos.domain_receipts AS row WHERE row.project_id = $1::text::uuid),
               (SELECT jsonb_agg(to_jsonb(row) ORDER BY row.project_activity_position)
                  FROM storyos.project_activity_events AS row WHERE row.project_id = $1::text::uuid),
               (SELECT jsonb_agg(to_jsonb(row) ORDER BY row.editor_session_id)
                  FROM storyos.editor_sessions AS row WHERE row.project_id = $1::text::uuid))::text",
            &[&project_id],
        )
        .await
        .unwrap()
        .get(0)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn migration_0003_drops_the_writer_and_keeps_the_rows_of_the_past_and_their_replay() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (_isolated, store, admin) =
        open_isolated_proof("storyos_es_writer_history", Some(MIGRATION)).await;
    let project_id = create_empty_project(&store, "d7a1").await;
    // The writer of the Project before ADR 0107: one Editor Session with writer generation 1.
    admin
        .batch_execute(&format!(
            "INSERT INTO storyos.editor_sessions (owner_user_id, project_id, editor_session_id,
               client_session_binding_ref, client_session_generation, client_contract_revision,
               security_policy_revision)
             VALUES ('{USER_A}', '{project_id}', '{SESSION}', 'binding', 1, 'contract', 'policy');
             INSERT INTO storyos.project_writer_generations
             VALUES ('{USER_A}', '{project_id}', 1, '{SESSION}');"
        ))
        .await
        .unwrap();
    let lookup = SnapshotLookup {
        project_scope: ProjectScope::new(UserId::new(USER_A), ProjectId::new(&project_id)),
        snapshot_id: admin
            .query_one(
                "SELECT snapshot_id::text FROM storyos.project_snapshots
                  WHERE project_id = $1::text::uuid
                  ORDER BY project_activity_position DESC LIMIT 1",
                &[&project_id],
            )
            .await
            .unwrap()
            .get(0),
    };
    let before = (
        history(&admin, &project_id).await,
        store.list_project_activity(&lookup, 0).await.unwrap(),
    );
    assert!(!before.1.is_empty());

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
    let after = (
        history(&admin, &project_id).await,
        store.list_project_activity(&lookup, 0).await.unwrap(),
    );
    assert_eq!(after, before);
}
