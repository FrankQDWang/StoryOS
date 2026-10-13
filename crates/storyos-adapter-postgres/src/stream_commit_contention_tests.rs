use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    ClaimedAgentRun, DispatchClaim, ModelDispatchStore, ModelProviderAdapter, NextDispatchWork,
    NoContractFaults, complete_agent_run,
};
use storyos_core::{DeploymentDestination, NativeStreamItem, StreamItemRole, StreamItemState};
use tokio_postgres::{Client, NoTls};

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{RuntimeClock, admit_run, claim_run, queued_run, stores};

/// Claims the next Create of `claim`, as the Gateway does before the exchange.
async fn dispatched(store: &PostgresProjectReader, claim: &ClaimedAgentRun) -> DispatchClaim {
    let NextDispatchWork::Request(request) = store.next_dispatch_work(claim).await.unwrap() else {
        panic!("The claimed AgentRun has no request");
    };
    let prepared = FakeDestination.prepare(&request).await.unwrap();
    store
        .commit_dispatch_claim(claim, &request, &prepared.projection)
        .await
        .unwrap()
        .unwrap()
}

/// Cancels the claimed AgentRun, so that the Worker of its next claim does cancellation duties.
async fn cancelled(admin: &Client, claim: ClaimedAgentRun) -> ClaimedAgentRun {
    let fence_token = admin
        .query_one(
            "UPDATE storyos.agent_runs
                SET status = 'cancelled', claim_generation = claim_generation + 1,
                    fence_token = claim_generation + 1, wakeup_pending = true,
                    lease_expires_at = clock_timestamp()
              WHERE run_id = $1::text::uuid
          RETURNING fence_token",
            &[&claim.run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    ClaimedAgentRun {
        fence_token,
        ..claim
    }
}

/// Waits until a backend waits for a row lock that `holder` keeps.
async fn wait_for_lock_wait(admin: &Client, holder: i32) {
    loop {
        let waiting: bool = admin
            .query_one(
                "SELECT EXISTS (
                   SELECT 1 FROM pg_stat_activity
                    WHERE $1 = ANY(pg_blocking_pids(pid)))",
                &[&holder],
            )
            .await
            .unwrap()
            .get(/*idx*/ 0);
        if waiting {
            return;
        }
    }
}

/// The stream commit of one Run and the cancellation duties of another Run of the same Project
/// overlap. Both commit (#1137).
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_stream_commit_beside_the_work_of_another_run_commits() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, streaming) =
        queued_run(&store, "5c1", DeploymentDestination::HostFake).await;
    let other = admit_run(
        &store,
        &scope,
        &chapter_id,
        "5c2",
        DeploymentDestination::HostFake,
        "Cancel this run.",
    )
    .await;
    let streaming = claim_run(&admin, scope.clone(), streaming).await;
    let other = claim_run(&admin, scope, other).await;
    let dispatch = dispatched(&store, &streaming).await;
    dispatched(&store, &other).await;
    let other = cancelled(&admin, other).await;

    // The stream commit reads its Run, then waits for the Attempt row that `holder` locks.
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL").unwrap();
    let (holder, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(connection);
    holder.batch_execute("BEGIN").await.unwrap();
    holder
        .execute(
            "SELECT 1 FROM storyos.model_attempts
              WHERE model_attempt_id = $1::text::uuid FOR UPDATE",
            &[&dispatch.model_attempt_id],
        )
        .await
        .unwrap();
    let holder_pid: i32 = holder
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let events = [NativeStreamItem {
        item_id: "1".to_owned(),
        role: StreamItemRole::Assistant,
        state: StreamItemState::Provisional,
        text: Some("Guard".to_owned()),
        summary: None,
        call_id: None,
        arguments: None,
        refusal: None,
        hosted_report: None,
    }];
    let (appended, ()) = tokio::join!(
        store.append_model_stream_events(&streaming, &dispatch, &events, /*reference*/ None),
        async {
            wait_for_lock_wait(&admin, holder_pid).await;
            complete_agent_run(
                &store,
                &FakeDestination,
                &NoContractFaults,
                &RuntimeClock,
                &other,
            )
            .await
            .unwrap();
            holder.batch_execute("COMMIT").await.unwrap();
        }
    );

    assert_eq!(appended.unwrap(), None);
    let items: String = admin
        .query_one(
            "SELECT payload->>'items' FROM storyos.model_attempts
              WHERE model_attempt_id = $1::text::uuid",
            &[&dispatch.model_attempt_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&items).unwrap()[0]["text"],
        "Guard"
    );
}
