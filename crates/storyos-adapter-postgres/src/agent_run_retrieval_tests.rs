use storyos_application::{CompleteAgentRunError, NoContractFaults, complete_agent_run};

use crate::shared_tests::agent_run_retrieval::{
    AfterExchange, adapter, broken_stream, destination, real_run, recovery,
};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::model_gateway::RuntimeClock;
use crate::shared_tests::model_gateway::claim_run;

/// The stored complete response that a Retrieve reads: one complete `finish` call.
pub(crate) fn stored_finish(reference: &str) -> String {
    let body = format!(
        r#"{{"id":"{reference}","status":"completed","output":[{{"id":"fc_finish","type":"function_call","status":"completed","call_id":"call_finish_{reference}","name":"finish","arguments":"{{}}"}}],"usage":{{"input_tokens":40,"output_tokens":4}}}}"#
    );
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_restarted_worker_retrieves_the_committed_reference_once_and_settles_the_result() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![
        broken_stream("resp_c41"),
        stored_finish("resp_c41"),
        crate::shared_tests::agent_run_tool_exposure::advisory_stream("resp_c41_closing"),
    ])
    .await;
    let (scope, run_id) = real_run(&store, &admin, "c41", &base).await;
    let crash = AfterExchange {
        admin: &admin,
        statement: format!(
            "UPDATE storyos.agent_runs SET fence_token = fence_token + 1 WHERE run_id = '{run_id}'"
        ),
    };
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let crashed =
        complete_agent_run(&store, &crash, &NoContractFaults, &RuntimeClock, &claim).await;
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();

    assert_eq!(
        (
            crashed
                .err()
                .map(|error| matches!(error, CompleteAgentRunError::StaleFence)),
            recovery(&admin, &run_id).await,
            served.await.unwrap(),
        ),
        (
            Some(true),
            [
                Some("completed".to_owned()),
                None,
                Some("Keep it.".to_owned()),
                Some("settled".to_owned()),
                None,
                Some("true".to_owned()),
                Some("decision:true,retrieval:true,decision:true".to_owned()),
            ],
            vec![
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
                "GET /api/plan/v3/responses/resp_c41 HTTP/1.1".to_owned(),
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
            ],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_route_that_is_no_longer_admitted_keeps_the_result_unknown_and_sends_no_retrieve() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![broken_stream("resp_c44")]).await;
    let (scope, run_id) = real_run(&store, &admin, "c44", &base).await;
    let off = AfterExchange {
        admin: &admin,
        statement: format!(
            "SET session_replication_role = replica;
             UPDATE storyos.project_policy_revisions SET availability = 'unavailable'
              WHERE project_id = '{}';
             SET session_replication_role = DEFAULT;",
            scope.project_id.as_ref()
        ),
    };
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    complete_agent_run(&store, &off, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();

    assert_eq!(
        (recovery(&admin, &run_id).await, served.await.unwrap()),
        (
            [
                // The successor gate then pauses the Run, because assistance is off.
                Some("paused".to_owned()),
                Some(
                    r#"{"kind": "unknown_create_successor", "reason": "authority_unavailable"}"#
                        .to_owned()
                ),
                None,
                Some("kept_unknown".to_owned()),
                Some("destination_mismatch".to_owned()),
                Some("false".to_owned()),
                Some("decision:true".to_owned()),
            ],
            vec!["POST /api/plan/v3/responses HTTP/1.1".to_owned()],
        )
    );
}
