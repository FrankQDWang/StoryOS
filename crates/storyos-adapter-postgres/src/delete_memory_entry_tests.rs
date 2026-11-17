//! The Delete Memory Entry command (ADR 0094 sections 6 and 9): each row of the outcome table,
//! the `deleted` version and its mark next to a delete by `write_memory`, each reader after the
//! delete, the Project Export Archive, and a Run that pinned its Index before the delete. The
//! lead of these tests is `ad`. Each entry of these tests has the origin `author_request`, and it
//! deletes like any other entry.

use std::convert::Infallible;

use serde_json::{Value, json};
use storyos_application::{
    ConversationSelection, DeleteMemoryEntryInput, MemoryEntryDeleted, ProjectScope,
    RefusableCommandError, list_memory_entries,
};
use storyos_core::{
    DeleteMemoryEntryConflict, DeleteMemoryEntryNoEffect, DeleteMemoryEntryRefusal,
    MemoryWriteRefusal, TransitionOutcome,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::stores;
use crate::agent_run_finish_rule::tests::complete;
use crate::agent_run_load_skill::tests::calls;
use crate::agent_run_read_chapter::tests::finish;
use crate::agent_run_read_memory_entry::tests::refused as read_refused;
use crate::agent_run_steering::tests::queued;
use crate::agent_run_write_memory::tests::{first_index, memory_head, refused, versions};
use crate::command_sequence::tests::{DELETE_MEMORY_ENTRY, issued, seed_project};
use crate::create_agent_run::context::chapter_list_tests::admitted;
use crate::memory_entries::tests::{Version, archived, write_versions};

type Outcome = Result<
    TransitionOutcome<
        MemoryEntryDeleted,
        DeleteMemoryEntryNoEffect,
        DeleteMemoryEntryConflict,
        Infallible,
    >,
    DeleteMemoryEntryRefusal,
>;

/// The entry 1 at the version 2 and the entry 2 at the version 1, at the Memory revision 3.
const TWO_ENTRIES: [Version<'static>; 3] = [
    (1, 1, 1, "current", "character", "阿青", "渡口的船夫。"),
    (2, 1, 2, "current", "foreshadowing", "铜铃", "铃声。"),
    (1, 2, 3, "current", "character", "阿青", "沉默的船夫。"),
];

/// Deletes `memory_entry_id` in the Project of `scope` with the Command Challenge `suffix`.
async fn delete(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    memory_entry_id: &str,
    expected_version: u64,
) -> Outcome {
    let input = DeleteMemoryEntryInput {
        memory_entry_id: memory_entry_id.to_owned(),
        expected_version,
    };
    let call = issued(store, scope, suffix, &DELETE_MEMORY_ENTRY, input).await;
    match store.delete_memory_entry(&call.envelope, &call.input).await {
        Ok(settlement) => Ok(settlement.outcome.map_applied(|applied| applied.effect)),
        Err(RefusableCommandError::RefusedBeforeAdmission(refusal)) => Err(refusal),
        Err(RefusableCommandError::Command(error)) => panic!("the delete must settle: {error:?}"),
    }
}

/// The identity of the entry 1 of the Project of `scope`.
async fn entry_id(admin: &Client, scope: &ProjectScope) -> String {
    admin
        .query_one(
            "SELECT DISTINCT memory_entry_id::text FROM storyos.memory_entry_versions
              WHERE project_id = $1::text::uuid AND entry_number = 1",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// One text of the query `sql` with the parameter `id`, as JSON.
async fn json_of(admin: &Client, sql: &str, id: &str) -> Value {
    let row = admin.query_one(sql, &[&id]).await.unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}

/// The number of each entry that the settings page lists in the Project of `scope`.
async fn listed(store: &PostgresProjectReader, scope: &ProjectScope) -> Vec<u64> {
    let entries = list_memory_entries(store, scope).await.unwrap().unwrap();
    entries.iter().map(|entry| entry.number).collect()
}

/// The applied delete writes the version 3 of the entry 1 with the state `deleted`, the title,
/// the description, no text, and the cause `author_delete` with its Domain Receipt, and moves the
/// Memory revision by 1. The earlier versions stay. A stale version conflicts, a second delete
/// gives no effect, and an entry of no version or of another Project is refused before Admission.
/// The version has the form of a delete by `write_memory`: only the cause differs.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_row_of_the_outcome_table_settles_and_the_deleted_version_keeps_its_mark() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "ad50").await;
    write_versions(&admin, &scope, &TWO_ENTRIES).await;
    let other = seed_project(&store, "ad58").await;
    write_versions(&admin, &other, &TWO_ENTRIES[..1]).await;
    let (entry, foreign) = (
        entry_id(&admin, &scope).await,
        entry_id(&admin, &other).await,
    );
    let unknown = "018f0000-0000-7001-8000-0000000dad5f";
    let outcomes = vec![
        delete(&store, &scope, 0xad51, &entry, 1).await,
        delete(&store, &scope, 0xad52, &entry, 2).await,
        delete(&store, &scope, 0xad53, &entry, 3).await,
        delete(&store, &scope, 0xad54, unknown, 1).await,
        delete(&store, &scope, 0xad55, &foreign, 1).await,
    ];
    let receipt = json_of(
        &admin,
        "SELECT jsonb_build_array(receipt.command_kind, count(*))::text
           FROM storyos.memory_entry_versions AS version
           JOIN storyos.domain_receipts AS receipt
             ON receipt.project_id = version.project_id
            AND receipt.receipt_id::text = version.cause ->> 'receipt_id'
          WHERE version.project_id = $1::text::uuid GROUP BY receipt.command_kind",
        scope.project_id.as_ref(),
    )
    .await;

    // The same delete by `write_memory` in a Project with the same versions.
    let (by_tool, chapter_id, first) = queued(&store, "ad6").await;
    complete(&store, &admin, &by_tool, &first).await;
    write_versions(&admin, &by_tool, &TWO_ENTRIES).await;
    let message = r#"SCRIPT:write_memory delete {"entry": 1}"#;
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &by_tool, &chapter_id, "ad7", message, new).await;
    complete(&store, &admin, &by_tool, &run_id).await;

    let deleted = MemoryEntryDeleted {
        memory_entry_id: entry.clone(),
        version: 3,
        memory_revision: 4,
    };
    let with = |cause: &str| {
        let mut rows: Vec<Value> = [
            "1 1 1 current character author_request 阿青 渡口的船夫。 渡口的船夫。",
            "2 1 2 current foreshadowing author_request 铜铃 铃声。 铃声。",
            "1 2 3 current character author_request 阿青 沉默的船夫。 沉默的船夫。",
        ]
        .iter()
        .map(|row| json!(row))
        .collect();
        rows.push(json!(format!(
            "1 3 4 deleted character author_request 阿青 沉默的船夫。  {cause}"
        )));
        rows
    };
    assert_eq!(
        (
            outcomes,
            receipt,
            versions(&admin, &scope).await,
            memory_head(&admin, &scope).await,
            versions(&admin, &other).await.len(),
            versions(&admin, &by_tool).await,
        ),
        (
            vec![
                Ok(TransitionOutcome::Conflicted(
                    DeleteMemoryEntryConflict::EntryChanged
                )),
                Ok(TransitionOutcome::Applied(deleted)),
                Ok(TransitionOutcome::NoEffect(
                    DeleteMemoryEntryNoEffect::AlreadyDeleted
                )),
                Err(DeleteMemoryEntryRefusal::EntryNotFound),
                Err(DeleteMemoryEntryRefusal::EntryNotFound),
            ],
            json!(["deleteMemoryEntry", 1]),
            with("author_delete"),
            Some((4, 3)),
            1,
            with("author_request"),
        )
    );
}

/// After the delete, the settings page and the Index of the next Run omit the entry, a read and
/// a change of it are refused, a search does not find it, and the Project Export Archive has each
/// version row.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn after_a_delete_no_reader_gives_the_entry_and_the_archive_keeps_each_version() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "ad8").await;
    complete(&store, &admin, &scope, &first).await;
    write_versions(&admin, &scope, &TWO_ENTRIES).await;
    let before = listed(&store, &scope).await;
    let entry = entry_id(&admin, &scope).await;
    let outcome = delete(&store, &scope, 0xad90, &entry, 2).await;
    let mut runs = Vec::new();
    for (prefix, message) in [
        ("ada", "SCRIPT:read_memory_entry 1"),
        ("adb", "SCRIPT:search_memory 阿青"),
        (
            "adc",
            r#"SCRIPT:write_memory change {"entry": 1, "title": "阿青嫂"}"#,
        ),
    ] {
        let new = ConversationSelection::New;
        let run_id = admitted(&store, &scope, &chapter_id, prefix, message, new).await;
        complete(&store, &admin, &scope, &run_id).await;
        runs.push(run_id);
    }
    let search = calls(&admin, &runs[1]).await;

    assert_eq!(
        (
            outcome.is_ok(),
            before,
            listed(&store, &scope).await,
            first_index(&admin, &runs[0]).await,
            calls(&admin, &runs[0]).await,
            search[1][0][3].as_str().unwrap().contains("阿青"),
            calls(&admin, &runs[2]).await,
            archived(&store, &scope, "ad").await,
        ),
        (
            true,
            vec![2, 1],
            vec![2],
            json!([r#"{"type":"foreshadowing"}
{"description":"铃声。","id":2,"title":"铜铃"}"#]),
            json!(["completed", [read_refused(), finish()]]),
            false,
            json!([
                "completed",
                [refused(MemoryWriteRefusal::NotFound), finish()]
            ]),
            (
                vec![
                    json!([1, 1, 1, "current"]),
                    json!([1, 2, 3, "current"]),
                    json!([1, 3, 4, "deleted"]),
                    json!([2, 1, 2, "current"]),
                ],
                vec![json!([4, 3])],
            ),
        )
    );
}

/// The author deletes the entry 1 after the Run pinned an Index that lists it and before the Run
/// reads it. The pinned Index of the Run still lists the entry, the read is refused, and the
/// delete does not change the Run row.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_that_pinned_its_index_before_the_delete_lists_the_entry_and_its_read_is_refused() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "ade").await;
    complete(&store, &admin, &scope, &first).await;
    write_versions(&admin, &scope, &TWO_ENTRIES[..1]).await;
    let message = "SCRIPT:read_memory_entry 1";
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &scope, &chapter_id, "adf", message, new).await;
    let run_row = "SELECT to_jsonb(run)::text FROM storyos.agent_runs AS run
                    WHERE run_id = $1::text::uuid";
    let held = json_of(&admin, run_row, &run_id).await;
    let entry = entry_id(&admin, &scope).await;
    let outcome = delete(&store, &scope, 0xadd0, &entry, 1).await;
    let after_delete = json_of(&admin, run_row, &run_id).await;
    complete(&store, &admin, &scope, &run_id).await;

    assert_eq!(
        (
            outcome.is_ok(),
            after_delete == held,
            first_index(&admin, &run_id).await,
            calls(&admin, &run_id).await,
        ),
        (
            true,
            true,
            json!([r#"{"type":"character"}
{"description":"渡口的船夫。","id":1,"title":"阿青"}"#]),
            json!(["completed", [read_refused(), finish()]]),
        )
    );
}
