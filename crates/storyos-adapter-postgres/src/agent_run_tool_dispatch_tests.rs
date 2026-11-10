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
use crate::agent_run_tool_exposure::tests::advisory_stream;

pub(crate) const SEARCH: &str = r#"{"query":"tide tables of 1890","result_count":null}"#;
pub(crate) const RESULT: &str = "Tide tables of 1890 list two high waters each day.";
const DISCOVERY: &str = include_str!("../../../docs/research/exa-mcp-discovery-2026-11-03.md");
/// The value that `TestCredential` resolves. No record may hold it.
const KEY: &str = "storyos-test-credential";

/// The real MCP client. After its preparation and after its exchange, a statement can change
/// the disposable database, as an author, an operator, or a crash does.
pub(crate) struct Hooked<'a> {
    pub admin: &'a Client,
    pub after_prepare: String,
    pub after_exchange: String,
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

pub(crate) fn client() -> McpClient<TestCredential> {
    McpClient {
        resolver: TestCredential,
        timeout: std::time::Duration::from_secs(/*secs*/ 10),
    }
}

pub(crate) fn reply(id: u64, result: Value) -> String {
    let body = json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string();
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// The `tools/list` result of the Discovery Record appendix, after `edit`.
pub(crate) fn exa_tools(edit: fn(&mut Value)) -> Value {
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
pub(crate) fn mcp_replies(tools: Value, call: Option<String>) -> Vec<String> {
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
/// and its evidence endpoint at the fake MCP server `mcp_base`. The continuation RunStep gets an
/// Advisory.
async fn tool_run(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    suffix: u16,
    mcp_base: &str,
) -> (storyos_application::ProjectScope, String) {
    let (model_base, _model) = destination(vec![
        text_and_call_stream(
            &format!("resp_{prefix}"),
            "call_01",
            "web_search",
            SEARCH,
            "completed",
        ),
        advisory_stream(&format!("resp_{prefix}_2")),
    ])
    .await;
    let (scope, run_id) = real_run(store, admin, prefix, &model_base).await;
    set_research_tool(store, &scope, suffix, ToolEnablement::Enabled).await;
    point_mcp(admin, &scope, mcp_base).await;
    (scope, run_id)
}

/// Points the MCP evidence of the Project at the fake MCP server `mcp_base`.
pub(crate) async fn point_mcp(
    admin: &Client,
    scope: &storyos_application::ProjectScope,
    mcp_base: &str,
) {
    admin
        .execute(
            "UPDATE storyos.processing_destination_identity_evidence_revisions SET endpoint = $2
              WHERE project_id = $1::text::uuid AND evidence_kind = 'mcp_remote_boundary'",
            &[&scope.project_id.as_ref(), &mcp_base],
        )
        .await
        .unwrap();
}

/// The Run, its one ToolCall without volatile identities, and its Activity Run Events.
async fn tool_facts(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_build_object(
                      'run', jsonb_build_object('status', run.status, 'settlement', run.settlement),
                      'tool_call', (SELECT jsonb_build_object(
                          'state', call.state, 'refusal', call.refusal,
                          'attempt', (call.attempt #- '{context_assembly}'
                                                 #- '{admission_decision,authority}'
                                                 #- '{outbound_disclosure_event,event_id}'
                                                 #- '{disclosure_confirmation,event_id}')
                                     || CASE WHEN call.attempt ? 'disclosure_confirmation'
                                        THEN jsonb_build_object('disclosure_confirmation',
                                          ((call.attempt -> 'disclosure_confirmation') - 'event_id')
                                          || jsonb_build_object('same_event',
                                             call.attempt #> '{disclosure_confirmation,event_id}'
                                             = call.attempt #> '{outbound_disclosure_event,event_id}'))
                                        ELSE '{}'::jsonb END
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
                                   || jsonb_build_object('kind', event.event_kind,
                                                         'position', event.event_position)
                                   ORDER BY event.event_position)
                                   FROM storyos.agent_run_events AS event
                                  WHERE event.run_id = run.run_id),
                      'manifests', (SELECT jsonb_agg(jsonb_build_object(
                          'requirement', requirement.payload - 'tool_call_id',
                          'position', manifest.decision_position,
                          'sufficiency', manifest.sufficiency,
                          'grant_scope', manifest.disclosure_grant_scope,
                          'gates', manifest.payload #- '{eligibility,project_scope}'
                                                    #- '{destination_context,endpoint}'
                                                    #- '{destination_context,authority}',
                          'authority', jsonb_build_object(
                              'kept', manifest.payload #> '{destination_context,authority}'
                                      = call.attempt #> '{admission_decision,authority}',
                              'exact', (SELECT array_agg(key ORDER BY key) FROM jsonb_each(
                                  manifest.payload #> '{destination_context,authority}')
                                  WHERE value <> 'null'::jsonb)),
                          'exact_scope', manifest.payload #>> '{eligibility,project_scope,project_id}'
                                           = manifest.project_id::text,
                          'claim_refers_to_it', call.attempt -> 'context_assembly' = jsonb_build_object(
                              'context_assembly_manifest_id', manifest.context_assembly_manifest_id,
                              'destination_context_manifest_id',
                              manifest.destination_context_manifest_id,
                              'outbound_disclosure_manifest_id',
                              manifest.outbound_disclosure_manifest_id)))
                        FROM storyos.context_assembly_manifests AS manifest
                        JOIN storyos.operation_requirements AS requirement
                          USING (owner_user_id, project_id, operation_requirement_id)
                        LEFT JOIN storyos.tool_calls AS call ON call.run_id = manifest.run_id
                       WHERE manifest.run_id = run.run_id AND manifest.manifest_role = 'tool_call'),
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
    let mut event = settled_event_after_quarantine(state, refusal, outcome, bytes);
    event["position"] = json!(0);
    event
}

/// The settlement event at position 1 of the Run, after its quarantine event at position 0.
fn settled_event_after_quarantine(
    state: &str,
    refusal: Value,
    outcome: Value,
    bytes: Value,
) -> Value {
    json!({"kind": "tool_call_settled", "call_id": "call_01", "position": 1,
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
        "outbound_disclosure_event": {"source_class": "agent_query", "state": "outcome_unknown"},
        "disclosure_confirmation": {"same_event": true, "state": disclosure},
    })
}

/// Gates one to six of the search ToolCall, with no volatile identity.
fn tool_manifest() -> Value {
    let effect = attempt("", "")["admission_decision"]["effect_request"].take();
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
                      "exact": ["enablement_revision", "evidence_revision",
                                "external_compatibility_decision", "grant_id",
                                "processing_destination_identity",
                                "project_model_use_binding_revision"]},
        "exact_scope": true,
        "claim_refers_to_it": true,
    })
}

pub(crate) const POST: &str = "POST /api/plan/v3 HTTP/1.1";

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_claimed_search_settles_with_its_result_text_and_one_run_event() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let call = reply(
        0,
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
        (
            "dce",
            0xdcea,
            // The Run Capability Grant narrows: the Exposure of the Step Snapshot names no Tool.
            "UPDATE storyos.model_attempts
                SET payload = jsonb_set(payload, '{step_snapshot,tool_exposure,tools}', '[]')
              WHERE project_id = '{project}'"
                .to_owned(),
            "tool_outside_run_grant",
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
            json!(facts["manifests"].as_array().map_or(0, Vec::len)),
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let call = reply(
        0,
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // The fake server listens for no message: a sent request finds no answer.
    let (mcp_base, mcp) = destination(Vec::new()).await;
    let (scope, run_id) = tool_run(&store, &admin, "dc9", 0xdc9a, &mcp_base).await;
    let (other_scope, other_run_id) = tool_run(&store, &admin, "dcb", 0xdcba, &mcp_base).await;
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let initialize = reply(
        1,
        json!({"protocolVersion": "2025-11-25\u{0}x", "capabilities": {"tools": {}},
               "serverInfo": {"name": "exa-search-server", "version": "3.5.1"}}),
    );
    let (mcp_base, mcp) = destination(vec![initialize]).await;
    let (scope, run_id) = tool_run(&store, &admin, "dcf", 0xdcfa, &mcp_base).await;
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

/// The real MCP client, and the cancellation of the Run by the author after the exchange, while
/// the Worker holds the old fence.
struct CancelAfterExchange<'a> {
    store: &'a crate::PostgresProjectReader,
    cancel: crate::command_sequence::tests::CommandCall<storyos_application::CancelAgentRunInput>,
}

impl ToolDestinationAdapter for CancelAfterExchange<'_> {
    type Prepared = McpExchange;

    async fn prepare(
        &self,
        request: &ToolCallRequest,
    ) -> Result<PreparedRequest<McpExchange>, PreDispatchRefusal> {
        ToolDestinationAdapter::prepare(&client(), request).await
    }

    async fn exchange(&self, prepared: McpExchange) -> ToolObservation {
        let observation = ToolDestinationAdapter::exchange(&client(), prepared).await;
        self.store
            .cancel_agent_run(&self.cancel.envelope, &self.cancel.input)
            .await
            .unwrap();
        observation
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_cancel_during_a_tool_exchange_settles_the_tool_call_unknown() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let call = reply(
        0,
        json!({"content": [{"type": "text", "text": RESULT}], "isError": false}),
    );
    let (mcp_base, mcp) = destination(mcp_replies(exa_tools(|_| {}), Some(call))).await;
    let (scope, run_id) = tool_run(&store, &admin, "dd1", 0xdd1a, &mcp_base).await;
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let tools = CancelAfterExchange {
        store: &store,
        cancel: crate::command_sequence::tests::issued(
            &store,
            &scope,
            0xdd1b,
            &crate::command_sequence::tests::CANCEL_AGENT_RUN,
            storyos_application::CancelAgentRunInput {
                run_id: run_id.clone(),
            },
        )
        .await,
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
    let outcome = json!({"external_read": "unknown", "outbound_disclosure": "unknown"});
    let facts = tool_facts(&admin, &run_id).await;

    assert_eq!(
        (
            matches!(completed, Err(CompleteAgentRunError::StaleFence)),
            facts["run"]["status"].clone(),
            facts["tool_call"].clone(),
            facts["events"].clone(),
            mcp.await.unwrap(),
        ),
        (
            true,
            json!("cancelled"),
            json!({"state": "unknown", "refusal": null,
                   "attempt": attempt("outcome_unknown", "unknown"), "has_attempt_id": true,
                   "effect_outcome": outcome, "result_text": null, "result_digest": null,
                   "result_bytes": null}),
            json!([settled_event("unknown", Value::Null, outcome, Value::Null)]),
            vec![POST.to_owned(); 4],
        )
    );
}
