use serde_json::json;
use storyos_application::{NoContractFaults, SetProjectToolEnablementInput, complete_agent_run};
use storyos_core::{
    EXA_TOOL_REGISTRATION, EnabledTool, ExposureFacts, ExposureSnapshot, NativeStreamItem,
    RESEARCH_TOOL_OFFER, StreamItemRole, StreamItemState, ToolCallBatch, ToolCompatibilityDecision,
    ToolEnablement, ToolExposure, ToolRegistrationLifecycle, validate_tool_call_batch,
};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::{RuntimeClock, claim_run, stores};
use crate::agent_run_retrieval::tests::{adapter, destination, real_run};
use crate::command_sequence::tests::{SET_PROJECT_TOOL_ENABLEMENT, issued};

const SEARCH_ARGUMENTS: &str = r#"{"query":"tide tables of 1890","result_count":null}"#;
const SIX_URLS: &str = r#"{"urls":["https://e.org/1","https://e.org/2","https://e.org/3","https://e.org/4","https://e.org/5","https://e.org/6"]}"#;

/// A stream that reports `reference`, one complete message, and one complete function call.
fn text_and_call_stream(reference: &str, name: &str, arguments: &str) -> String {
    let message = json!({"id": "msg_01", "type": "message", "role": "assistant",
        "status": "completed", "content": [{"type": "output_text", "text": "Looking it up."}]});
    let call = json!({"id": "fc_01", "type": "function_call", "status": "completed",
        "call_id": "call_01", "name": name, "arguments": arguments});
    let events = [
        json!({"type": "response.created",
               "response": {"id": reference, "status": "in_progress", "output": []}}),
        json!({"type": "response.output_item.done", "output_index": 0, "item": message}),
        json!({"type": "response.output_item.done", "output_index": 1, "item": call}),
        json!({"type": "response.completed", "response": {
            "id": reference, "status": "completed", "output": [message, call],
            "usage": {"input_tokens": 90, "output_tokens": 30}}}),
    ];
    let mut reply = String::from(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
    );
    for event in events {
        reply.push_str(&format!("data: {event}\n\n"));
    }
    reply
}

/// Enables the research Tool of the Project, so that the next request exposes it.
async fn enable_research_tool(
    store: &crate::PostgresProjectReader,
    scope: &storyos_application::ProjectScope,
    suffix: u16,
) {
    let call = issued(
        store,
        scope,
        suffix,
        &SET_PROJECT_TOOL_ENABLEMENT,
        SetProjectToolEnablementInput {
            registration_id: "web_research".to_owned(),
            requested: ToolEnablement::Enabled,
            offer: Some(&RESEARCH_TOOL_OFFER),
        },
    )
    .await;
    store
        .set_project_tool_enablement(&call.envelope, &call.input)
        .await
        .unwrap();
}

/// The ToolCall rows of the Run, without their row identity.
async fn tool_call_rows(admin: &Client, run_id: &str) -> Vec<serde_json::Value> {
    admin
        .query(
            "SELECT (to_jsonb(call) - 'tool_call_id' - 'owner_user_id' - 'project_id')::text
               FROM storyos.tool_calls AS call
              WHERE run_id = $1::text::uuid ORDER BY call_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| serde_json::from_str(row.get::<_, &str>(0)).unwrap())
        .collect()
}

/// The status and settlement of the Run, and the decision identity, dispatch state, decision
/// record, batch evidence, and item count of its decision Attempt.
async fn decision_facts(admin: &Client, run_id: &str) -> serde_json::Value {
    admin
        .query_one(
            "SELECT jsonb_build_object(
                      'status', run.status, 'settlement', run.settlement,
                      'model_attempt_id', attempt.model_attempt_id,
                      'decision_id', attempt.decision_id,
                      'dispatch_state', attempt.dispatch_state,
                      'decision', attempt.payload->'decision',
                      'tool_call_batch', attempt.payload->'tool_call_batch',
                      'item_count', jsonb_array_length(attempt.payload->'items'))::text
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS attempt
                 ON attempt.run_id = run.run_id AND attempt.attempt_role = 'decision'
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .map(|row| serde_json::from_str(row.get::<_, &str>(0)).unwrap())
        .unwrap()
}

/// The one member of the batch of a complete `web_search` call, validated against the pinned
/// research Exposure.
fn validated_search_call() -> storyos_core::ValidatedToolCall {
    let exposure = ToolExposure::compute(&ExposureFacts {
        enabled: &[EnabledTool {
            registration_revision: EXA_TOOL_REGISTRATION.revision.to_owned(),
            lifecycle: ToolRegistrationLifecycle::Active,
            compatibility: Some(ToolCompatibilityDecision::Compatible),
        }],
        model_registration_revision: storyos_core::AGENT_PLAN_REGISTRATION.revision,
    });
    let item = NativeStreamItem {
        item_id: "fc_01".to_owned(),
        role: StreamItemRole::Tool,
        state: StreamItemState::Complete,
        text: None,
        summary: None,
        call_id: Some("call_01".to_owned()),
        arguments: Some(SEARCH_ARGUMENTS.to_owned()),
        refusal: None,
        native_type: Some("function_call".to_owned()),
        name: Some("web_search".to_owned()),
    };
    match validate_tool_call_batch(
        &[item],
        &ExposureSnapshot::decode(Some(&exposure.snapshot())).unwrap(),
    ) {
        ToolCallBatch::Valid(mut calls) => calls.remove(0),
        ToolCallBatch::Invalid(refusal) => panic!("{}", refusal.label()),
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_durable_function_request_decision_derives_one_pending_tool_call_once() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![text_and_call_stream(
        "resp_f75",
        "web_search",
        SEARCH_ARGUMENTS,
    )])
    .await;
    let (scope, run_id) = real_run(&store, &admin, "f75", &base).await;
    enable_research_tool(&store, &scope, 0xf75a).await;
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let facts = decision_facts(&admin, &run_id).await;
    let rows = tool_call_rows(&admin, &run_id).await;
    let decision_id = facts["decision_id"].as_str().unwrap().to_owned();
    let attempt_id = facts["model_attempt_id"].as_str().unwrap().to_owned();
    let call = validated_search_call();
    // A second delivery of the same batch, and a row of another identity for the same call.
    crate::agent_run_tool_call::insert_tool_calls(
        &admin,
        &claim,
        &crate::agent_run_tool_call::ToolCallOwner {
            run_id: &run_id,
            decision_position: "0",
            model_attempt_id: &attempt_id,
            decision_id: &decision_id,
        },
        std::slice::from_ref(&call),
    )
    .await
    .unwrap();
    let second_identity = admin
        .execute(
            "INSERT INTO storyos.tool_calls
               SELECT owner_user_id, project_id, gen_random_uuid(), run_id, decision_position,
                      model_attempt_id, decision_id, call_id, 1, caller_route, function_name,
                      registration_id, tool_registration_revision, tool_spec_revision,
                      arguments, argument_digest, state
                 FROM storyos.tool_calls WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap_err();
    let exported = crate::project_archive_build::collect_exportable_families(&admin, &scope)
        .await
        .unwrap()
        .into_iter()
        .find(|family| family.table == "tool_calls")
        .map(|family| family.rows_json.contains("\"call_id\":\"call_01\""));

    assert_eq!(
        (
            facts,
            rows,
            tool_call_rows(&admin, &run_id).await.len(),
            second_identity.code().map(|code| code.code().to_owned()),
            crate::agent_run_tool_call::pending_tool_call_exists(&admin, &claim)
                .await
                .unwrap(),
            exported,
            served.await.unwrap(),
        ),
        (
            json!({
                "status": "refused",
                "settlement": {"kind": "execution_refused", "capability": "research_tool_unavailable"},
                "model_attempt_id": attempt_id,
                "decision_id": decision_id,
                "dispatch_state": "settled",
                "decision": {
                    "kind": "tool_request",
                    "decision_id": decision_id,
                    "selected": true,
                    "call_ids": ["call_01"],
                    "authoritative": false,
                    "advances_continuation": false
                },
                "tool_call_batch": {"kind": "valid", "call_ids": ["call_01"]},
                "item_count": 2
            }),
            vec![json!({
                "run_id": run_id,
                "decision_position": 0,
                "model_attempt_id": attempt_id,
                "decision_id": decision_id,
                "call_id": "call_01",
                "call_position": 0,
                "caller_route": "model",
                "function_name": "web_search",
                "registration_id": "web_research",
                "tool_registration_revision": EXA_TOOL_REGISTRATION.revision,
                "tool_spec_revision": "storyos.tool.web-search.v1",
                "arguments": {"query": "tide tables of 1890", "result_count": null},
                "argument_digest": call.argument_digest(),
                "state": "pending"
            })],
            1,
            Some("23505".to_owned()),
            true,
            Some(true),
            vec!["POST /api/plan/v3/responses HTTP/1.1".to_owned()],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_intake_violation_rejects_the_batch_and_keeps_the_attempt_as_evidence() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![text_and_call_stream(
        "resp_f76",
        "web_fetch",
        SIX_URLS,
    )])
    .await;
    let (scope, run_id) = real_run(&store, &admin, "f76", &base).await;
    enable_research_tool(&store, &scope, 0xf76a).await;
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let facts = decision_facts(&admin, &run_id).await;
    let attempt_id = facts["model_attempt_id"].clone();

    assert_eq!(
        (
            facts,
            tool_call_rows(&admin, &run_id).await,
            crate::agent_run_tool_call::pending_tool_call_exists(&admin, &claim)
                .await
                .unwrap(),
            served.await.unwrap(),
        ),
        (
            json!({
                "status": "refused",
                "settlement": {"kind": "execution_refused", "capability": "research_tool_unavailable"},
                "model_attempt_id": attempt_id,
                "decision_id": null,
                "dispatch_state": "settled",
                "decision": null,
                "tool_call_batch": {"kind": "invalid", "refusal": "intake_url_count"},
                "item_count": 2
            }),
            Vec::<serde_json::Value>::new(),
            false,
            vec!["POST /api/plan/v3/responses HTTP/1.1".to_owned()],
        )
    );
}
