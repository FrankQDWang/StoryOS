use storyos_application::{
    AgentRunDecisionInspect, AgentRunStatus, NoContractFaults, complete_agent_run, open_agent_run,
};

use crate::agent_run_dispatch::tests::{RuntimeClock, claim_run, stores};
use crate::agent_run_retrieval::tests::{adapter, destination, real_run};

/// A native rejection of the Create, before any stream.
fn rejection(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Sends one Agent Plan AgentRun to a destination that answers with `reply`. Returns the status
/// and the decision that the author reads.
async fn rejected_run(prefix: &str, reply: String) -> (AgentRunStatus, AgentRunDecisionInspect) {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![reply]).await;
    let (scope, run_id) = real_run(&store, &admin, prefix, &base).await;
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    assert_eq!(
        served.await.unwrap(),
        vec!["POST /api/plan/v3/responses HTTP/1.1".to_owned()]
    );
    let run = open_agent_run(&store, &scope, &run_id)
        .await
        .unwrap()
        .unwrap();
    (run.status, run.decision)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_destination_rejection_refuses_the_run_with_a_result_for_the_author() {
    assert_eq!(
        rejected_run(
            "4d1",
            rejection(
                "400 Bad Request",
                r#"{"error":{"code":"InvalidParameter","message":"The parameter is invalid."}}"#,
            ),
        )
        .await,
        (
            AgentRunStatus::Refused,
            AgentRunDecisionInspect::ExecutionRefused {
                capability: "destination_refused".to_owned(),
            },
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_input_above_the_model_window_refuses_the_run_as_a_conversation_too_long() {
    assert_eq!(
        rejected_run(
            "4d2",
            rejection(
                "400 Bad Request",
                r#"{"error":{"code":"InvalidParameter","message":"Total tokens of image and text exceed max message tokens. Request id: 0217","param":"","type":"BadRequest"}}"#,
            ),
        )
        .await,
        (
            AgentRunStatus::Refused,
            AgentRunDecisionInspect::ExecutionRefused {
                capability: "conversation_too_long".to_owned(),
            },
        )
    );
}
