use serde_json::{Value, json};
use storyos_application::{
    AgentRunDecisionInspect, NoContractFaults, complete_agent_run, open_agent_run,
};
use storyos_core::AgentRunStatus;

use crate::agent_run_dispatch::tests::{RuntimeClock, claim_run, stores};
use crate::agent_run_retrieval::tests::{adapter, destination, real_run};
use crate::agent_run_tool_exposure::tests::sse;

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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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

/// Sends one Agent Plan AgentRun whose one response is a complete message of `text`, which can
/// name the target block of the Run. Returns the settled Run, its decision Attempt with each
/// stored item, and the count of its Proposals, and the sent text.
async fn answered_run(prefix: &str, text: impl FnOnce(&str) -> String) -> (Value, String) {
    let (store, admin) = stores().await;
    let (scope, run_id) = real_run(&store, &admin, prefix, "http://127.0.0.1:9/api/plan/v3").await;
    let block: String = admin
        .query_one(
            "SELECT payload #>> '{operation_requirement,proposal_target_block_ids,0}'
               FROM storyos.operation_requirements
              WHERE run_id = $1::text::uuid AND decision_position = 0",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let reference = format!("resp_{prefix}");
    let text = text(&block);
    let message = json!({"id": "msg_01", "type": "message", "role": "assistant",
        "status": "completed", "content": [{"type": "output_text", "text": text}]});
    let (base, served) = destination(vec![sse(&[
        json!({"type": "response.created",
               "response": {"id": reference, "status": "in_progress", "output": []}}),
        json!({"type": "response.output_item.done", "output_index": 0, "item": message}),
        json!({"type": "response.completed", "response": {
            "id": reference, "status": "completed", "output": [message],
            "usage": {"input_tokens": 40, "output_tokens": 12}}}),
    ])])
    .await;
    admin
        .execute(
            "UPDATE storyos.processing_destination_identity_evidence_revisions
                SET endpoint = $2 WHERE project_id = $1::text::uuid AND endpoint IS NOT NULL",
            &[&scope.project_id.as_ref(), &base],
        )
        .await
        .unwrap();
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    served.await.unwrap();
    let row = admin
        .query_one(
            "SELECT jsonb_build_object(
                      'status', run.status, 'settlement', run.settlement,
                      'decision_id', attempt.decision_id,
                      'dispatch_state', attempt.dispatch_state,
                      'decision', attempt.payload -> 'decision',
                      'items', attempt.payload -> 'items',
                      'proposals', (SELECT count(*) FROM storyos.proposals AS proposal
                                     WHERE proposal.project_id = run.project_id))::text
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS attempt
                 ON attempt.run_id = run.run_id AND attempt.attempt_role = 'decision'
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    (serde_json::from_str(row.get::<_, &str>(0)).unwrap(), text)
}

/// The facts of a Run that completed with no Decision and no Proposal, and that keeps its one
/// message item with `text`. `escaped` names the escaped fields of the item.
fn settled_without_decision(text: &str, escaped: &[&str]) -> Value {
    let mut item = json!({"item_id": "msg_01", "role": "assistant", "state": "complete",
        "phase": "complete", "text": text, "summary": null, "call_id": null, "arguments": null,
        "refusal": null, "native_type": null, "name": null});
    if !escaped.is_empty() {
        item["escaped_fields"] = json!(escaped);
    }
    json!({"status": "completed", "settlement": null, "decision_id": null,
           "dispatch_state": "settled", "decision": null, "items": [item], "proposals": 0})
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_decision_text_with_u0000_settles_the_run_with_no_decision_and_no_proposal() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (advisory, _) = answered_run("4d3", |_| "Keep\0it.".to_owned()).await;
    // The prose change is JSON text, so its candidate text holds U+0000 through an escape.
    let (prose, prose_text) = answered_run("4d4", |block| {
        format!(
            r#"{{"kind":"prose_change","summary":"Shorter.","changes":[{{"block_id":"{block}","candidate_text":"Rain\u0000hit.","explanation":"Shorter."}}]}}"#
        )
    })
    .await;

    assert_eq!(
        (advisory, prose),
        (
            settled_without_decision(r"Keep\u0000it.", &["text"]),
            settled_without_decision(&prose_text, &[]),
        )
    );
}
