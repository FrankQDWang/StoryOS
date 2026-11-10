use serde_json::json;
use storyos_adapter_volcengine_responses::AgentPlanExchange;
use storyos_application::{
    ConversationSelection, DestinationRequest, ModelProviderAdapter, ModelStreamSink,
    NoContractFaults, Observation, PreDispatchRefusal, PreparedRequest,
    SetProjectToolEnablementInput, complete_agent_run,
};
use storyos_core::{
    DeploymentDestination, EXA_TOOL_REGISTRATION, EnabledTool, ExposureFacts, ModelAdapter,
    RESEARCH_TOOL_OFFER, ToolCompatibilityDecision, ToolEnablement, ToolExposure,
    ToolRegistrationLifecycle,
};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::{RuntimeClock, admit_run_in, claim_run, queued_run, stores};
use crate::agent_run_retrieval::tests::{adapter, broken_stream, destination, real_run, recovery};
use crate::command_sequence::tests::{SET_PROJECT_TOOL_ENABLEMENT, issued};

const ADVISORY: &str = r#"{"kind":"advisory","text":"Keep it."}"#;
const ARGUMENTS: &str = r#"{"query":"tide tables of 1890","result_count":null}"#;

pub(crate) fn sse(events: &[serde_json::Value]) -> String {
    let mut reply = String::from(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
    );
    for event in events {
        reply.push_str(&format!("data: {event}\n\n"));
    }
    reply
}

/// A stream that reports `reference` and ends with one complete Advisory.
pub(crate) fn advisory_stream(reference: &str) -> String {
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

/// One function call item with `arguments` and `status`.
fn call(arguments: &str, status: &str) -> serde_json::Value {
    json!({"id": "fc_01", "type": "function_call", "status": status, "call_id": "call_01",
           "name": "web_search", "arguments": arguments})
}

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

/// A stream that reports `reference`, streams the arguments of one function call, and ends
/// with the complete call.
pub(crate) fn function_call_stream(reference: &str) -> String {
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
async fn an_enabled_tool_is_declared_on_a_new_transport_and_its_function_call_settles_the_run() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![
        advisory_stream("resp_t1"),
        function_call_stream("resp_t2"),
        // The continuation RunStep answers the refused ToolCall.
        advisory_stream("resp_t3"),
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
            Some(json!("completed")),
            None,
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
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
            ],
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
async fn a_retrieved_function_request_settles_the_run_with_the_research_refusal() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![
        broken_stream("resp_f73"),
        stored_function_call("resp_f73"),
    ])
    .await;
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
                Some("refused".to_owned()),
                Some(
                    r#"{"kind": "execution_refused", "capability": "research_tool_unavailable"}"#
                        .to_owned()
                ),
                None,
                Some("settled".to_owned()),
                None,
                Some("true".to_owned()),
                Some("decision:true,retrieval:true".to_owned()),
            ],
            (
                Some("complete_function_request".to_owned()),
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
            ],
        )
    );
}

/// The real adapter. Before the claim of the first Create that is a successor when `successor`
/// is set, and that is not a successor when it is not, the author disables the research Tool
/// with the Command Challenge `suffix`. Each prepared Create keeps its Exposure Snapshot and the
/// `tools` field of its body.
pub(crate) struct DisableBeforeClaim<'a> {
    pub store: &'a crate::PostgresProjectReader,
    pub scope: &'a storyos_application::ProjectScope,
    pub suffix: u16,
    pub successor: bool,
    pub disabled: std::sync::atomic::AtomicBool,
    pub prepared: std::sync::Mutex<Vec<(serde_json::Value, Option<serde_json::Value>)>>,
}

impl ModelProviderAdapter for DisableBeforeClaim<'_> {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = AgentPlanExchange;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<AgentPlanExchange>, PreDispatchRefusal> {
        let prepared = adapter().prepare(request).await?;
        if let DestinationRequest::Create(create) = request {
            let body: serde_json::Value =
                serde_json::from_str(prepared.projection.serialized_payload.as_deref().unwrap())
                    .unwrap();
            self.prepared
                .lock()
                .unwrap()
                .push((create.tool_exposure.snapshot(), body.get("tools").cloned()));
            if create.successor_of.is_some() == self.successor
                && !self
                    .disabled
                    .swap(/*val*/ true, std::sync::atomic::Ordering::Relaxed)
            {
                crate::agent_run_tool_call::tests::set_research_tool(
                    self.store,
                    self.scope,
                    self.suffix,
                    ToolEnablement::Disabled,
                )
                .await;
            }
        }
        Ok(prepared)
    }

    async fn exchange(
        &self,
        prepared: AgentPlanExchange,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        adapter().exchange(prepared, sink).await
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_exposure_change_before_the_claim_sends_the_request_again_with_the_current_exposure() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![advisory_stream("resp_f79")]).await;
    let (scope, run_id) = real_run(&store, &admin, "f79", &base).await;
    crate::agent_run_tool_call::tests::set_research_tool(
        &store,
        &scope,
        0xf79a,
        ToolEnablement::Enabled,
    )
    .await;
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let route = DisableBeforeClaim {
        store: &store,
        scope: &scope,
        suffix: 0xf79b,
        successor: false,
        disabled: std::sync::atomic::AtomicBool::new(/*v*/ false),
        prepared: std::sync::Mutex::new(Vec::new()),
    };
    complete_agent_run(&store, &route, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
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
                    Some(json!(
                        research_exposure()
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
                    )),
                ),
                (ToolExposure::empty().snapshot(), None),
            ],
            1,
            Some(json!("completed")),
            Some(ToolExposure::empty().snapshot()),
            None,
            vec!["POST /api/plan/v3/responses HTTP/1.1".to_owned()],
        )
    );
}
