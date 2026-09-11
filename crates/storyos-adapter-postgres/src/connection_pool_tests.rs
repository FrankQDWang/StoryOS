use std::time::SystemTime;

use storyos_application::{ProjectId, ProjectScope, UserId};

use crate::PostgresProjectReader;

const USER: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT: &str = "018f0000-0000-7001-8000-000000000002";

// A PID can be reused; the pair with `backend_start` is unique.
async fn backend_identity(client: &tokio_postgres::Client) -> (i32, SystemTime) {
    let row = client
        .query_one(
            "SELECT pid, backend_start FROM pg_stat_activity WHERE pid = pg_backend_pid()",
            &[],
        )
        .await
        .unwrap();
    (row.get(0), row.get(1))
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_settled_connection_is_reused_and_an_unsettled_transaction_is_not() {
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
