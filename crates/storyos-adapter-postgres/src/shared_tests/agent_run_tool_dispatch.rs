//! The shared test support from `agent_run_tool_dispatch_tests.rs`.

use crate::shared_tests::agent_run_retrieval::{TestCredential, destination, research_run};
use crate::shared_tests::agent_run_tool_call::text_and_call_stream;
use crate::shared_tests::agent_run_tool_exposure::{advisory_stream, finish_stream};
use crate::shared_tests::json_row::json_of;
use serde_json::{Value, json};
use storyos_adapter_mcp_client::{McpClient, McpExchange};
use storyos_application::{
    PreDispatchRefusal, PreparedRequest, ToolCallRequest, ToolDestinationAdapter,
};
use storyos_core::{EXA_TOOL_REGISTRATION, ToolObservation};
use tokio_postgres::Client;

pub(crate) const SEARCH: &str = r#"{"query":"tide tables of 1890","result_count":null}"#;

pub(crate) const RESULT: &str = "Tide tables of 1890 list two high waters each day.";

pub(crate) const DISCOVERY: &str =
    include_str!("../../../../docs/research/exa-mcp-discovery-2026-11-03.md");

/// The value that `TestCredential` resolves. No record may hold it.
pub(crate) const KEY: &str = "storyos-test-credential";

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
pub(crate) async fn tool_run(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    prefix: &str,
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
        finish_stream(&format!("resp_{prefix}_2")),
        advisory_stream(&format!("resp_{prefix}_3")),
    ])
    .await;
    let (scope, run_id) = research_run(store, admin, prefix, &model_base).await;
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

/// The Run, its one research ToolCall without volatile identities, and the Activity Run Events
/// that are not of a Host Tool call.
pub(crate) async fn tool_facts(admin: &Client, run_id: &str) -> Value {
    json_of(admin, "SELECT jsonb_build_object(
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
                                                 #- '{destination_attempt,endpoint}'
                                                 #- '{fence,fence_id}'
                                                 #- '{fence,run_fence_token}'
                                                 #- '{late_observation,disclosure_confirmation,event_id}',
                          'has_attempt_id', call.destination_attempt_id IS NOT NULL,
                          'effect_outcome', call.effect_outcome, 'result_text', call.result_text,
                          'result_digest', call.result_digest, 'result_bytes', call.result_bytes)
                        FROM storyos.tool_calls AS call
                       WHERE call.run_id = run.run_id AND call.registration_id IS DISTINCT FROM 'host_tools'),
                      'events', (SELECT jsonb_agg((event.payload - 'tool_call_id'
                                   - 'destination_attempt_id')
                                   || jsonb_build_object('kind', event.event_kind,
                                                         'position', event.event_position)
                                   ORDER BY event.event_position)
                                   FROM storyos.agent_run_events AS event
                                  WHERE event.run_id = run.run_id
                                    AND NOT EXISTS (
                                        SELECT 1 FROM storyos.tool_calls AS host
                                         WHERE host.run_id = run.run_id
                                           AND host.registration_id = 'host_tools'
                                           AND host.tool_call_id::text
                                               = event.payload ->> 'tool_call_id')),
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
                        LEFT JOIN storyos.tool_calls AS call
                          ON call.run_id = manifest.run_id AND call.registration_id IS DISTINCT FROM 'host_tools'
                       WHERE manifest.run_id = run.run_id AND manifest.manifest_role = 'tool_call'),
                      'holds_key', (SELECT bool_or(position($2 IN record::text) > 0)
                                      FROM (SELECT to_jsonb(call) AS record
                                              FROM storyos.tool_calls AS call
                                             WHERE call.run_id = run.run_id
                                            UNION ALL
                                            SELECT to_jsonb(event)
                                              FROM storyos.agent_run_events AS event
                                             WHERE event.run_id = run.run_id) AS records))::text
               FROM storyos.agent_runs AS run WHERE run.run_id = $1::text::uuid", &[&run_id, &KEY]).await
}

pub(crate) fn settled_event(state: &str, refusal: Value, outcome: Value, bytes: Value) -> Value {
    let mut event = settled_event_after_quarantine(state, refusal, outcome, bytes);
    event["position"] = json!(0);
    event
}

/// The settlement event at position 1 of the Run, after its quarantine event at position 0.
pub(crate) fn settled_event_after_quarantine(
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

/// The claimed attempt record with its settlement and its request reservation.
pub(crate) fn attempt(settlement: &str, disclosure: &str, released: bool) -> Value {
    json!({
        "reservation": {"kind": "worst_case", "requests": 1, "released": released},
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

pub(crate) const POST: &str = "POST /api/plan/v3 HTTP/1.1";

/// The attempt record of a ToolCall that the fence of `cause` settled `unknown`, with no volatile
/// identity.
pub(crate) fn interrupted_attempt(cause: &str) -> Value {
    let mut attempt = attempt("outcome_unknown", "unknown", /*released*/ false);
    attempt["fence"] = json!({"cause": cause, "local_stop": "drop_exchange",
                              "remote_abort": "unsupported"});
    attempt
}
