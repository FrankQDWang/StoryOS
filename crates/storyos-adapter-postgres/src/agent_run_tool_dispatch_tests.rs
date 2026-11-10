use serde_json::{Value, json};
use storyos_adapter_mcp_client::{McpClient, McpExchange};
use storyos_application::{
    CompleteAgentRunError, NoContractFaults, PreDispatchRefusal, PreparedRequest, ToolCallRequest,
    ToolDestinationAdapter, complete_agent_run_with_tools,
};
use storyos_core::{EXA_TOOL_REGISTRATION, ToolEnablement, ToolObservation};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::{RuntimeClock, claim_run, stores};
use crate::agent_run_retrieval::tests::{TestCredential, adapter, destination, real_run};
use crate::agent_run_tool_call::tests::{set_research_tool, text_and_call_stream};

const SEARCH: &str = r#"{"query":"tide tables of 1890","result_count":null}"#;
const RESULT: &str = "Tide tables of 1890 list two high waters each day.";
const DISCOVERY: &str = include_str!("../../../docs/research/exa-mcp-discovery-2026-11-03.md");
/// The value that `TestCredential` resolves. No record may hold it.
const KEY: &str = "storyos-test-credential";

/// The real MCP client. After its preparation and after its exchange, a statement can change
/// the disposable database, as an author, an operator, or a crash does.
struct Hooked<'a> {
    admin: &'a Client,
    after_prepare: String,
    after_exchange: String,
}

impl ToolDestinationAdapter for Hooked<'_> {
    type Prepared = McpExchange;

    async fn prepare(
        &self,
        request: &ToolCallRequest,
    ) -> Result<PreparedRequest<McpExchange>, PreDispatchRefusal> {
        let prepared = ToolDestinationAdapter::prepare(&client(), request).await;
        self.admin.batch_execute(&self.after_prepare).await.unwrap();
        prepared
    }

    async fn exchange(&self, prepared: McpExchange) -> ToolObservation {
        let observation = ToolDestinationAdapter::exchange(&client(), prepared).await;
        self.admin
            .batch_execute(&self.after_exchange)
            .await
            .unwrap();
        observation
    }
}

fn client() -> McpClient<TestCredential> {
    McpClient {
        resolver: TestCredential,
        timeout: std::time::Duration::from_secs(/*secs*/ 10),
    }
}

fn reply(id: u64, result: Value) -> String {
    let body = json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string();
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// The `tools/list` result of the Discovery Record appendix, after `edit`.
fn exa_tools(edit: fn(&mut Value)) -> Value {
    let appendix = &DISCOVERY[DISCOVERY.find("## Appendix").unwrap()..];
    let start = appendix.find("```json\n").unwrap() + "```json\n".len();
    let end = start + appendix[start..].find("\n```").unwrap();
    let recorded: Value = serde_json::from_str(&appendix[start..end]).unwrap();
    let mut tools: Vec<Value> = recorded["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| {
            let mut listed = json!({"name": tool["name"], "description": tool["description"],
                "inputSchema": tool["input_schema"], "annotations": tool["annotations"]});
            if !tool["output_schema"].is_null() {
                listed["outputSchema"] = tool["output_schema"].clone();
            }
            listed
        })
        .collect();
    let mut tools_value = Value::Array(std::mem::take(&mut tools));
    edit(&mut tools_value);
    json!({ "tools": tools_value })
}

/// The answers of the fake MCP server: the session, the tool list, and then `call`, if any.
fn mcp_replies(tools: Value, call: Option<String>) -> Vec<String> {
    let mut replies = vec![
        reply(
            1,
            json!({"protocolVersion": "2025-11-25", "capabilities": {"tools": {}},
                   "serverInfo": {"name": "exa-search-server", "version": "3.5.1"}}),
        ),
        "HTTP/1.1 202 Accepted\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_owned(),
        reply(2, tools),
    ];
    replies.extend(call);
    replies
}

/// One Run whose decision requests one `web_search` ToolCall, with the research Tool enabled
/// and its evidence endpoint at the fake MCP server `mcp_base`.
async fn tool_run(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    suffix: u16,
    mcp_base: &str,
) -> (storyos_application::ProjectScope, String) {
    let (model_base, _model) = destination(vec![text_and_call_stream(
        &format!("resp_{prefix}"),
        "call_01",
        "web_search",
        SEARCH,
        "completed",
    )])
    .await;
    let (scope, run_id) = real_run(store, admin, prefix, &model_base).await;
    set_research_tool(store, &scope, suffix, ToolEnablement::Enabled).await;
    admin
        .execute(
            "UPDATE storyos.processing_destination_identity_evidence_revisions SET endpoint = $2
              WHERE project_id = $1::text::uuid AND evidence_kind = 'mcp_remote_boundary'",
            &[&scope.project_id.as_ref(), &mcp_base],
        )
        .await
        .unwrap();
    (scope, run_id)
}

/// The Run, its one ToolCall without volatile identities, and its Activity Run Events.
async fn tool_facts(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_build_object(
                      'run', jsonb_build_object('status', run.status, 'settlement', run.settlement),
                      'tool_call', (SELECT jsonb_build_object(
                          'state', call.state, 'refusal', call.refusal,
                          'attempt', call.attempt #- '{outbound_disclosure_event,event_id}'
                                                 #- '{admission_decision,grant_id}'
                                                 #- '{wire_payload_projection,digest}'
                                                 #- '{wire_payload_projection,serialized_payload}'
                                                 #- '{destination_attempt,endpoint}',
                          'has_attempt_id', call.destination_attempt_id IS NOT NULL,
                          'effect_outcome', call.effect_outcome, 'result_text', call.result_text,
                          'result_digest', call.result_digest, 'result_bytes', call.result_bytes)
                        FROM storyos.tool_calls AS call WHERE call.run_id = run.run_id),
                      'events', (SELECT jsonb_agg((event.payload - 'tool_call_id'
                                   - 'destination_attempt_id')
                                   || jsonb_build_object('kind', event.event_kind)
                                   ORDER BY event.event_id)
                                   FROM storyos.agent_run_events AS event
                                  WHERE event.run_id = run.run_id),
                      'holds_key', (SELECT bool_or(position($2 IN record::text) > 0)
                                      FROM (SELECT to_jsonb(call) AS record
                                              FROM storyos.tool_calls AS call
                                             WHERE call.run_id = run.run_id
                                            UNION ALL
                                            SELECT to_jsonb(event)
                                              FROM storyos.agent_run_events AS event
                                             WHERE event.run_id = run.run_id) AS records))::text
               FROM storyos.agent_runs AS run WHERE run.run_id = $1::text::uuid",
            &[&run_id, &KEY],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get::<_, &str>(0)).unwrap()
}

fn settled_event(state: &str, refusal: Value, outcome: Value, bytes: Value) -> Value {
    json!({"kind": "tool_call_settled", "call_id": "call_01",
           "tool_registration_revision": EXA_TOOL_REGISTRATION.revision,
           "tool_spec_revision": "storyos.tool.web-search.v1", "state": state,
           "refusal": refusal, "outcome": outcome, "result_bytes": bytes})
}

fn attempt(settlement: &str, disclosure: &str) -> Value {
    json!({
        "admission_decision": {
            "decision": "admitted",
            "grant_scope": "research_tool_v1",
            "effect_request": {"external_read": "one_search", "external_write": "none",
                               "outbound_disclosure": "agent_query",
                               "disclosed": ["tide tables of 1890"]},
        },
        "wire_payload_projection": {"execution_profile": "storyos.mcp-streamable-http.v1",
                                    "mapping_revision": "2025-11-25"},
        "destination_attempt": {"settlement": settlement},
        "outbound_disclosure_event": {"source_class": "agent_query", "state": disclosure},
    })
}

const POST: &str = "POST /api/plan/v3 HTTP/1.1";

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_claimed_search_settles_with_its_result_text_and_one_run_event() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let call = reply(
        3,
        json!({"content": [{"type": "text", "text": RESULT}], "isError": false}),
    );
    let (mcp_base, mcp) = destination(mcp_replies(exa_tools(|_| {}), Some(call))).await;
    let (scope, run_id) = tool_run(&store, &admin, "dc1", 0xdc1a, &mcp_base).await;
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
                    "attempt": attempt("terminal_result", "confirmed"),
                    "has_attempt_id": true,
                    "effect_outcome": outcome,
                    "result_text": RESULT,
                    "result_digest": format!("sha256:{}", storyos_core::hex_sha256(RESULT.as_bytes())),
                    "result_bytes": RESULT.len(),
                },
                "events": [settled_event("settled", Value::Null, outcome, json!(RESULT.len()))],
                "holds_key": false,
            }),
            vec![POST.to_owned(); 4],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_authority_change_before_the_claim_refuses_with_its_reason_and_sends_nothing() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let binding = "UPDATE storyos.project_external_use_binding_revisions AS binding SET";
    let tool_binding = "binding.project_id = '{project}'
                        AND binding.tool_registration_revision IS NOT NULL";
    let cases = [
        (
            "dc2",
            0xdc2a,
            "UPDATE storyos.project_tool_enablements SET enablement = 'disabled',
                    external_compatibility_decision = NULL WHERE project_id = '{project}'"
                .to_owned(),
            "tool_not_enabled",
        ),
        (
            "dc3",
            0xdc3a,
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
            0xdc4a,
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
            0xdc5a,
            format!(
                "{binding} credential_source = 'none', credential_reference = NULL
                  WHERE {tool_binding}"
            ),
            "credential_unbound",
        ),
        (
            "dc6",
            0xdc6a,
            format!(
                "{binding} grant_id = (SELECT grant_id FROM storyos.project_destination_grants
                                       WHERE project_id = '{{project}}'
                                         AND grant_kind <> 'research_tool_use' LIMIT 1)
                  WHERE {tool_binding}"
            ),
            "disclosure_outside_grant",
        ),
    ];
    let mut observed = Vec::new();
    for (prefix, suffix, statement, _) in &cases {
        // The fake server listens for no message: a sent request finds no answer.
        let (mcp_base, mcp) = destination(Vec::new()).await;
        let (scope, run_id) = tool_run(&store, &admin, prefix, *suffix, &mcp_base).await;
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
            mcp.await.unwrap(),
        ));
    }

    assert_eq!(
        observed,
        cases
            .iter()
            .map(|(_, _, _, code)| (
                json!({"state": "refused", "refusal": code, "attempt": null,
                       "has_attempt_id": false, "effect_outcome": null, "result_text": null,
                       "result_digest": null, "result_bytes": null}),
                json!([settled_event(
                    "refused",
                    json!(code),
                    Value::Null,
                    Value::Null
                )]),
                Vec::<String>::new(),
            ))
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_tool_call_that_a_crash_left_dispatched_settles_unknown_and_is_not_sent_again() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let call = reply(
        3,
        json!({"content": [{"type": "text", "text": RESULT}], "isError": false}),
    );
    let (mcp_base, mcp) = destination(mcp_replies(exa_tools(|_| {}), Some(call))).await;
    let (scope, run_id) = tool_run(&store, &admin, "dc7", 0xdc7a, &mcp_base).await;
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
                   "attempt": attempt("outcome_unknown", "unknown"), "has_attempt_id": true,
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let drifted = exa_tools(|tools| {
        tools[0]["inputSchema"]["required"] = json!(["query", "objective", "numResults"]);
    });
    let (mcp_base, mcp) = destination(mcp_replies(drifted, None)).await;
    let (scope, run_id) = tool_run(&store, &admin, "dc8", 0xdc8a, &mcp_base).await;
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
                   "attempt": attempt("not_submitted", "not_attempted"), "has_attempt_id": true,
                   "effect_outcome": outcome, "result_text": null, "result_digest": null,
                   "result_bytes": null}),
            settled_event(
                "refused",
                json!("tool_registration_unavailable"),
                outcome,
                Value::Null
            ),
            vec![POST.to_owned(); 3],
        )
    );
}
