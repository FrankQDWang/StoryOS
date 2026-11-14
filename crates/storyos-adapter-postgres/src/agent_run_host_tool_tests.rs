use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, ConversationSelection,
    NoContractFaults, complete_agent_run,
};
use storyos_contracts::{ToolCallInspect, ToolCallOutcomeInspect};
use storyos_core::{
    ADVISORY_TEXT, DeploymentDestination, FINISH_DOES_NOT_COUNT_RESULT, HOST_TOOL_REGISTRATION,
    RESEARCH_TOOL_OFFER,
};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::{
    RuntimeClock, admit_run_in, claim_run, queued_run, queued_run_offering, stores,
};

/// The `tool_result` projection of an empty result (ADR 0085).
const EMPTY_OUTPUT: &str = r#"<tool_result source="call_finish" state="empty">The tool answered with no entry.</tool_result>"#;

/// The Run, each of its RunSteps in order with the `tool_choice`, the declared function names,
/// the sent function outputs, and the `tool_result` sources of the Operation Requirement, each
/// ToolCall row without its identities, and each Activity Run Event.
async fn run_facts(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_build_object(
                      'status', run.status, 'position', run.active_decision_position,
                      'steps', (SELECT jsonb_agg(jsonb_build_object(
                          'position', attempt.decision_position,
                          'decision', attempt.payload #>> '{decision,kind}',
                          'tool_choice', (attempt.payload #>> '{wire,serialized_payload}')::jsonb
                                         ->> 'tool_choice',
                          'tools', (attempt.payload #>> '{wire,serialized_payload}')::jsonb
                                   -> 'tools',
                          'tool_outputs', (attempt.payload #>> '{wire,serialized_payload}')::jsonb
                                          -> 'tool_outputs',
                          'tool_results', jsonb_path_query_array(
                            requirement.payload, '$.operation_requirement.tool_results[*].state'))
                          ORDER BY attempt.model_attempt_id)
                        FROM storyos.model_attempts AS attempt
                        JOIN storyos.operation_requirements AS requirement
                          ON (requirement.run_id, requirement.decision_position,
                              requirement.requirement_role) =
                             (attempt.run_id, attempt.decision_position, 'primary')
                       WHERE attempt.run_id = run.run_id),
                      'calls', (SELECT jsonb_agg(to_jsonb(call) - 'tool_call_id' - 'owner_user_id'
                                  - 'project_id' - 'run_id' - 'model_attempt_id' - 'decision_id'
                                  ORDER BY call.decision_position, call.call_position)
                                  FROM storyos.tool_calls AS call WHERE call.run_id = run.run_id),
                      'events', (SELECT jsonb_agg(jsonb_build_object(
                                   'kind', event.event_kind,
                                   'payload', event.payload - 'tool_call_id')
                                   ORDER BY event.event_position)
                                   FROM storyos.agent_run_events AS event
                                  WHERE event.run_id = run.run_id))::text
               FROM storyos.agent_runs AS run WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(0)).unwrap()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_finish_call_settles_in_process_and_reenters_as_one_tool_result() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, run_id) =
        queued_run(&store, "1e1", DeploymentDestination::HostFakeToolLoop).await;
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;

    // The Worker of the Server has no Tool adapter.
    complete_agent_run(
        &store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();

    let empty = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let host = HOST_TOOL_REGISTRATION.revision;
    assert_eq!(
        run_facts(&admin, &run_id).await,
        json!({
            "status": "completed", "position": 1,
            "steps": [
                {"position": 0, "decision": "tool_request", "tool_choice": "required",
                 "tools": ["propose_edit", "submit_research", "read_tool_result", "finish"], "tool_outputs": [], "tool_results": []},
                {"position": 1, "decision": "advisory", "tool_choice": "none",
                 "tools": ["propose_edit", "submit_research", "read_tool_result", "finish"],
                 "tool_outputs": [{"call_id": "call_finish", "output": EMPTY_OUTPUT}],
                 "tool_results": ["empty"]},
            ],
            "calls": [{
                "decision_position": 0, "call_id": "call_finish", "call_position": 0,
                "caller_route": "model", "function_name": "finish",
                "registration_id": "host_tools", "tool_registration_revision": host,
                "tool_spec_revision": "storyos.tool.finish.v1", "arguments": {},
                "argument_digest":
                  "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
                "state": "settled", "refusal": null, "destination_attempt_id": null,
                "attempt": null, "effect_outcome": null, "result_text": "",
                "result_digest": empty, "result_bytes": 0,
            }],
            "events": [{"kind": "tool_call_settled", "payload": {
                "call_id": "call_finish", "tool_registration_revision": host,
                "tool_spec_revision": "storyos.tool.finish.v1", "destination_attempt_id": null,
                "state": "settled", "refusal": null, "outcome": null, "result_bytes": 0,
            }}],
        })
    );
    let completed = store
        .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        completed.decision,
        AgentRunDecisionInspect::Advisory { ref text, .. } if text == ADVISORY_TEXT
    ));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_process_with_no_tool_adapter_executes_finish_and_refuses_the_research_call() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let research = Some(&RESEARCH_TOOL_OFFER);
    let (scope, chapter_id, first) = queued_run_offering(
        &store,
        "1e2",
        DeploymentDestination::HostFakeToolLoop,
        research,
    )
    .await;
    let claim = claim_run(&admin, scope.clone(), first).await;
    complete_agent_run(
        &store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();
    let run_id = admit_run_in(
        &store,
        &scope,
        &chapter_id,
        "1e3",
        DeploymentDestination::HostFakeToolLoop,
        "Search the web: tide tables of 1890",
        ConversationSelection::New,
        research,
    )
    .await;
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;

    complete_agent_run(
        &store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();

    let facts = run_facts(&admin, &run_id).await;
    let calls: Vec<Value> = facts["calls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|call| {
            json!([
                call["function_name"],
                call["state"],
                call["refusal"],
                call["attempt"],
                call["result_text"]
            ])
        })
        .collect();
    let steps: Vec<Value> = facts["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| json!([step["decision"], step["tool_choice"], step["tool_results"]]))
        .collect();
    // The refused call beside it makes the first `finish` call count for nothing, and its result
    // says so.
    assert_eq!(
        (calls, steps, &facts["status"]),
        (
            vec![
                json!([
                    "web_search",
                    "refused",
                    "destination_request_unsupported",
                    null,
                    null
                ]),
                json!([
                    "finish",
                    "settled",
                    null,
                    null,
                    FINISH_DOES_NOT_COUNT_RESULT
                ]),
                json!(["finish", "settled", null, null, ""]),
            ],
            vec![
                json!(["tool_request", "required", []]),
                json!(["tool_request", "required", ["rejected", "ok"]]),
                json!(["advisory", "none", ["rejected", "ok", "empty"]]),
            ],
            &json!("completed"),
        )
    );
    let registration = admin
        .query_one(
            "SELECT registration_kind, lifecycle, server_url FROM storyos.tool_registration_revisions
              WHERE tool_registration_revision = $1::text::uuid",
            &[&HOST_TOOL_REGISTRATION.revision],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            registration.get::<_, &str>(0),
            registration.get::<_, &str>(1),
            registration.get::<_, Option<&str>>(2),
        ),
        ("host", "active", None)
    );

    // The public ToolCall list of the request Attempt holds only the research call.
    let request_attempt: String = admin
        .query_one(
            "SELECT model_attempt_id::text FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND decision_position = 0",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(0);
    let record = store
        .read_agent_run(
            &scope,
            &run_id,
            &AgentRunReadSelection::ModelAttempt(request_attempt),
        )
        .await
        .unwrap()
        .unwrap();
    let AgentRunDecisionInspect::ToolRequest { calls, .. } = record.decision else {
        panic!("the request Attempt shows its function request");
    };
    assert_eq!(
        calls,
        vec![ToolCallInspect::Search {
            query: "tide tables of 1890".to_owned(),
            outcome: ToolCallOutcomeInspect::Failed,
        }]
    );
}
