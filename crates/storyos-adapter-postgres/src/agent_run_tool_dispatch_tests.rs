use serde_json::{Value, json};
use storyos_application::{CompleteAgentRunError, NoContractFaults, complete_agent_run_with_tools};
use storyos_core::EXA_TOOL_REGISTRATION;

use crate::shared_tests::agent_run_retrieval::{adapter, destination};
use crate::shared_tests::agent_run_tool_dispatch::{
    Hooked, POST, RESULT, SEARCH, attempt, exa_tools, interrupted_attempt, mcp_replies, reply,
    settled_event, settled_event_after_quarantine, tool_facts, tool_run,
};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::model_gateway::{RuntimeClock, claim_run, stores};

/// Gates one to six of the search ToolCall, with no volatile identity.
fn tool_manifest() -> Value {
    let effect = attempt("", "", /*released*/ false)["admission_decision"]["effect_request"].take();
    json!({
        "requirement": {"kind": "tool_call", "call_id": "call_01",
                        "registration_revision": EXA_TOOL_REGISTRATION.revision,
                        "spec_revision": "storyos.tool.web-search.v1"},
        "position": 0, "sufficiency": "complete", "grant_scope": "research_tool_v1",
        "gates": {
            "candidate": {"arguments": serde_json::from_str::<Value>(SEARCH).unwrap()},
            "eligibility": {"grant_scope": "research_tool_v1"},
            "selection": "one_candidate",
            "bounded_projection": {"intake": "typed_arguments", "effect_request": effect},
            "destination_context": {"destination": "tool",
                                    "registration_revision": EXA_TOOL_REGISTRATION.revision,
                                    "spec_revision": "storyos.tool.web-search.v1"},
            "outbound_disclosure": {"source_class": "agent_query",
                                    "disclosed": ["tide tables of 1890"]},
        },
        "authority": {"kept": true,
                      "exact": ["evidence_revision",
                                "external_compatibility_decision", "grant_id",
                                "processing_destination_identity",
                                "project_model_use_binding_revision"]},
        "exact_scope": true,
        "claim_refers_to_it": true,
    })
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_claimed_search_settles_with_its_result_text_and_one_run_event() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let call = reply(
        0,
        json!({"content": [{"type": "text", "text": RESULT}], "isError": false}),
    );
    let (mcp_base, mcp) = destination(mcp_replies(exa_tools(|_| {}), Some(call))).await;
    let (scope, run_id) = tool_run(&store, &admin, "dc1", &mcp_base).await;
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    let tools = Hooked {
        admin: &admin,
        after_prepare: String::new(),
        after_exchange: String::new(),
    };
    complete_agent_run_with_tools(
        &store,
        &adapter(),
        &tools,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();
    let outcome = json!({"external_read": "confirmed", "outbound_disclosure": "confirmed"});

    assert_eq!(
        (tool_facts(&admin, &run_id).await, mcp.await.unwrap()),
        (
            json!({
                "run": {"status": "completed", "settlement": null},
                "tool_call": {
                    "state": "settled", "refusal": null,
                    "attempt": attempt("terminal_result", "confirmed", /*released*/ true),
                    "has_attempt_id": true,
                    "effect_outcome": outcome,
                    "result_text": RESULT,
                    "result_digest": format!("sha256:{}", storyos_core::hex_sha256(RESULT.as_bytes())),
                    "result_bytes": RESULT.len(),
                },
                "events": [settled_event("settled", Value::Null, outcome, json!(RESULT.len()))],
                "manifests": [tool_manifest()],
                "holds_key": false,
            }),
            vec![POST.to_owned(); 4],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_authority_change_before_the_claim_refuses_with_its_reason_and_sends_nothing() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let binding = "UPDATE storyos.project_external_use_binding_revisions AS binding SET";
    let tool_binding = "binding.project_id = '{project}'
                        AND binding.tool_registration_revision IS NOT NULL";
    let cases = [
        (
            "dc3",
            format!(
                "UPDATE storyos.tool_registration_revisions SET lifecycle = 'retired',
                        quarantine_differences = '[{{\"kind\":\"tool_removed\",\"tool\":\"t\"}}]',
                        quarantined_at = clock_timestamp()
                  WHERE tool_registration_revision = '{}'",
                EXA_TOOL_REGISTRATION.revision
            ),
            "tool_registration_unavailable",
        ),
        (
            "dc4",
            format!(
                "INSERT INTO storyos.tool_registration_revisions
                   (tool_registration_revision, registration_kind, server_url, server_name,
                    protocol_revision, adapter_revision)
                 VALUES ('018f0000-0000-7001-8000-0000000edc4a', 'mcp_remote', 'https://e.org',
                         'other', '2025-11-25', 'other') ON CONFLICT DO NOTHING;
                 {binding} tool_registration_revision = '018f0000-0000-7001-8000-0000000edc4a'
                  WHERE {tool_binding}"
            ),
            "tool_registration_incompatible",
        ),
        (
            "dc5",
            format!(
                "{binding} credential_source = 'none', credential_reference = NULL
                  WHERE {tool_binding}"
            ),
            "credential_unbound",
        ),
        (
            "dc6",
            format!(
                "{binding} grant_id = (SELECT grant_id FROM storyos.project_destination_grants
                                       WHERE project_id = '{{project}}'
                                         AND grant_kind <> 'research_tool_use' LIMIT 1)
                  WHERE {tool_binding}"
            ),
            "disclosure_outside_grant",
        ),
        (
            "dce",
            // The Run Capability Grant narrows: the Exposure of the Step Snapshot names no Tool.
            "UPDATE storyos.model_attempts
                SET payload = jsonb_set(payload, '{step_snapshot,tool_exposure,tools}', '[]')
              WHERE project_id = '{project}'"
                .to_owned(),
            "tool_outside_run_grant",
        ),
    ];
    let mut observed = Vec::new();
    for (prefix, statement, _) in &cases {
        // The fake server listens for no message: a sent request finds no answer.
        let (mcp_base, mcp) = destination(Vec::new()).await;
        let (scope, run_id) = tool_run(&store, &admin, prefix, &mcp_base).await;
        let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
        let tools = Hooked {
            admin: &admin,
            after_prepare: format!(
                "SET session_replication_role = replica;
                 {};
                 SET session_replication_role = DEFAULT;",
                statement.replace("{project}", scope.project_id.as_ref())
            ),
            after_exchange: String::new(),
        };
        let completed = complete_agent_run_with_tools(
            &store,
            &adapter(),
            &tools,
            &NoContractFaults,
            &RuntimeClock,
            &claim,
        )
        .await;
        admin
            .batch_execute(&format!(
                "UPDATE storyos.tool_registration_revisions SET lifecycle = 'active',
                        quarantine_differences = NULL, quarantined_at = NULL
                  WHERE tool_registration_revision = '{}'",
                EXA_TOOL_REGISTRATION.revision
            ))
            .await
            .unwrap();
        completed.unwrap();
        let facts = tool_facts(&admin, &run_id).await;
        observed.push((
            facts["tool_call"].clone(),
            facts["events"].clone(),
            json!(facts["manifests"].as_array().map_or(0, Vec::len)),
            mcp.await.unwrap(),
        ));
    }

    assert_eq!(
        observed,
        cases
            .iter()
            .map(|(_, _, code)| (
                json!({"state": "refused", "refusal": code, "attempt": null,
                       "has_attempt_id": false, "effect_outcome": null, "result_text": null,
                       "result_digest": null, "result_bytes": null}),
                json!([settled_event(
                    "refused",
                    json!(code),
                    Value::Null,
                    Value::Null
                )]),
                // Gate six committed before the preparation, and the claim refused.
                json!(1),
                Vec::<String>::new(),
            ))
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_tool_call_that_a_crash_left_dispatched_settles_unknown_and_is_not_sent_again() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let call = reply(
        0,
        json!({"content": [{"type": "text", "text": RESULT}], "isError": false}),
    );
    let (mcp_base, mcp) = destination(mcp_replies(exa_tools(|_| {}), Some(call))).await;
    let (scope, run_id) = tool_run(&store, &admin, "dc7", &mcp_base).await;
    let crash = Hooked {
        admin: &admin,
        after_prepare: String::new(),
        after_exchange: format!(
            "UPDATE storyos.agent_runs SET fence_token = fence_token + 1 WHERE run_id = '{run_id}'"
        ),
    };
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let lost = complete_agent_run_with_tools(
        &store,
        &adapter(),
        &crash,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await;
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    let nothing_sent = Hooked {
        admin: &admin,
        after_prepare: "SELECT 1 / 0".to_owned(),
        after_exchange: "SELECT 1 / 0".to_owned(),
    };
    complete_agent_run_with_tools(
        &store,
        &adapter(),
        &nothing_sent,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();
    let outcome = json!({"external_read": "unknown", "outbound_disclosure": "unknown"});
    let facts = tool_facts(&admin, &run_id).await;

    assert_eq!(
        (
            matches!(lost, Err(CompleteAgentRunError::StaleFence)),
            facts["run"].clone(),
            facts["tool_call"].clone(),
            facts["events"].clone(),
            mcp.await.unwrap(),
        ),
        (
            true,
            json!({"status": "completed", "settlement": null}),
            json!({"state": "unknown", "refusal": null,
                   "attempt": interrupted_attempt("worker_restart"), "has_attempt_id": true,
                   "effect_outcome": outcome, "result_text": null, "result_digest": null,
                   "result_bytes": null}),
            json!([settled_event("unknown", Value::Null, outcome, Value::Null)]),
            vec![POST.to_owned(); 4],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn tool_contract_drift_quarantines_the_revision_and_sends_no_call() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let drifted = exa_tools(|tools| {
        tools[0]["inputSchema"]["required"] = json!(["query", "objective", "numResults"]);
    });
    let (mcp_base, mcp) = destination(mcp_replies(drifted, None)).await;
    let (scope, run_id) = tool_run(&store, &admin, "dc8", &mcp_base).await;
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    let tools = Hooked {
        admin: &admin,
        after_prepare: String::new(),
        after_exchange: String::new(),
    };
    let completed = complete_agent_run_with_tools(
        &store,
        &adapter(),
        &tools,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await;
    let lifecycle: String = admin
        .query_one(
            "WITH lifecycle AS (SELECT lifecycle FROM storyos.tool_registration_revisions
                                 WHERE tool_registration_revision = $1::text::uuid),
                  restored AS (UPDATE storyos.tool_registration_revisions
                                  SET lifecycle = 'active', quarantine_differences = NULL,
                                      quarantined_at = NULL
                                WHERE tool_registration_revision = $1::text::uuid)
             SELECT lifecycle FROM lifecycle",
            &[&EXA_TOOL_REGISTRATION.revision],
        )
        .await
        .unwrap()
        .get(0);
    completed.unwrap();
    let outcome = json!({"external_read": "not_attempted", "outbound_disclosure": "not_attempted"});
    let mut facts = tool_facts(&admin, &run_id).await;
    let quarantined = facts["events"][0].take();

    assert_eq!(
        (
            lifecycle,
            quarantined["kind"].clone(),
            quarantined["differences"][0]["kind"].clone(),
            facts["tool_call"].clone(),
            facts["events"][1].clone(),
            mcp.await.unwrap(),
        ),
        (
            "quarantined".to_owned(),
            json!("tool_registration_quarantined"),
            json!("input_schema"),
            json!({"state": "refused", "refusal": "tool_registration_unavailable",
                   "attempt": attempt("not_submitted", "not_attempted", /*released*/ true),
                   "has_attempt_id": true,
                   "effect_outcome": outcome, "result_text": null, "result_digest": null,
                   "result_bytes": null}),
            settled_event_after_quarantine(
                "refused",
                json!("tool_registration_unavailable"),
                outcome,
                Value::Null
            ),
            vec![POST.to_owned(); 3],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_claim_in_another_project_scope_sends_nothing_and_changes_no_tool_call() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // The fake server listens for no message: a sent request finds no answer.
    let (mcp_base, mcp) = destination(Vec::new()).await;
    let (scope, run_id) = tool_run(&store, &admin, "dc9", &mcp_base).await;
    let (other_scope, other_run_id) = tool_run(&store, &admin, "dcb", &mcp_base).await;
    // A lost lease after the preparation leaves the derived ToolCall `pending`.
    let lost_lease = Hooked {
        admin: &admin,
        after_prepare: format!(
            "UPDATE storyos.agent_runs SET fence_token = fence_token + 1 WHERE run_id = '{run_id}'"
        ),
        after_exchange: String::new(),
    };
    let first = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let lost = complete_agent_run_with_tools(
        &store,
        &adapter(),
        &lost_lease,
        &NoContractFaults,
        &RuntimeClock,
        &first,
    )
    .await;
    let before = tool_facts(&admin, &run_id).await;
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    let foreign = storyos_application::ClaimedAgentRun {
        project_scope: other_scope,
        ..claim
    };
    let tools = Hooked {
        admin: &admin,
        after_prepare: String::new(),
        after_exchange: String::new(),
    };
    let next = storyos_application::ToolDispatchStore::next_tool_call(&store, &foreign).await;
    let completed = complete_agent_run_with_tools(
        &store,
        &adapter(),
        &tools,
        &NoContractFaults,
        &RuntimeClock,
        &foreign,
    )
    .await;
    let after = tool_facts(&admin, &run_id).await;
    let other = tool_facts(&admin, &other_run_id).await;

    assert_eq!(
        (
            matches!(lost, Err(CompleteAgentRunError::StaleFence)),
            before["tool_call"]["state"].clone(),
            next.is_err(),
            completed.is_err(),
            after["tool_call"] == before["tool_call"] && after["manifests"] == before["manifests"],
            after["events"].clone(),
            other["tool_call"].clone(),
            mcp.await.unwrap(),
        ),
        (
            true,
            json!("pending"),
            true,
            true,
            true,
            Value::Null,
            Value::Null,
            Vec::<String>::new(),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_protocol_revision_with_a_null_character_quarantines_and_settles() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let initialize = reply(
        1,
        json!({"protocolVersion": "2025-11-25\u{0}x", "capabilities": {"tools": {}},
               "serverInfo": {"name": "exa-search-server", "version": "3.5.1"}}),
    );
    let (mcp_base, mcp) = destination(vec![initialize]).await;
    let (scope, run_id) = tool_run(&store, &admin, "dcf", &mcp_base).await;
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    let tools = Hooked {
        admin: &admin,
        after_prepare: String::new(),
        after_exchange: String::new(),
    };
    let completed = complete_agent_run_with_tools(
        &store,
        &adapter(),
        &tools,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await;
    let lifecycle: String = admin
        .query_one(
            "WITH lifecycle AS (SELECT lifecycle FROM storyos.tool_registration_revisions
                                 WHERE tool_registration_revision = $1::text::uuid),
                  restored AS (UPDATE storyos.tool_registration_revisions
                                  SET lifecycle = 'active', quarantine_differences = NULL,
                                      quarantined_at = NULL
                                WHERE tool_registration_revision = $1::text::uuid)
             SELECT lifecycle FROM lifecycle",
            &[&EXA_TOOL_REGISTRATION.revision],
        )
        .await
        .unwrap()
        .get(0);
    completed.unwrap();
    let facts = tool_facts(&admin, &run_id).await;

    assert_eq!(
        (
            lifecycle,
            facts["events"][0]["differences"].clone(),
            facts["tool_call"]["state"].clone(),
            facts["events"][1]["kind"].clone(),
            mcp.await.unwrap(),
        ),
        (
            "quarantined".to_owned(),
            json!([{"kind": "protocol_revision", "pinned": "2025-11-25",
                    "observed": {"nul_separated": ["2025-11-25", "x"]}}]),
            json!("refused"),
            json!("tool_call_settled"),
            vec![POST.to_owned()],
        )
    );
}
