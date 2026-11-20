use storyos_application::{
    AgentRunWorkStore, ClaimedAgentRun, CompleteAgentRun, ModelDispatchStore, NoContractFaults,
    complete_agent_run,
};
use storyos_core::ModelAdapter;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;
use tokio_postgres::{Client, NoTls};

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_lease::SteppedClock;
use crate::shared_tests::agent_run_retrieval::{adapter, real_run};
use crate::shared_tests::agent_run_tool_exposure::{advisory_stream, finish_stream};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::model_gateway::claim_run;

/// Serves one streamed Create. It reports the reference, then holds the response until the test
/// releases it. A dropped release closes the stream with no terminal event. A released response
/// ends with a `finish` call, and the closing request gets the final answer.
async fn held_destination(
    reference: &'static str,
) -> (String, oneshot::Receiver<()>, oneshot::Sender<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/api/plan/v3", listener.local_addr().unwrap());
    let (received, received_rx) = oneshot::channel();
    let (release, release_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let mut stream = accepted(&listener).await;
        let created = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n\
             data: {{\"type\":\"response.created\",\"response\":{{\"id\":\"{reference}\",\
             \"status\":\"in_progress\",\"output\":[]}}}}\n\n"
        );
        stream.write_all(created.as_bytes()).await.unwrap();
        let _ = received.send(());
        if release_rx.await.is_ok() {
            let finish = finish_stream(reference);
            let (_head, events) = finish.split_once("\r\n\r\n").unwrap();
            stream.write_all(events.as_bytes()).await.unwrap();
            drop(stream);
            let mut closing = accepted(&listener).await;
            let answer = advisory_stream(&format!("{reference}_closing"));
            closing.write_all(answer.as_bytes()).await.unwrap();
        }
    });
    (base, received_rx, release)
}

/// The next connection of `listener`, after the head of its request.
async fn accepted(listener: &tokio::net::TcpListener) -> tokio::net::TcpStream {
    let (mut stream, _) = listener.accept().await.unwrap();
    let mut request = Vec::new();
    let mut chunk = [0_u8; 4096];
    while !request
        .windows(/*size*/ 4)
        .any(|window| window == b"\r\n\r\n")
    {
        let read = stream.read(&mut chunk).await.unwrap();
        request.extend_from_slice(&chunk[..read]);
    }
    stream
}

/// Whether the lease of the Run ends in the future, and its fence token.
async fn lease(admin: &Client, run_id: &str) -> (bool, i64) {
    let row = admin
        .query_one(
            "SELECT COALESCE(lease_expires_at > clock_timestamp(), false), fence_token
               FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    (row.get(/*idx*/ 0), row.get(/*idx*/ 1))
}

/// Ends the lease of the Run now, as the lease period does when no Worker renews it.
async fn expire(admin: &Client, run_id: &str) {
    admin
        .execute(
            "UPDATE storyos.agent_runs SET lease_expires_at = clock_timestamp()
              WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
}

/// Claims as another Agent Plan Worker. An admin transaction locks each other AgentRun, so that
/// the claim can take only this Run and changes no other test data.
async fn claim_of_another_worker(store: &PostgresProjectReader, run_id: &str) -> Option<String> {
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL").unwrap();
    let (others, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(connection);
    others.batch_execute("BEGIN").await.unwrap();
    others
        .execute(
            "SELECT 1 FROM storyos.agent_runs WHERE run_id <> $1::text::uuid FOR UPDATE",
            &[&run_id],
        )
        .await
        .unwrap();
    let claimed = store
        .claim_next_agent_run(&[ModelAdapter::VolcengineAgentPlanResponses])
        .await
        .unwrap();
    others.batch_execute("ROLLBACK").await.unwrap();
    claimed.map(|claim| claim.run_id)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_request_longer_than_the_lease_keeps_its_claim() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (base, received, release) = held_destination("resp_2b1").await;
    let (scope, run_id) = real_run(&store, &admin, "2b1", &base).await;
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    let clock = SteppedClock::new();
    let adapter = adapter();

    let (completed, taken) = tokio::join!(
        complete_agent_run(&store, &adapter, &NoContractFaults, &clock, &claim),
        async {
            received.await.unwrap();
            clock.waited(1).await;
            // The lease period passes twice while the request is held.
            let mut taken = Vec::new();
            for renewal in 2..=3 {
                expire(&admin, &run_id).await;
                clock.step();
                clock.waited(renewal).await;
                taken.push(claim_of_another_worker(&store, &run_id).await);
            }
            release.send(()).unwrap();
            taken
        }
    );

    assert_eq!(completed.unwrap(), CompleteAgentRun::Settled);
    assert_eq!(taken, vec![None, None]);
    let status: String = admin
        .query_one(
            "SELECT status FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    assert_eq!(
        (status, lease(&admin, &run_id).await.1),
        ("completed".to_owned(), claim.fence_token)
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_stopped_worker_stops_renewing_and_recovery_takes_the_claim() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (base, received, release) = held_destination("resp_2b2").await;
    let (scope, run_id) = real_run(&store, &admin, "2b2", &base).await;
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    let clock = SteppedClock::new();
    let adapter = adapter();

    // The Worker stops while its request is held.
    tokio::select! {
        _ = complete_agent_run(&store, &adapter, &NoContractFaults, &clock, &claim) => {
            panic!("the request is still held");
        }
        () = async {
            received.await.unwrap();
            clock.waited(1).await;
        } => {}
    }
    drop(release);
    expire(&admin, &run_id).await;
    clock.step();
    let (renewed, _) = lease(&admin, &run_id).await;
    assert!(!renewed);

    let taken = claim_of_another_worker(&store, &run_id).await;
    let (_, fence_token) = lease(&admin, &run_id).await;
    assert_eq!(
        (taken, fence_token > claim.fence_token),
        (Some(run_id), true)
    );
    let stale = ClaimedAgentRun {
        fence_token: claim.fence_token,
        ..claim
    };
    assert_eq!(store.renew_agent_run_lease(&stale).await.unwrap(), None);
}
