use serde_json::{Value, json};
use storyos_application::{
    AgentRunDecisionInspect, NoContractFaults, complete_agent_run, open_agent_run,
};
use storyos_core::AgentRunStatus;

use crate::agent_run_dispatch::tests::{RuntimeClock, claim_run, stores};
use crate::agent_run_retrieval::tests::{adapter, destination, real_run};
use crate::agent_run_tool_exposure::tests::{finish_stream, sse};
use crate::research_source::tests::calls_stream;

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

/// A stream that reports `reference` and ends with one complete message of `text`.
fn text_stream(reference: &str, text: &str) -> String {
    let message = json!({"id": "msg_01", "type": "message", "role": "assistant",
        "status": "completed", "content": [{"type": "output_text", "text": text}]});
    sse(&[
        json!({"type": "response.created",
               "response": {"id": reference, "status": "in_progress", "output": []}}),
        json!({"type": "response.output_item.done", "output_index": 0, "item": message}),
        json!({"type": "response.completed", "response": {
            "id": reference, "status": "completed", "output": [message],
            "usage": {"input_tokens": 40, "output_tokens": 12}}}),
    ])
}

/// Sends one Agent Plan AgentRun whose responses are `replies`, which can name the target block
/// of the Run. Returns the settled Run, its last decision Attempt with each stored item, the
/// count of its Proposals, and the refusal of each other ToolCall than `finish`.
async fn answered_run(prefix: &str, replies: impl FnOnce(&str) -> Vec<String>) -> Value {
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
    let (base, served) = destination(replies(&block)).await;
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
                                     WHERE proposal.project_id = run.project_id),
                      'refusals', (SELECT jsonb_agg(call.refusal)
                                     FROM storyos.tool_calls AS call
                                    WHERE call.run_id = run.run_id
                                      AND call.function_name <> 'finish'))::text
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS attempt
                 ON attempt.run_id = run.run_id AND attempt.attempt_role = 'decision'
                AND attempt.decision_position = run.active_decision_position
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(0)).unwrap()
}

/// The facts of a Run whose closing RunStep completed with no Decision and no Proposal, and
/// that keeps its one message item with `text`. `escaped` names the escaped fields of the item.
/// `refusals` are the refusals of the other calls than `finish`.
fn settled_without_decision(text: &str, escaped: &[&str], refusals: Value) -> Value {
    let mut item = json!({"item_id": "msg_01", "role": "assistant", "state": "complete",
        "phase": "complete", "text": text, "summary": null, "call_id": null, "arguments": null,
        "refusal": null, "native_type": null, "name": null});
    if !escaped.is_empty() {
        item["escaped_fields"] = json!(escaped);
    }
    json!({"status": "completed", "settlement": null, "decision_id": null,
           "dispatch_state": "settled", "decision": null, "items": [item], "proposals": 0,
           "refusals": refusals})
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_decision_text_with_u0000_settles_the_run_with_no_decision_and_no_proposal() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let advisory = answered_run("4d3", |_| {
        vec![
            finish_stream("resp_4d3"),
            text_stream("resp_4d3_closing", "Keep\0it."),
        ]
    })
    .await;
    // The arguments are JSON text, so the candidate text holds U+0000 through an escape. The
    // model reads the refusal of the call, and its next `finish` call counts.
    let prose = answered_run("4d4", |block| {
        let arguments = format!(
            r#"{{"summary":"Shorter.","changes":[{{"block_id":"{block}","candidate_text":"Rain\u0000hit.","explanation":"Shorter."}}]}}"#
        );
        vec![
            calls_stream(
                "resp_4d4",
                &[
                    ("call_propose", "propose_edit", &arguments),
                    ("call_finish", "finish", "{}"),
                ],
            ),
            finish_stream("resp_4d4_2"),
            text_stream("resp_4d4_closing", "Keep\0it."),
        ]
    })
    .await;

    assert_eq!(
        (advisory, prose),
        (
            settled_without_decision(r"Keep\u0000it.", &["text"], Value::Null),
            settled_without_decision(r"Keep\u0000it.", &["text"], json!(["nul_character"])),
        )
    );
}
