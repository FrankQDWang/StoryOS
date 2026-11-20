use serde_json::json;
use storyos_application::{ConversationSelection, NoContractFaults, complete_agent_run};
use storyos_core::{
    BoundTool, DeploymentDestination, EXA_TOOL_REGISTRATION, ExposureFacts, RESEARCH_TOOL_OFFER,
    ToolCompatibilityDecision, ToolExposure, ToolRegistrationLifecycle,
};
use tokio_postgres::Client;

use crate::shared_tests::agent_run_retrieval::{
    adapter, broken_stream, destination, real_run, recovery, research_run,
};
use crate::shared_tests::agent_run_tool_exposure::{
    ARGUMENTS, QuarantineBeforeClaim, call, finish_then_advisory, function_call_stream,
    host_exposure,
};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::model_gateway::{RuntimeClock, admit_run_in, claim_run, queued_run};

/// The stored complete response that a Retrieve reads: one complete function call.
fn stored_function_call(reference: &str) -> String {
    let body =
        json!({"id": reference, "status": "completed", "output": [call(ARGUMENTS, "completed")],
        "usage": {"input_tokens": 90, "output_tokens": 30}})
        .to_string();
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// The Exposure of the research Tool and the Host Tools on the current Agent Plan Registration,
/// for a Run with Memory use.
fn research_exposure() -> ToolExposure {
    ToolExposure::compute(&ExposureFacts {
        bound: &[BoundTool {
            registration_revision: EXA_TOOL_REGISTRATION.revision.to_owned(),
            lifecycle: ToolRegistrationLifecycle::Active,
            compatibility: Some(ToolCompatibilityDecision::Compatible),
        }],
        model_registration_revision: storyos_core::AGENT_PLAN_REGISTRATION.revision,
        skill_catalog: false,
        memory_use: true,
        hybrid_search: false,
        run_kind: storyos_core::AgentRunKind::Author,
    })
}

/// The `tools` field of a request body that declares `exposure`.
fn declared(exposure: &ToolExposure) -> serde_json::Value {
    json!(
        exposure
            .declarations()
            .iter()
            .map(|declaration| json!({
                "type": "function",
                "name": declaration.name,
                "description": declaration.description,
                "parameters": declaration.parameters,
                "strict": true,
            }))
            .collect::<Vec<_>>()
    )
}

/// The status and settlement of the Run, and the input mapping, prior binding, Step Snapshot,
/// sent body, and items of its decision Attempt.
async fn attempt_facts(admin: &Client, run_id: &str) -> [Option<serde_json::Value>; 7] {
    let row = admin
        .query_one(
            "SELECT to_jsonb(run.status)::text, run.settlement::text,
                    (attempt.payload->'continuation'->'input_mapping')::text,
                    (attempt.payload->'wire'->'prior_continuation_binding_id')::text,
                    (attempt.payload->'step_snapshot'->'tool_exposure')::text,
                    attempt.payload->'wire'->>'serialized_payload',
                    (attempt.payload->'items')::text
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS attempt
                 ON attempt.run_id = run.run_id AND attempt.attempt_role = 'decision'
                AND attempt.decision_position = 0
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    let parsed = |index: usize| {
        row.get::<_, Option<String>>(index)
            .map(|text| serde_json::from_str(&text).unwrap())
    };
    [0, 1, 2, 3, 4, 5, 6].map(parsed)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_bound_tool_is_declared_on_a_new_transport_and_its_function_call_settles_the_run() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let mut replies = finish_then_advisory("resp_t1").to_vec();
    replies.push(function_call_stream("resp_t2"));
    // The continuation RunStep answers the refused ToolCall.
    replies.extend(finish_then_advisory("resp_t3"));
    let (base, served) = destination(replies).await;
    let plan = DeploymentDestination::VolcengineAgentPlan {
        qualification_authorization: None,
    };
    let (scope, chapter_id, first) = queued_run(&store, "f71", plan.clone()).await;
    admin
        .execute(
            "UPDATE storyos.processing_destination_identity_evidence_revisions
                SET endpoint = $2
              WHERE project_id = $1::text::uuid AND evidence_kind = 'volcengine_agent_plan_boundary'",
            &[&scope.project_id.as_ref(), &base],
        )
        .await
        .unwrap();
    let claim = claim_run(&admin, scope.clone(), first.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    // The deployment starts to offer the research destination between the two requests of one
    // Conversation, so the second createAgentRun binds it.
    let second = admit_run_in(
        &store,
        &scope,
        &chapter_id,
        "f72",
        plan,
        "Check the tide tables of 1890.",
        ConversationSelection::Existing {
            conversation_id: "018f0000-0000-7001-8000-00000006f716".to_owned(),
        },
        Some(&RESEARCH_TOOL_OFFER),
    )
    .await;
    let claim = claim_run(&admin, scope, second.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let exposure = research_exposure();
    let [
        status,
        settlement,
        mapping,
        prior_binding,
        snapshot,
        body,
        items,
    ] = attempt_facts(&admin, &second).await;
    let body = body.unwrap();
    let declared: Vec<serde_json::Value> = exposure
        .declarations()
        .into_iter()
        .map(|declaration| {
            json!({
                "type": "function",
                "name": declaration.name,
                "description": declaration.description,
                "parameters": declaration.parameters,
                "strict": true,
            })
        })
        .collect();

    assert_eq!(
        (
            attempt_facts(&admin, &first).await[2..5].to_vec(),
            status,
            settlement,
            mapping,
            prior_binding,
            snapshot,
            body["tools"].clone(),
            body["tool_choice"].clone(),
            body.get("previous_response_id").cloned(),
            items,
            served.await.unwrap(),
        ),
        (
            vec![
                Some(json!("none")),
                Some(serde_json::Value::Null),
                Some(host_exposure().snapshot()),
            ],
            Some(json!("completed")),
            None,
            Some(json!("new_transport")),
            Some(serde_json::Value::Null),
            Some(exposure.snapshot()),
            json!(declared),
            json!("required"),
            None,
            Some(json!([{
                "item_id": "fc_01",
                "role": "tool",
                "state": "complete",
                "phase": "complete",
                "text": null,
                "summary": null,
                "call_id": "call_01",
                "arguments": ARGUMENTS,
                "refusal": null,
                "native_type": "function_call",
                "name": "web_search"
            }])),
            vec!["POST /api/plan/v3/responses HTTP/1.1".to_owned(); 5],
        )
    );
}

/// The result label and items of the retrieval Attempt of the Run.
async fn retrieval_facts(admin: &Client, run_id: &str) -> (Option<String>, Option<String>) {
    let row = admin
        .query_one(
            "SELECT payload->>'result', payload->'items'->0->>'name'
               FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND attempt_role = 'retrieval'",
            &[&run_id],
        )
        .await
        .unwrap();
    (row.get(0), row.get(1))
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_retrieved_call_of_an_unexposed_name_is_a_refused_call_and_the_run_continues() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let mut replies = vec![broken_stream("resp_f73"), stored_function_call("resp_f73")];
    replies.extend(finish_then_advisory("resp_f73b"));
    let (base, served) = destination(replies).await;
    let (scope, run_id) = real_run(&store, &admin, "f73", &base).await;
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();

    assert_eq!(
        (
            recovery(&admin, &run_id).await,
            retrieval_facts(&admin, &run_id).await,
            attempt_facts(&admin, &run_id).await[6].clone(),
            served.await.unwrap(),
        ),
        (
            [
                Some("completed".to_owned()),
                None,
                Some("Keep it.".to_owned()),
                Some("settled".to_owned()),
                None,
                Some("true".to_owned()),
                Some("decision:true,retrieval:true,decision:true,decision:true".to_owned()),
            ],
            (
                Some("complete_selected".to_owned()),
                Some("web_search".to_owned()),
            ),
            // The provisional item of the lost stream stays before the retrieved call.
            Some(json!([{
                "item_id": "msg_01",
                "role": "assistant",
                "state": "provisional",
                "phase": "provisional",
                "text": "{\"kind\"",
                "summary": null,
                "call_id": null,
                "arguments": null,
                "refusal": null,
                "native_type": null,
                "name": null
            }, {
                "item_id": "fc_01",
                "role": "tool",
                "state": "complete",
                "phase": "complete",
                "text": null,
                "summary": null,
                "call_id": "call_01",
                "arguments": ARGUMENTS,
                "refusal": null,
                "native_type": "function_call",
                "name": "web_search"
            }])),
            vec![
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
                "GET /api/plan/v3/responses/resp_f73 HTTP/1.1".to_owned(),
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
            ],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_exposure_change_before_the_claim_sends_the_request_again_with_the_current_exposure() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (base, served) = destination(finish_then_advisory("resp_f79").to_vec()).await;
    let (scope, run_id) = research_run(&store, &admin, "f79", &base).await;
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let route = QuarantineBeforeClaim {
        admin: &admin,
        successor: false,
        quarantined: std::sync::atomic::AtomicBool::new(/*v*/ false),
        prepared: std::sync::Mutex::new(Vec::new()),
    };
    complete_agent_run(&store, &route, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    crate::shared_tests::agent_run_tool_call::reactivate_research_revision(&admin).await;
    let attempts: i64 = admin
        .query_one(
            "SELECT count(*) FROM storyos.model_attempts WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(0);
    let [status, _, _, _, snapshot, body, _] = attempt_facts(&admin, &run_id).await;
    let prepared = route.prepared.into_inner().unwrap();

    assert_eq!(
        (
            prepared,
            attempts,
            status,
            snapshot,
            body.and_then(|body| body.get("tools").cloned()),
            served.await.unwrap(),
        ),
        (
            vec![
                (
                    research_exposure().snapshot(),
                    Some(declared(&research_exposure()))
                ),
                (host_exposure().snapshot(), Some(declared(&host_exposure()))),
                (host_exposure().snapshot(), Some(declared(&host_exposure()))),
            ],
            2,
            Some(json!("completed")),
            Some(host_exposure().snapshot()),
            Some(declared(&host_exposure())),
            vec!["POST /api/plan/v3/responses HTTP/1.1".to_owned(); 2],
        )
    );
}
