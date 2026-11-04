use serde_json::json;
use storyos_application::{
    ConversationSelection, NoContractFaults, SetProjectToolEnablementInput, complete_agent_run,
};
use storyos_core::{
    DeploymentDestination, EXA_TOOL_REGISTRATION, EnabledTool, ExposureFacts, RESEARCH_TOOL_OFFER,
    ToolCompatibilityDecision, ToolEnablement, ToolExposure, ToolRegistrationLifecycle,
};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::{RuntimeClock, admit_run_in, claim_run, queued_run, stores};
use crate::agent_run_retrieval::tests::{adapter, destination};
use crate::command_sequence::tests::{SET_PROJECT_TOOL_ENABLEMENT, issued};

const ADVISORY: &str = r#"{"kind":"advisory","text":"Keep it."}"#;
const ARGUMENTS: &str = r#"{"query":"tide tables of 1890","result_count":null}"#;

fn sse(events: &[serde_json::Value]) -> String {
    let mut reply = String::from(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
    );
    for event in events {
        reply.push_str(&format!("data: {event}\n\n"));
    }
    reply
}

/// A stream that reports `reference` and ends with one complete Advisory.
fn advisory_stream(reference: &str) -> String {
    let message = json!({"id": "msg_01", "type": "message", "role": "assistant",
        "status": "completed", "content": [{"type": "output_text", "text": ADVISORY}]});
    sse(&[
        json!({"type": "response.created",
               "response": {"id": reference, "status": "in_progress", "output": []}}),
        json!({"type": "response.output_item.done", "output_index": 0, "item": message}),
        json!({"type": "response.completed", "response": {
            "id": reference, "status": "completed", "output": [message],
            "usage": {"input_tokens": 40, "output_tokens": 12}}}),
    ])
}

/// A stream that reports `reference`, streams the arguments of one function call, and ends
/// with the complete call.
fn function_call_stream(reference: &str) -> String {
    let call = |arguments: &str, status: &str| {
        json!({"id": "fc_01", "type": "function_call", "status": status, "call_id": "call_01",
               "name": "web_search", "arguments": arguments})
    };
    sse(&[
        json!({"type": "response.created",
               "response": {"id": reference, "status": "in_progress", "output": []}}),
        json!({"type": "response.output_item.added", "output_index": 0,
               "item": call("", "in_progress")}),
        json!({"type": "response.function_call_arguments.delta", "output_index": 0,
               "item_id": "fc_01", "delta": &ARGUMENTS[..20]}),
        json!({"type": "response.function_call_arguments.delta", "output_index": 0,
               "item_id": "fc_01", "delta": &ARGUMENTS[20..]}),
        json!({"type": "response.output_item.done", "output_index": 0,
               "item": call(ARGUMENTS, "completed")}),
        json!({"type": "response.completed", "response": {
            "id": reference, "status": "completed", "output": [call(ARGUMENTS, "completed")],
            "usage": {"input_tokens": 90, "output_tokens": 30}}}),
    ])
}

/// The Exposure of the research Tool on the current Agent Plan Registration.
fn research_exposure() -> ToolExposure {
    ToolExposure::compute(&ExposureFacts {
        enabled: &[EnabledTool {
            registration_revision: EXA_TOOL_REGISTRATION.revision.to_owned(),
            lifecycle: ToolRegistrationLifecycle::Active,
            compatibility: Some(ToolCompatibilityDecision::Compatible),
        }],
        model_registration_revision: storyos_core::AGENT_PLAN_REGISTRATION.revision,
    })
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
async fn an_enabled_tool_is_declared_on_a_new_transport_and_its_function_call_settles_the_run() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![
        advisory_stream("resp_t1"),
        function_call_stream("resp_t2"),
    ])
    .await;
    let plan = DeploymentDestination::VolcengineAgentPlan {
        qualification_authorization: None,
    };
    let (scope, chapter_id, first) = queued_run(&store, "f71", plan.clone()).await;
    admin
        .execute(
            "UPDATE storyos.processing_destination_identity_evidence_revisions
                SET endpoint = $2 WHERE project_id = $1::text::uuid AND endpoint IS NOT NULL",
            &[&scope.project_id.as_ref(), &base],
        )
        .await
        .unwrap();
    let claim = claim_run(&admin, scope.clone(), first.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    // The author enables the research Tool between the two requests of one Conversation.
    let call = issued(
        &store,
        &scope,
        0xf71a,
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
                Some(ToolExposure::empty().snapshot()),
            ],
            Some(json!("refused")),
            Some(json!({"kind": "execution_refused", "capability": "research_tool_unavailable"})),
            Some(json!("new_transport")),
            Some(serde_json::Value::Null),
            Some(exposure.snapshot()),
            json!(declared),
            json!("auto"),
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
            vec![
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
            ],
        )
    );
}
