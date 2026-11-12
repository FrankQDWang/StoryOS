use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, NoContractFaults,
    complete_agent_run,
};
use storyos_core::{ADVISORY_TEXT, DeploymentDestination, HOST_TOOL_REGISTRATION};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::{RuntimeClock, claim_run, queued_run, stores};

/// The `tool_result` projection of an empty result (ADR 0085).
const EMPTY_OUTPUT: &str = r#"<tool_result source="call_finish" state="empty">The tool answered with no entry.</tool_result>"#;

/// The Run, each of its RunSteps in order with the declared function names, the sent function
/// outputs, and the `tool_result` sources of the Operation Requirement, each ToolCall row without
/// its identities, and each Activity Run Event.
async fn run_facts(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_build_object(
                      'status', run.status, 'position', run.active_decision_position,
                      'steps', (SELECT jsonb_agg(jsonb_build_object(
                          'position', attempt.decision_position,
                          'decision', attempt.payload #>> '{decision,kind}',
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
                                  - 'project_id' - 'run_id' - 'model_attempt_id' - 'decision_id')
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
                {"position": 0, "decision": "tool_request", "tools": ["finish"],
                 "tool_outputs": [], "tool_results": []},
                {"position": 1, "decision": "advisory", "tools": ["finish"],
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
async fn the_public_tool_call_list_does_not_hold_a_host_tool_call() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, run_id) =
        queued_run(&store, "1e2", DeploymentDestination::HostFakeToolLoop).await;
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

    assert!(matches!(
        record.decision,
        AgentRunDecisionInspect::ToolRequest { ref calls, .. } if calls.is_empty()
    ));
}
