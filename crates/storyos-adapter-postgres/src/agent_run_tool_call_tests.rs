use serde_json::json;
use storyos_application::{NoContractFaults, SetProjectToolEnablementInput, complete_agent_run};
use storyos_core::{
    CurrentToolAuthority, EXA_TOOL_REGISTRATION, EnabledTool, ExposureFacts, ExposureSnapshot,
    NativeStreamItem, RESEARCH_TOOL_OFFER, StreamItemRole, StreamItemState, ToolCallBatch,
    ToolCompatibilityDecision, ToolEnablement, ToolExposure, ToolRegistrationLifecycle,
    validate_tool_call_batch,
};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::{RuntimeClock, claim_run, stores};
use crate::agent_run_retrieval::tests::{AfterExchange, adapter, destination, real_run};
use crate::command_sequence::tests::{SET_PROJECT_TOOL_ENABLEMENT, issued};

const SEARCH_ARGUMENTS: &str = r#"{"query":"tide tables of 1890","result_count":null}"#;
const SIX_URLS: &str = r#"{"urls":["https://e.org/1","https://e.org/2","https://e.org/3","https://e.org/4","https://e.org/5","https://e.org/6"]}"#;

/// A stream that reports `reference`, one complete message, and one complete function call with
/// `call_id`, and ends with the terminal `status` of the response.
pub(crate) fn text_and_call_stream(
    reference: &str,
    call_id: &str,
    name: &str,
    arguments: &str,
    status: &str,
) -> String {
    let message = json!({"id": "msg_01", "type": "message", "role": "assistant",
        "status": "completed", "content": [{"type": "output_text", "text": "Looking it up."}]});
    let call = json!({"id": "fc_01", "type": "function_call", "status": "completed",
        "call_id": call_id, "name": name, "arguments": arguments});
    let events = [
        json!({"type": "response.created",
               "response": {"id": reference, "status": "in_progress", "output": []}}),
        json!({"type": "response.output_item.done", "output_index": 0, "item": message}),
        json!({"type": "response.output_item.done", "output_index": 1, "item": call}),
        json!({"type": format!("response.{status}"), "response": {
            "id": reference, "status": status, "output": [message, call],
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
    set_research_tool(store, scope, suffix, ToolEnablement::Enabled).await;
}

/// Settles the `requested` enablement of the research Tool of the Project, as the author does.
pub(crate) async fn set_research_tool(
    store: &crate::PostgresProjectReader,
    scope: &storyos_application::ProjectScope,
    suffix: u16,
    requested: ToolEnablement,
) {
    let call = issued(
        store,
        scope,
        suffix,
        &SET_PROJECT_TOOL_ENABLEMENT,
        SetProjectToolEnablementInput {
            registration_id: "web_research".to_owned(),
            requested,
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
        StreamItemState::Complete,
        &[item],
        &ExposureSnapshot::decode(Some(&exposure.snapshot())).unwrap(),
        &[CurrentToolAuthority {
            registration_id: "web_research".to_owned(),
            registration_revision: EXA_TOOL_REGISTRATION.revision.to_owned(),
            enabled: true,
            lifecycle: ToolRegistrationLifecycle::Active,
        }],
    ) {
        ToolCallBatch::Valid(mut calls) => calls.remove(0),
        ToolCallBatch::Invalid(refusal) => panic!("{}", refusal.label()),
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_durable_function_request_decision_derives_one_tool_call_once() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![text_and_call_stream(
        "resp_f75",
        "call_01",
        "web_search",
        SEARCH_ARGUMENTS,
        "completed",
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
                      arguments, argument_digest, 'pending'
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
            // A process with no Tool adapter refuses the ToolCall before its claim.
            json!({
                "status": "completed",
                "settlement": null,
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
                "state": "refused",
                "refusal": "destination_request_unsupported",
                "destination_attempt_id": null,
                "attempt": null,
                "effect_outcome": null,
                "result_text": null,
                "result_digest": null,
                "result_bytes": null
            })],
            1,
            Some("23505".to_owned()),
            false,
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
        "call_01",
        "web_fetch",
        SIX_URLS,
        "completed",
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

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_incomplete_response_with_a_complete_function_call_derives_no_tool_call() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![text_and_call_stream(
        "resp_f77",
        "call_01",
        "web_search",
        SEARCH_ARGUMENTS,
        "incomplete",
    )])
    .await;
    let (scope, run_id) = real_run(&store, &admin, "f77", &base).await;
    enable_research_tool(&store, &scope, 0xf77a).await;
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
                "tool_call_batch": {"kind": "invalid", "refusal": "response_incomplete"},
                "item_count": 2
            }),
            Vec::<serde_json::Value>::new(),
            false,
            vec!["POST /api/plan/v3/responses HTTP/1.1".to_owned()],
        )
    );
}

/// Runs one real Create whose response requests `web_search` with `call_id` and `arguments`.
/// Returns the decision facts without the Attempt identity, the ToolCall rows, whether a pending
/// ToolCall exists, and the stored function call item.
async fn create_with_call(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    suffix: u16,
    call_id: &str,
    arguments: &str,
) -> (
    serde_json::Value,
    Vec<serde_json::Value>,
    bool,
    serde_json::Value,
) {
    let (base, served) = destination(vec![text_and_call_stream(
        &format!("resp_{prefix}"),
        call_id,
        "web_search",
        arguments,
        "completed",
    )])
    .await;
    let (scope, run_id) = real_run(store, admin, prefix, &base).await;
    enable_research_tool(store, &scope, suffix).await;
    let claim = claim_run(admin, scope, run_id.clone()).await;
    complete_agent_run(store, &adapter(), &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    served.await.unwrap();
    let mut facts = decision_facts(admin, &run_id).await;
    facts.as_object_mut().unwrap().remove("model_attempt_id");
    let item: Option<String> = admin
        .query_one(
            "SELECT (payload->'items'->1)::text FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(0);
    (
        facts,
        tool_call_rows(admin, &run_id).await,
        crate::agent_run_tool_call::pending_tool_call_exists(admin, &claim)
            .await
            .unwrap(),
        item.map_or(serde_json::Value::Null, |item| {
            serde_json::from_str(&item).unwrap()
        }),
    )
}

/// The facts of a Create whose batch `refusal` rejected it, with the stored function call `item`.
fn rejected_create(
    refusal: &str,
    item: serde_json::Value,
) -> (
    serde_json::Value,
    Vec<serde_json::Value>,
    bool,
    serde_json::Value,
) {
    (
        json!({
            "status": "refused",
            "settlement": {"kind": "execution_refused", "capability": "research_tool_unavailable"},
            "decision_id": null,
            "dispatch_state": "settled",
            "decision": null,
            "tool_call_batch": {"kind": "invalid", "refusal": refusal},
            "item_count": 2
        }),
        Vec::new(),
        false,
        item,
    )
}

/// The stored `web_search` call item of `text_and_call_stream`, with the stored `call_id` and
/// `arguments` text. Each name of `escaped` is a field in its escaped form.
fn stored_call_item(call_id: &str, arguments: &str, escaped: &[&str]) -> serde_json::Value {
    let mut item = json!({
        "item_id": "fc_01",
        "role": "tool",
        "state": "complete",
        "phase": "complete",
        "text": null,
        "summary": null,
        "call_id": call_id,
        "arguments": arguments,
        "refusal": null,
        "native_type": "function_call",
        "name": "web_search"
    });
    if !escaped.is_empty() {
        item["escaped_fields"] = json!(escaped);
    }
    item
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_null_character_in_the_arguments_rejects_the_batch_and_keeps_the_raw_item() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // The arguments text escapes U+0000, so the parsed query holds it.
    let escaped = r#"{"query":"tide\u0000tables","result_count":null}"#;
    // The arguments text holds U+0000 itself, so it is not JSON text. It also holds an escaped
    // backslash, which the stored form doubles.
    let raw = "{\"query\":\"tide\\\\\u{0}tables\",\"result_count\":null}";
    let in_query = create_with_call(&store, &admin, "f7e", 0xf7ea, "call_01", escaped).await;
    let in_text = create_with_call(&store, &admin, "f7f", 0xf7fa, "call_01", raw).await;

    assert_eq!(
        (in_query, in_text),
        (
            rejected_create("intake_query", stored_call_item("call_01", escaped, &[])),
            rejected_create(
                "malformed_arguments",
                stored_call_item(
                    "call_01",
                    r#"{"query":"tide\\\\\u0000tables","result_count":null}"#,
                    &["arguments"],
                ),
            ),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_null_character_in_the_call_identifier_rejects_the_batch_and_keeps_the_raw_item() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // The call identifier holds U+0000 itself. The arguments are valid.
    let facts = create_with_call(
        &store,
        &admin,
        "e31",
        0xe31a,
        "call_\u{0}01",
        SEARCH_ARGUMENTS,
    )
    .await;

    assert_eq!(
        facts,
        rejected_create(
            "invalid_call_id",
            stored_call_item(r"call_\u000001", SEARCH_ARGUMENTS, &["call_id"]),
        )
    );
}

/// A change of the current Tool authority that occurs while the model response is pending.
#[derive(Clone, Copy)]
enum Revocation {
    /// The Tool Contract Drift of the pinned research revision quarantines it.
    Quarantine,
    /// The author disables the research Tool of the Project.
    Disable,
}

impl Revocation {
    /// The administration statement that makes the change in the Project of `run_id`.
    fn statement(self, run_id: &str) -> String {
        match self {
            Self::Quarantine => format!(
                "UPDATE storyos.tool_registration_revisions
                    SET lifecycle = 'quarantined', quarantined_at = clock_timestamp(),
                        quarantine_differences = '[{{\"kind\": \"tool_removed\", \"tool\": \"web_fetch_exa\"}}]'
                  WHERE tool_registration_revision = '{}'",
                EXA_TOOL_REGISTRATION.revision
            ),
            Self::Disable => format!(
                "INSERT INTO storyos.project_tool_enablements
                   SELECT enablement.owner_user_id, enablement.project_id,
                          enablement.registration_id, enablement.enablement_revision + 1,
                          'disabled', NULL, enablement.receipt_id
                     FROM storyos.project_tool_enablements AS enablement
                     JOIN storyos.agent_runs AS run
                       ON (run.owner_user_id, run.project_id) =
                          (enablement.owner_user_id, enablement.project_id)
                    WHERE run.run_id = '{run_id}'"
            ),
        }
    }
}

/// Makes the shared research revision active again after a quarantine, so that later tests
/// expose it.
async fn reactivate_research_revision(admin: &Client) {
    admin
        .execute(
            "UPDATE storyos.tool_registration_revisions
                SET lifecycle = 'active', quarantine_differences = NULL, quarantined_at = NULL
              WHERE tool_registration_revision = $1::text::uuid",
            &[&EXA_TOOL_REGISTRATION.revision],
        )
        .await
        .unwrap();
}

/// Runs one real Create whose response requests a valid `web_search` call, and applies
/// `revocation` between the exchange and the record. Returns the decision facts without the
/// Attempt identity, the ToolCall rows, and whether a pending ToolCall exists.
async fn revoked_create(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    suffix: u16,
    revocation: Revocation,
) -> (serde_json::Value, Vec<serde_json::Value>, bool) {
    let (base, served) = destination(vec![text_and_call_stream(
        &format!("resp_{prefix}"),
        "call_01",
        "web_search",
        SEARCH_ARGUMENTS,
        "completed",
    )])
    .await;
    let (scope, run_id) = real_run(store, admin, prefix, &base).await;
    enable_research_tool(store, &scope, suffix).await;
    let claim = claim_run(admin, scope, run_id.clone()).await;
    let route = AfterExchange {
        admin,
        statement: revocation.statement(&run_id),
    };
    complete_agent_run(store, &route, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    reactivate_research_revision(admin).await;
    served.await.unwrap();
    let mut facts = decision_facts(admin, &run_id).await;
    facts.as_object_mut().unwrap().remove("model_attempt_id");
    (
        facts,
        tool_call_rows(admin, &run_id).await,
        crate::agent_run_tool_call::pending_tool_call_exists(admin, &claim)
            .await
            .unwrap(),
    )
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_tool_authority_revoked_before_the_create_record_derives_no_decision() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let quarantined = revoked_create(&store, &admin, "f7a", 0xf7aa, Revocation::Quarantine).await;
    let disabled = revoked_create(&store, &admin, "f7b", 0xf7ba, Revocation::Disable).await;
    let rejected = |refusal: &str| {
        (
            json!({
                "status": "refused",
                "settlement": {"kind": "execution_refused", "capability": "research_tool_unavailable"},
                "decision_id": null,
                "dispatch_state": "settled",
                "decision": null,
                "tool_call_batch": {"kind": "invalid", "refusal": refusal},
                "item_count": 2
            }),
            Vec::<serde_json::Value>::new(),
            false,
        )
    };

    assert_eq!(
        (quarantined, disabled),
        (
            rejected("tool_registration_unavailable"),
            rejected("tool_not_enabled")
        )
    );
}

/// A route whose first Create and original Retrieve stay unknown, whose successor Create
/// returns one complete function call, and whose late Retrieve first delivers the successor
/// observation a second time through the store, before the Run settles.
struct DuplicateSuccessorDelivery<'a> {
    store: &'a crate::PostgresProjectReader,
    admin: &'a Client,
    claim: &'a storyos_application::ClaimedAgentRun,
    successor: std::sync::Mutex<Option<storyos_application::CreateRequest>>,
    delivered_twice: std::sync::Mutex<Option<bool>>,
    /// The administration statement that changes the current Tool authority after the
    /// successor exchange, before its record.
    revocation: Option<String>,
}

enum DuplicatePlan {
    Unknown,
    FunctionCall,
    LateResult,
}

impl DuplicateSuccessorDelivery<'_> {
    fn function_response() -> storyos_application::ModelResponse {
        let call = |item_id: &str, name: &str, arguments: &str| NativeStreamItem {
            item_id: item_id.to_owned(),
            role: StreamItemRole::Tool,
            state: StreamItemState::Complete,
            text: None,
            summary: None,
            call_id: Some(format!("call_{item_id}")),
            arguments: Some(arguments.to_owned()),
            refusal: None,
            native_type: Some("function_call".to_owned()),
            name: Some(name.to_owned()),
        };
        storyos_application::ModelResponse {
            state: StreamItemState::Complete,
            items: vec![
                call("01", "web_search", SEARCH_ARGUMENTS),
                call(
                    "02",
                    "web_fetch",
                    r#"{"urls":["https://example.org/tides"]}"#,
                ),
            ],
            output: None,
            usage: storyos_application::ModelUsage::Unknown,
            cache_use: storyos_application::CacheUse::Unknown,
            response_reference: Some("resp_f78s".to_owned()),
        }
    }
}

impl storyos_application::ModelProviderAdapter for DuplicateSuccessorDelivery<'_> {
    const ADAPTERS: &'static [storyos_core::ModelAdapter] =
        &[storyos_core::ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = DuplicatePlan;

    async fn prepare(
        &self,
        request: &storyos_application::DestinationRequest,
    ) -> Result<
        storyos_application::PreparedRequest<Self::Prepared>,
        storyos_application::PreDispatchRefusal,
    > {
        use storyos_application::DestinationRequest;
        let (digest, prepared) = match request {
            DestinationRequest::Create(create) if create.successor_of.is_some() => {
                *self.successor.lock().unwrap() = Some(create.clone());
                ("sha256:create", DuplicatePlan::FunctionCall)
            }
            DestinationRequest::Create(_) => ("sha256:create", DuplicatePlan::Unknown),
            DestinationRequest::Retrieve(retrieve)
                if retrieve.purpose == storyos_application::RetrievePurpose::LateResult =>
            {
                ("sha256:retrieve", DuplicatePlan::LateResult)
            }
            DestinationRequest::Retrieve(_) => ("sha256:retrieve", DuplicatePlan::Unknown),
            DestinationRequest::Abort(_) => {
                return Err(storyos_application::PreDispatchRefusal::UnsupportedRequest);
            }
        };
        Ok(storyos_application::PreparedRequest {
            projection: storyos_application::WirePayloadProjection {
                execution_profile: "storyos.volcengine-agent-plan.execution.v1".to_owned(),
                mapping_revision: storyos_core::AGENT_PLAN_REGISTRATION
                    .mapping_revision
                    .to_owned(),
                digest: digest.to_owned(),
                serialized_payload: None,
                cache_setting: None,
            },
            prepared,
        })
    }

    async fn exchange(
        &self,
        prepared: Self::Prepared,
        _sink: &mut impl storyos_application::ModelStreamSink,
    ) -> storyos_application::Observation {
        use storyos_application::{DispatchRecord, ModelDispatchStore, Observation};
        match prepared {
            DuplicatePlan::FunctionCall => {
                if let Some(statement) = &self.revocation {
                    self.admin.batch_execute(statement).await.unwrap();
                }
                return Observation::Terminal(Self::function_response());
            }
            DuplicatePlan::LateResult => {
                let successor_attempt_id: String = self
                    .admin
                    .query_one(
                        "SELECT model_attempt_id::text FROM storyos.model_attempts
                          WHERE run_id = $1::text::uuid AND attempt_role = 'successor'",
                        &[&self.claim.run_id],
                    )
                    .await
                    .unwrap()
                    .get(0);
                let request = storyos_application::DestinationRequest::Create(
                    self.successor.lock().unwrap().clone().unwrap(),
                );
                let delivered = self
                    .store
                    .record(
                        self.claim,
                        DispatchRecord::Exchange {
                            claim: &storyos_application::DispatchClaim {
                                model_attempt_id: successor_attempt_id,
                            },
                            request: &request,
                            observation: Observation::Terminal(Self::function_response()),
                        },
                    )
                    .await;
                *self.delivered_twice.lock().unwrap() = Some(delivered.is_ok());
            }
            DuplicatePlan::Unknown => {}
        }
        Observation::OutcomeUnknown {
            response_reference: Some(storyos_application::ResponseReference {
                reference_id: "resp_f78".to_owned(),
                retrieval: storyos_application::ReferenceRetrieval::Supported {
                    bounds: storyos_core::RetrievalBounds::DeclaredReadOnly,
                },
                reported_binding: storyos_application::ReportedBinding {
                    owner_user_id: None,
                    conversation_id: None,
                    destination_identity: None,
                    mapping_revision: storyos_core::AGENT_PLAN_REGISTRATION
                        .mapping_revision
                        .to_owned(),
                },
            }),
        }
    }
}

/// Runs the unknown Create, the unknown original Retrieve, the successor Create that returns two
/// valid calls, and the late Retrieve that delivers the successor observation a second time.
/// `revocation` applies between the successor exchange and its record. Returns the Run and
/// Attempt facts, the call identifier, position, and Decision of each ToolCall row, and whether
/// the second delivery recorded.
async fn successor_run(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    suffix: u16,
    revocation: Option<Revocation>,
) -> (
    serde_json::Value,
    Vec<(serde_json::Value, serde_json::Value, serde_json::Value)>,
    Option<bool>,
) {
    let (scope, _, run_id) = crate::agent_run_dispatch::tests::queued_run(
        store,
        prefix,
        storyos_core::DeploymentDestination::VolcengineAgentPlan {
            qualification_authorization: None,
        },
    )
    .await;
    enable_research_tool(store, &scope, suffix).await;
    let claim = claim_run(admin, scope, run_id.clone()).await;
    let route = DuplicateSuccessorDelivery {
        store,
        admin,
        claim: &claim,
        successor: std::sync::Mutex::new(None),
        delivered_twice: std::sync::Mutex::new(None),
        revocation: revocation.map(|revocation| revocation.statement(&run_id)),
    };
    complete_agent_run(store, &route, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    reactivate_research_revision(admin).await;
    let facts: serde_json::Value = admin
        .query_one(
            "SELECT jsonb_build_object(
                      'status', run.status, 'settlement', run.settlement,
                      'successor_decision_id', successor.decision_id,
                      'marker_decision_id',
                        decision.payload->'unknown_create_successor'->'successor_decision_id',
                      'supplies_tool_call',
                        decision.payload->'unknown_create_successor'->'supplies_tool_call',
                      'decision', successor.payload->'decision',
                      'tool_call_batch', successor.payload->'tool_call_batch',
                      'roles', (SELECT string_agg(attempt.attempt_role, ','
                                                  ORDER BY attempt.model_attempt_id)
                                  FROM storyos.model_attempts AS attempt
                                 WHERE attempt.run_id = run.run_id))::text
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS decision
                 ON decision.run_id = run.run_id AND decision.attempt_role = 'decision'
               JOIN storyos.model_attempts AS successor
                 ON successor.run_id = run.run_id AND successor.attempt_role = 'successor'
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .map(|row| serde_json::from_str(row.get::<_, &str>(0)).unwrap())
        .unwrap();
    let rows = tool_call_rows(admin, &run_id)
        .await
        .into_iter()
        .map(|row| {
            (
                row["call_id"].clone(),
                row["call_position"].clone(),
                row["decision_id"].clone(),
            )
        })
        .collect();
    let delivered_twice = *route.delivered_twice.lock().unwrap();
    (facts, rows, delivered_twice)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_repeated_successor_observation_keeps_the_decision_and_its_tool_calls() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (facts, rows, delivered_twice) =
        successor_run(&store, &admin, "f78", 0xf78a, /*revocation*/ None).await;
    let decision_id = facts["successor_decision_id"].clone();

    assert_eq!(
        (facts, rows, delivered_twice),
        (
            json!({
                "status": "completed",
                "settlement": null,
                "successor_decision_id": decision_id,
                "marker_decision_id": decision_id,
                "supplies_tool_call": true,
                "decision": {
                    "kind": "tool_request",
                    "decision_id": decision_id,
                    "selected": true,
                    "call_ids": ["call_01", "call_02"],
                    "authoritative": false,
                    "advances_continuation": false
                },
                "tool_call_batch": {"kind": "valid", "call_ids": ["call_01", "call_02"]},
                "roles": "decision,retrieval,successor,late_retrieval"
            }),
            vec![
                (json!("call_01"), json!(0), decision_id.clone()),
                (json!("call_02"), json!(1), decision_id),
            ],
            Some(true),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_tool_authority_revoked_before_the_successor_record_derives_no_decision() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let quarantined =
        successor_run(&store, &admin, "f7c", 0xf7ca, Some(Revocation::Quarantine)).await;
    let disabled = successor_run(&store, &admin, "f7d", 0xf7da, Some(Revocation::Disable)).await;
    let rejected = |refusal: &str| {
        (
            json!({
                "status": "refused",
                "settlement": {"kind": "execution_refused", "capability": "research_tool_unavailable"},
                "successor_decision_id": null,
                "marker_decision_id": null,
                "supplies_tool_call": false,
                "decision": null,
                "tool_call_batch": {"kind": "invalid", "refusal": refusal},
                "roles": "decision,retrieval,successor,late_retrieval"
            }),
            Vec::new(),
            Some(true),
        )
    };

    assert_eq!(
        (quarantined, disabled),
        (
            rejected("tool_registration_unavailable"),
            rejected("tool_not_enabled")
        )
    );
}
