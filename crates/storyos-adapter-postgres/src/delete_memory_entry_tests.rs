//! The Delete Memory Entry command (ADR 0094 sections 6 and 9): each row of the outcome table,
//! the `deleted` version and its mark next to a delete by `write_memory`, each reader after the
//! delete, the Project Export Archive, and a Run that pinned its Index before the delete. The
//! lead of these tests is `9d`. Each entry of these tests has the origin `author_request`, and it
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
use crate::agent_run_steering::tests::queued;
use crate::agent_run_write_memory::tests::first_index;
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

/// The identity of the entry `number` of the Project of `scope`.
async fn entry_id(admin: &Client, scope: &ProjectScope, number: u64) -> String {
    admin
        .query_one(
            "SELECT DISTINCT memory_entry_id::text FROM storyos.memory_entry_versions
              WHERE project_id = $1::text::uuid AND entry_number = $2::text::numeric",
            &[&scope.project_id.as_ref(), &number.to_string()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// Each version row of the Project of `scope` in the order of its Memory revision, as one line:
/// the number, the version, the revision, the state, the type, the origin, the title, the
/// description, the text, the kind of the cause, and the kinds of the source references.
async fn version_rows(admin: &Client, scope: &ProjectScope) -> Vec<Value> {
    admin
        .query(
            "SELECT concat_ws(' ', entry_number, version, memory_revision, state, entry_type,
                      origin, title, description, '[' || text || ']', cause ->> 'kind',
                      jsonb_path_query_array(source_references, '$[*].kind')::text)
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

/// The Memory revision of the head of the Project of `scope`.
async fn head_revision(admin: &Client, scope: &ProjectScope) -> i64 {
    admin
        .query_one(
            "SELECT memory_revision::bigint FROM storyos.project_memory_heads
              WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// The number of each entry that the settings page lists in the Project of `scope`.
async fn listed(store: &PostgresProjectReader, scope: &ProjectScope) -> Vec<u64> {
    list_memory_entries(store, scope)
        .await
        .unwrap()
        .unwrap()
        .iter()
        .map(|entry| entry.number)
        .collect()
}

/// The Run row of `run_id` as JSON.
async fn run_row(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT to_jsonb(run)::text FROM storyos.agent_runs AS run
              WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap()
}

/// The refused `read_memory_entry` call of a deleted entry.
fn read_refused() -> Value {
    let text = "The Host refused this call because no current entry has this id: use an id of \
                the section Memory index, so no result is available, and this is not an empty \
                result.";
    json!([
        "read_memory_entry",
        "refused",
        "memory_entry_not_found",
        text,
        true
    ])
}

fn finish() -> Value {
    json!(["finish", "settled", null, "", true])
}

/// The applied delete writes the version 3 of the entry 1 with the state `deleted`, the title,
/// the description, no text, and the cause `author_delete` with its Domain Receipt, and moves the
/// Memory revision by 1. The earlier versions stay. A stale version conflicts, a second delete
/// gives no effect, and an entry of no version or of another Project is refused before Admission.
/// The version has the form of a delete by `write_memory`: only the cause and the source
/// references differ.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_row_of_the_outcome_table_settles_and_the_deleted_version_keeps_its_mark() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "9d50").await;
    write_versions(&admin, &scope, &TWO_ENTRIES).await;
    let other = seed_project(&store, "9d58").await;
    write_versions(&admin, &other, &TWO_ENTRIES[..1]).await;
    let (entry, foreign) = (
        entry_id(&admin, &scope, 1).await,
        entry_id(&admin, &other, 1).await,
    );
    let unknown = "018f0000-0000-7001-8000-0000000d9d5f";
    let outcomes = vec![
        delete(&store, &scope, 0x9d51, &entry, 1).await,
        delete(&store, &scope, 0x9d52, &entry, 2).await,
        delete(&store, &scope, 0x9d53, &entry, 3).await,
        delete(&store, &scope, 0x9d54, unknown, 1).await,
        delete(&store, &scope, 0x9d55, &foreign, 1).await,
    ];
    let receipt: Value = admin
        .query_one(
            "SELECT jsonb_build_array(receipt.receipt_id::text = version.cause ->> 'receipt_id',
                      receipt.command_kind)::text
               FROM storyos.memory_entry_versions AS version
               JOIN storyos.domain_receipts AS receipt
                 ON receipt.project_id = version.project_id
                AND receipt.receipt_id::text = version.cause ->> 'receipt_id'
              WHERE version.project_id = $1::text::uuid AND version.state = 'deleted'",
            &[&scope.project_id.as_ref()],
        )
        .await
        .map(|row| serde_json::from_str(row.get::<_, &str>(/*idx*/ 0)).unwrap())
        .unwrap();

    // The same delete by `write_memory` in a Project with the same versions.
    let (by_tool, chapter_id, first) = queued(&store, "9d6").await;
    complete(&store, &admin, &by_tool, &first).await;
    write_versions(&admin, &by_tool, &TWO_ENTRIES).await;
    let message = r#"SCRIPT:write_memory delete {"entry": 1}"#;
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &by_tool, &chapter_id, "9d7", message, new).await;
    complete(&store, &admin, &by_tool, &run_id).await;

    let deleted = MemoryEntryDeleted {
        memory_entry_id: entry.clone(),
        version: 3,
        memory_revision: 4,
    };
    let earlier = [
        "1 1 1 current character author_request 阿青 渡口的船夫。 [渡口的船夫。]  []",
        "2 1 2 current foreshadowing author_request 铜铃 铃声。 [铃声。]  []",
        "1 2 3 current character author_request 阿青 沉默的船夫。 [沉默的船夫。]  []",
    ];
    let by_author = "1 3 4 deleted character author_request 阿青 沉默的船夫。 [] author_delete []";
    let by_write_memory = "1 3 4 deleted character author_request 阿青 沉默的船夫。 [] \
                           author_request [\"conversation\"]";
    let with = |last: &str| {
        let mut rows: Vec<Value> = earlier.iter().map(|row| json!(row)).collect();
        rows.push(json!(last));
        rows
    };
    assert_eq!(
        (
            outcomes,
            receipt,
            version_rows(&admin, &scope).await,
            head_revision(&admin, &scope).await,
            version_rows(&admin, &other).await.len(),
            version_rows(&admin, &by_tool).await,
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
            json!([true, "deleteMemoryEntry"]),
            with(by_author),
            4,
            1,
            with(by_write_memory),
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
    let (scope, chapter_id, first) = queued(&store, "9d8").await;
    complete(&store, &admin, &scope, &first).await;
    write_versions(&admin, &scope, &TWO_ENTRIES).await;
    let before = listed(&store, &scope).await;
    let entry = entry_id(&admin, &scope, 1).await;
    let outcome = delete(&store, &scope, 0x9d90, &entry, 2).await;
    let mut runs = Vec::new();
    for (prefix, message) in [
        ("9da", "SCRIPT:read_memory_entry 1"),
        ("9db", "SCRIPT:search_memory 阿青"),
        (
            "9dc",
            r#"SCRIPT:write_memory change {"entry": 1, "title": "阿青嫂"}"#,
        ),
    ] {
        let new = ConversationSelection::New;
        let run_id = admitted(&store, &scope, &chapter_id, prefix, message, new).await;
        complete(&store, &admin, &scope, &run_id).await;
        runs.push(run_id);
    }
    let not_found = MemoryWriteRefusal::NotFound;
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
            archived(&store, &scope, "9d").await,
        ),
        (
            true,
            vec![1, 2],
            vec![2],
            json!([r#"{"type":"foreshadowing"}
{"description":"铃声。","id":2,"title":"铜铃"}"#]),
            json!(["completed", [read_refused(), finish()]]),
            false,
            json!([
                "completed",
                [
                    [
                        "write_memory",
                        "refused",
                        not_found.label(),
                        not_found.result().text,
                        true
                    ],
                    finish()
                ]
            ]),
            (
                vec![
                    json!(["1", "1", "1", "current"]),
                    json!(["1", "2", "3", "current"]),
                    json!(["1", "3", "4", "deleted"]),
                    json!(["2", "1", "2", "current"]),
                ],
                vec![json!(["4", "3"])],
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
    let (scope, chapter_id, first) = queued(&store, "9de").await;
    complete(&store, &admin, &scope, &first).await;
    write_versions(&admin, &scope, &TWO_ENTRIES[..1]).await;
    let message = "SCRIPT:read_memory_entry 1";
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &scope, &chapter_id, "9df", message, new).await;
    let held = run_row(&admin, &run_id).await;
    let entry = entry_id(&admin, &scope, 1).await;
    let outcome = delete(&store, &scope, 0x9dd0, &entry, 1).await;
    let after_delete = run_row(&admin, &run_id).await;
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
