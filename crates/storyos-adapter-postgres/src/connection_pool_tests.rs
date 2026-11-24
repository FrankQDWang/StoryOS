use std::collections::BTreeMap;
use std::time::SystemTime;

use storyos_application::{ProjectId, ProjectScope, UserId};

use crate::PostgresProjectReader;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::statement_cache::STATEMENT_CAPACITY;

const USER: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT: &str = "018f0000-0000-7001-8000-000000000002";

const BACKEND_IDENTITY: &str =
    "SELECT pid, backend_start FROM pg_stat_activity WHERE pid = pg_backend_pid()";

// A PID can be reused; the pair with `backend_start` is unique.
async fn backend_identity(client: &crate::statement_cache::Client) -> (i32, SystemTime) {
    let row = client.query_one(BACKEND_IDENTITY, &[]).await.unwrap();
    (row.get(0), row.get(1))
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_settled_connection_is_reused_and_an_unsettled_transaction_is_not() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let scope = ProjectScope::new(UserId::new(USER), ProjectId::new(PROJECT));

    let first_read = backend_identity(&store.connect().await.unwrap()).await;
    let second_read = backend_identity(&store.connect().await.unwrap()).await;

    let unsettled = store
        .begin_project_command_transaction(&scope)
        .await
        .unwrap();
    let unsettled_backend = backend_identity(&unsettled.client).await;
    drop(unsettled);
    let committed = store
        .begin_project_command_transaction(&scope)
        .await
        .unwrap();
    let committed_backend = backend_identity(&committed.client).await;
    committed.commit().await.unwrap();
    let after_commit = backend_identity(&store.connect().await.unwrap()).await;

    assert_eq!(
        first_read, second_read,
        "sequential reads must share one backend"
    );
    assert_ne!(
        unsettled_backend, committed_backend,
        "a transaction dropped before COMMIT or ROLLBACK must not return its connection"
    );
    assert_eq!(
        committed_backend, after_commit,
        "a committed transaction must return its connection"
    );
}

const KEPT_STATEMENTS: &str = "SELECT statement, generic_plans + custom_plans FROM pg_prepared_statements \
     WHERE statement NOT LIKE '%pg_prepared_statements%'";

/// The kept statements of the session of the client, with the number of runs of each.
async fn kept_statements(client: &crate::statement_cache::Client) -> BTreeMap<String, i64> {
    client
        .query(KEPT_STATEMENTS, &[])
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect()
}

fn runtime_store() -> PostgresProjectReader {
    PostgresProjectReader::new(
        std::env::var("STORYOS_TEST_DATABASE_URL")
            .expect("run through scripts/verify-project-scope.sh"),
    )
}

const TERMINATE: &str = "SELECT pg_terminate_backend($1)";

const REPEATED: &str = "SELECT $1::text || ' and one repeated statement'";

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_second_run_of_one_statement_on_one_connection_prepares_nothing() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let store = runtime_store();
    let client = store.connect().await.unwrap();

    client.query_one(REPEATED, &[&"first"]).await.unwrap();
    client.query_one(REPEATED, &[&"second"]).await.unwrap();

    assert_eq!(
        kept_statements(&client).await,
        BTreeMap::from([(REPEATED.to_owned(), 2)])
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn statement_texts_built_from_data_keep_no_more_statements_than_the_capacity() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let store = runtime_store();
    let client = store.connect().await.unwrap();

    for index in 0..STATEMENT_CAPACITY + 16 {
        client
            .query_one(&format!("SELECT {index} AS built_from_data"), &[])
            .await
            .unwrap();
    }

    let session_statements: i64 = client
        .query_one("SELECT count(*) FROM pg_prepared_statements", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        session_statements,
        i64::try_from(STATEMENT_CAPACITY).unwrap()
    );
    let kept = kept_statements(&client).await;
    assert!(
        !kept.contains_key("SELECT 0 AS built_from_data"),
        "{kept:?}"
    );
    let newest = format!("SELECT {} AS built_from_data", STATEMENT_CAPACITY + 15);
    assert_eq!(kept.get(&newest), Some(&1), "{kept:?}");
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_storage_activation_proof_read_on_a_warm_connection_is_one_statement() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let store = runtime_store();
    store
        .require_release1_storage_activation_proof()
        .await
        .unwrap();
    let before = kept_statements(&store.connect().await.unwrap()).await;

    store
        .require_release1_storage_activation_proof()
        .await
        .unwrap();

    let after = kept_statements(&store.connect().await.unwrap()).await;
    let changed = after
        .iter()
        .filter(|(text, runs)| before.get(*text) != Some(*runs))
        .map(|(text, runs)| {
            (
                text.clone(),
                runs - before.get(text).copied().unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        after.len(),
        before.len(),
        "the warm read prepares nothing: {after:?}"
    );
    assert_eq!(changed.len(), 1, "{changed:?}");
    assert!(
        changed[0].0.contains("storyos.storage_activation_proofs"),
        "{changed:?}"
    );
    assert_eq!(changed[0].1, 1, "{changed:?}");
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_returned_connection_keeps_its_statements_and_a_connection_that_does_not_return_takes_them()
 {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let store = runtime_store();
    let first = store.connect().await.unwrap();
    first.query_one(REPEATED, &[&"first"]).await.unwrap();
    let first_backend = backend_identity(&first).await;
    drop(first);

    let returned = store.connect().await.unwrap();
    let returned_backend = backend_identity(&returned).await;
    returned.query_one(REPEATED, &[&"second"]).await.unwrap();
    let returned_statements = kept_statements(&returned).await;
    returned.batch_execute("BEGIN").await.unwrap();
    drop(returned);

    let after_open_transaction = store.connect().await.unwrap();
    let after_open_transaction_backend = backend_identity(&after_open_transaction).await;
    let after_open_transaction_statements = kept_statements(&after_open_transaction).await;
    after_open_transaction
        .query_one(REPEATED, &[&"third"])
        .await
        .unwrap();
    let terminator = store.connect().await.unwrap();
    terminator
        .query_one(TERMINATE, &[&after_open_transaction_backend.0])
        .await
        .unwrap();
    drop(terminator);
    let refused = after_open_transaction
        .query_one(REPEATED, &[&"fourth"])
        .await;
    while !after_open_transaction.is_closed() {
        tokio::task::yield_now().await;
    }
    drop(after_open_transaction);
    let after_close = store.connect().await.unwrap();
    let after_close_backend = backend_identity(&after_close).await;

    let only_identity = BTreeMap::from([(BACKEND_IDENTITY.to_owned(), 1)]);
    assert_eq!(first_backend, returned_backend);
    assert_eq!(
        returned_statements,
        BTreeMap::from([(REPEATED.to_owned(), 2), (BACKEND_IDENTITY.to_owned(), 2)])
    );
    assert_ne!(after_open_transaction_backend, returned_backend);
    assert_eq!(after_open_transaction_statements, only_identity);
    assert!(refused.is_err(), "a terminated backend runs no statement");
    assert_ne!(after_close_backend, after_open_transaction_backend);
    assert_eq!(
        kept_statements(&after_close).await,
        BTreeMap::from([(BACKEND_IDENTITY.to_owned(), 1), (TERMINATE.to_owned(), 1)]),
        "the next checkout reuses the connection of the terminator, with no statement of the closed one"
    );
}
