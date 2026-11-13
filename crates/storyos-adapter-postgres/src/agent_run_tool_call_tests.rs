use serde_json::json;
use storyos_application::{NoContractFaults, complete_agent_run};
use storyos_core::{
    BoundTool, CallValidationRefusal, CurrentToolAuthority, EXA_TOOL_REGISTRATION, ExposureFacts,
    ExposureSnapshot, NativeStreamItem, RESEARCH_TOOL_OFFER, StreamItemRole, StreamItemState,
    ToolCallBatch, ToolCompatibilityDecision, ToolExposure, ToolRegistrationLifecycle,
    validate_tool_call_batch,
};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::{RuntimeClock, claim_run, stores};
use crate::agent_run_finish_rule::tests::{finish_item, text_item};
use crate::agent_run_retrieval::tests::{AfterExchange, adapter, destination, research_run};
use crate::agent_run_tool_exposure::tests::advisory_stream;

const SEARCH_ARGUMENTS: &str = r#"{"query":"tide tables of 1890","result_count":null}"#;
/// The digest of the validated arguments of `SEARCH_ARGUMENTS`.
const SEARCH_DIGEST: &str =
    "sha256:012398a6d5fe487115a70022409a7dd2e27bfbcfdbb9930129b6d2ea5f84d9b2";
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

/// The ToolCall rows of the Run that are not Host Tool calls, without their row identity.
async fn tool_call_rows(admin: &Client, run_id: &str) -> Vec<serde_json::Value> {
    admin
        .query(
            "SELECT (to_jsonb(call) - 'tool_call_id' - 'owner_user_id' - 'project_id')::text
               FROM storyos.tool_calls AS call
              WHERE run_id = $1::text::uuid AND registration_id IS DISTINCT FROM 'host_tools'
              ORDER BY call_position",
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
                AND attempt.decision_position = 0
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .map(|row| serde_json::from_str(row.get::<_, &str>(0)).unwrap())
        .unwrap()
}

/// The one member of the batch of a complete `web_search` call, validated against the pinned
/// research Exposure.
fn validated_search_call() -> storyos_core::RequestedToolCall {
    let exposure = ToolExposure::compute(&ExposureFacts {
        bound: &[BoundTool {
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
    let (base, served) = destination(vec![
        text_and_call_stream(
            "resp_f75",
            "call_01",
            "web_search",
            SEARCH_ARGUMENTS,
            "completed",
        ),
        // The continuation RunStep answers the refused ToolCall.
        advisory_stream("resp_f75b"),
    ])
    .await;
    let (scope, run_id) = research_run(&store, &admin, "f75", &base).await;
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
                "argument_digest": SEARCH_DIGEST,
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
            vec!["POST /api/plan/v3/responses HTTP/1.1".to_owned(); 2],
        )
    );
}

/// The facts of `refused_call_facts` for a completed Run whose one ToolCall has `refusal`.
fn refused_call(refusal: CallValidationRefusal, arguments: &str) -> serde_json::Value {
    let state = [
        "completed",
        "tool_request",
        "refused",
        refusal.label(),
        arguments,
    ];
    json!([state, refusal.result_text()])
}

/// The status of the Run and the kind of its first Decision, and the state, refusal, arguments,
/// and result of its one ToolCall.
fn refused_call_facts(facts: &serde_json::Value, rows: &[serde_json::Value]) -> serde_json::Value {
    let row = &rows[0];
    let state = [
        &facts["status"],
        &facts["decision"]["kind"],
        &row["state"],
        &row["refusal"],
    ];
    json!([
        [state[0], state[1], state[2], state[3], &row["arguments"]],
        row["result_text"]
    ])
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_intake_violation_is_a_refused_call_and_the_run_continues() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![
        text_and_call_stream("resp_f76", "call_01", "web_fetch", SIX_URLS, "completed"),
        advisory_stream("resp_f76b"),
    ])
    .await;
    let (scope, run_id) = research_run(&store, &admin, "f76", &base).await;
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let refusal = CallValidationRefusal::Intake(storyos_core::IntakeRefusal::UrlCount);

    assert_eq!(
        (
            refused_call_facts(
                &decision_facts(&admin, &run_id).await,
                &tool_call_rows(&admin, &run_id).await
            ),
            served.await.unwrap(),
        ),
        (
            refused_call(refusal, SIX_URLS),
            vec!["POST /api/plan/v3/responses HTTP/1.1".to_owned(); 2],
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
    let (scope, run_id) = research_run(&store, &admin, "f77", &base).await;
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
                "settlement": {"kind": "execution_refused", "capability": "tool_request_invalid"},
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

/// Runs one real Create whose response requests `web_search` with `call_id` and `arguments`, and
/// then one advisory reply when the Run `continues`. Returns the decision facts without the Attempt identity, the
/// ToolCall rows, whether a pending ToolCall exists, and the stored function call item.
async fn create_with_call(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    call_id: &str,
    arguments: &str,
    continues: bool,
) -> (
    serde_json::Value,
    Vec<serde_json::Value>,
    bool,
    serde_json::Value,
) {
    let mut replies = vec![text_and_call_stream(
        &format!("resp_{prefix}"),
        call_id,
        "web_search",
        arguments,
        "completed",
    )];
    replies.extend(continues.then(|| advisory_stream(&format!("resp_{prefix}b"))));
    let (base, served) = destination(replies).await;
    let (scope, run_id) = research_run(store, admin, prefix, &base).await;
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
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'
                AND decision_position = 0",
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
            "settlement": {"kind": "execution_refused", "capability": "tool_request_invalid"},
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
async fn a_null_character_in_the_arguments_is_a_refused_call_and_keeps_the_raw_item() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // The arguments text escapes U+0000, so the parsed query holds it.
    let escaped = r#"{"query":"tide\u0000tables","result_count":null}"#;
    // The arguments text holds U+0000 itself, so it is not JSON text. It also holds an escaped
    // backslash, which the stored form doubles.
    let raw = "{\"query\":\"tide\\\\\u{0}tables\",\"result_count\":null}";
    let in_query = create_with_call(&store, &admin, "f7e", "call_01", escaped, true).await;
    let in_text = create_with_call(&store, &admin, "f7f", "call_01", raw, true).await;
    let (query, malformed) = (
        CallValidationRefusal::Intake(storyos_core::IntakeRefusal::Query),
        CallValidationRefusal::Malformed,
    );

    // The arguments of the refused ToolCall are the text as a JSON string.
    assert_eq!(
        (
            (
                refused_call_facts(&in_query.0, &in_query.1),
                in_query.2,
                in_query.3
            ),
            (
                refused_call_facts(&in_text.0, &in_text.1),
                in_text.2,
                in_text.3
            ),
        ),
        (
            (
                refused_call(query, escaped),
                false,
                stored_call_item("call_01", escaped, &[]),
            ),
            (
                refused_call(
                    malformed,
                    "{\"query\":\"tide\\\\\u{fffd}tables\",\"result_count\":null}"
                ),
                false,
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
        "call_\u{0}01",
        SEARCH_ARGUMENTS,
        false,
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

/// The administration statement of a Tool Contract Drift that quarantines the pinned research
/// revision while the model response is pending.
pub(crate) fn quarantine_statement() -> String {
    format!(
        "UPDATE storyos.tool_registration_revisions
            SET lifecycle = 'quarantined', quarantined_at = clock_timestamp(),
                quarantine_differences = '[{{\"kind\": \"tool_removed\", \"tool\": \"web_fetch_exa\"}}]'
          WHERE tool_registration_revision = '{}'",
        EXA_TOOL_REGISTRATION.revision
    )
}

/// Makes the shared research revision active again after a quarantine, so that later tests
/// expose it.
pub(crate) async fn reactivate_research_revision(admin: &Client) {
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

/// Runs one real Create whose response requests a valid `web_search` call, and quarantines the
/// pinned research revision between the exchange and the record. Returns the decision facts without the
/// Attempt identity, the ToolCall rows, and whether a pending ToolCall exists.
async fn revoked_create(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    prefix: &str,
) -> (serde_json::Value, Vec<serde_json::Value>, bool) {
    let (base, served) = destination(vec![text_and_call_stream(
        &format!("resp_{prefix}"),
        "call_01",
        "web_search",
        SEARCH_ARGUMENTS,
        "completed",
    )])
    .await;
    let (scope, run_id) = research_run(store, admin, prefix, &base).await;
    let claim = claim_run(admin, scope, run_id.clone()).await;
    let route = AfterExchange {
        admin,
        statement: quarantine_statement(),
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
    let quarantined = revoked_create(&store, &admin, "f7a").await;

    assert_eq!(
        quarantined,
        (
            json!({
                "status": "refused",
                "settlement": {"kind": "execution_refused", "capability": "research_tool_unavailable"},
                "decision_id": null,
                "dispatch_state": "settled",
                "decision": null,
                "tool_call_batch": {"kind": "invalid", "refusal": "tool_registration_unavailable"},
                "item_count": 2
            }),
            Vec::<serde_json::Value>::new(),
            false,
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
    Finish,
    Advisory,
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
            // The next RunStep sends the refused ToolCalls of the successor (ADR 0088). The model
            // then calls `finish`, and the closing request gets the final answer.
            DestinationRequest::Create(create) if !create.tool_outputs.is_empty() => {
                let plan = match create.tool_choice {
                    storyos_core::ToolChoice::None => DuplicatePlan::Advisory,
                    _ => DuplicatePlan::Finish,
                };
                ("sha256:continuation", plan)
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
            DuplicatePlan::Finish => {
                return Observation::Terminal(storyos_application::ModelResponse {
                    items: vec![finish_item("call_finish")],
                    ..Self::function_response()
                });
            }
            DuplicatePlan::Advisory => {
                return Observation::Terminal(storyos_application::ModelResponse {
                    items: vec![text_item("No source was read.")],
                    ..Self::function_response()
                });
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
    revocation: Option<String>,
) -> (
    serde_json::Value,
    Vec<(serde_json::Value, serde_json::Value, serde_json::Value)>,
    Option<bool>,
) {
    let (scope, _, run_id) = crate::agent_run_dispatch::tests::queued_run_offering(
        store,
        prefix,
        storyos_core::DeploymentDestination::VolcengineAgentPlan {
            qualification_authorization: None,
        },
        Some(&RESEARCH_TOOL_OFFER),
    )
    .await;
    let claim = claim_run(admin, scope, run_id.clone()).await;
    let route = DuplicateSuccessorDelivery {
        store,
        admin,
        claim: &claim,
        successor: std::sync::Mutex::new(None),
        delivered_twice: std::sync::Mutex::new(None),
        revocation,
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
                AND decision.decision_position = 0
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
        successor_run(&store, &admin, "f78", /*revocation*/ None).await;
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
                // The refused ToolCalls continue at position 1, whose `finish` call counts, and
                // the closing request at position 2 completes the Run.
                "roles": "decision,retrieval,successor,late_retrieval,decision,decision"
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
    let quarantined = successor_run(&store, &admin, "f7c", Some(quarantine_statement())).await;

    assert_eq!(
        quarantined,
        (
            json!({
                "status": "refused",
                "settlement": {"kind": "execution_refused", "capability": "research_tool_unavailable"},
                "successor_decision_id": null,
                "marker_decision_id": null,
                "supplies_tool_call": false,
                "decision": null,
                "tool_call_batch": {"kind": "invalid", "refusal": "tool_registration_unavailable"},
                "roles": "decision,retrieval,successor,late_retrieval"
            }),
            Vec::new(),
            Some(true),
        )
    );
}
