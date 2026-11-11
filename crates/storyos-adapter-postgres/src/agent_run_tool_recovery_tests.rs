use std::sync::Mutex;

use serde_json::{Value, json};
use storyos_adapter_mcp_client::McpExchange;
use storyos_application::{
    CancelAgentRunInput, CompleteAgentRunError, ContractFaultObserver, ContractFaultPoint,
    ConversationSelection, NoContractFaults, PreDispatchRefusal, PreparedRequest, ProjectScope,
    ResearchSourceReadStore, ToolCallRequest, ToolDestinationAdapter,
    complete_agent_run_with_tools,
};
use storyos_core::{DeploymentDestination, ToolEnablement, ToolObservation};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{RuntimeClock, admit_run_in, claim_run, stores};
use crate::agent_run_lease::tests::SteppedClock;
use crate::agent_run_retrieval::tests::{adapter, broken_stream, destination, real_run};
use crate::agent_run_tool_call::tests::{set_research_tool, text_and_call_stream};
use crate::agent_run_tool_dispatch::tests::{
    POST, RESULT, SEARCH, attempt, client, exa_tools, interrupted_attempt, mcp_replies, point_mcp,
    reply, settled_event, tool_facts, tool_run,
};
use crate::agent_run_tool_exposure::tests::advisory_stream;
use crate::command_sequence::tests::{CANCEL_AGENT_RUN, CommandCall, issued};
use crate::research_source::tests::calls_stream;

/// The result of the one search, as the fake MCP server sends it.
fn search_result() -> String {
    reply(
        /*id*/ 0,
        json!({"content": [{"type": "text", "text": RESULT}], "isError": false}),
    )
}

/// The cancelAgentRun of the author, with a fresh challenge.
async fn cancel_call(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    run_id: &str,
) -> CommandCall<CancelAgentRunInput> {
    issued(
        store,
        scope,
        suffix,
        &CANCEL_AGENT_RUN,
        CancelAgentRunInput {
            run_id: run_id.to_owned(),
        },
    )
    .await
}

enum Fault<'a> {
    /// The author cancels the Run.
    Cancel {
        store: &'a PostgresProjectReader,
        call: Box<CommandCall<CancelAgentRunInput>>,
    },
    /// Another Worker takes the Run over, as after a crash of this Worker.
    TakeOver { admin: &'a Client, run_id: String },
}

/// The Contract Fault Point observer: it injects `fault` when the Tool Gateway reaches `point`
/// for the `nth` time.
struct FaultAt<'a> {
    point: ContractFaultPoint,
    nth: usize,
    fault: Fault<'a>,
    reached: Mutex<usize>,
}

impl<'a> FaultAt<'a> {
    fn new(point: ContractFaultPoint, nth: usize, fault: Fault<'a>) -> Self {
        Self {
            point,
            nth,
            fault,
            reached: Mutex::new(/*value*/ 0),
        }
    }
}

impl ContractFaultObserver for FaultAt<'_> {
    async fn reached(&self, point: ContractFaultPoint) {
        let count = {
            let mut reached = self.reached.lock().unwrap();
            *reached += usize::from(point == self.point);
            *reached
        };
        if point != self.point || count != self.nth {
            return;
        }
        match &self.fault {
            Fault::Cancel { store, call } => {
                store
                    .cancel_agent_run(&call.envelope, &call.input)
                    .await
                    .unwrap();
            }
            Fault::TakeOver { admin, run_id } => {
                admin
                    .execute(
                        "UPDATE storyos.agent_runs SET fence_token = fence_token + 1
                          WHERE run_id = $1::text::uuid",
                        &[run_id],
                    )
                    .await
                    .unwrap();
            }
        }
    }
}

/// The real MCP client, and the cancellation of the Run by the author after the exchange
/// returned and before its record, while the Worker holds the old fence.
struct CancelAfterExchange<'a> {
    store: &'a PostgresProjectReader,
    cancel: CommandCall<CancelAgentRunInput>,
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

/// A fake MCP server that answers `replies` in order, then reads one more request, reports it,
/// and holds its answer `held` until the test releases it. A dropped release sends no answer.
async fn held_mcp(
    replies: Vec<String>,
    held: String,
) -> (
    String,
    oneshot::Receiver<()>,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<Vec<String>>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/api/plan/v3", listener.local_addr().unwrap());
    let (received, received_rx) = oneshot::channel();
    let (release, release_rx) = oneshot::channel::<()>();
    let served = tokio::spawn(async move {
        let mut lines = Vec::new();
        let mut received = Some(received);
        let mut release_rx = Some(release_rx);
        let count = replies.len();
        for (index, reply) in replies.into_iter().chain([held]).enumerate() {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 4096];
            while !request
                .windows(/*size*/ 4)
                .any(|window| window == b"\r\n\r\n")
            {
                let read = stream.read(&mut chunk).await.unwrap();
                request.extend_from_slice(&chunk[..read]);
            }
            let text = String::from_utf8_lossy(&request).into_owned();
            lines.push(text.lines().next().unwrap_or_default().to_owned());
            if index == count {
                let _reported = received.take().unwrap().send(());
                if release_rx.take().unwrap().await.is_err() {
                    break;
                }
            }
            let _answered = stream.write_all(reply.as_bytes()).await;
        }
        lines
    });
    (base, received_rx, release, served)
}

/// The Run status, the number of Model Attempts and Source Snapshots of the Run, and whether an
/// Attempt holds an automatic successor marker.
async fn run_counts(admin: &Client, run_id: &str) -> Value {
    admin
        .query_one(
            "SELECT jsonb_build_object(
                      'status', run.status, 'settlement', run.settlement,
                      'model_attempts', (SELECT count(*) FROM storyos.model_attempts AS attempt
                                          WHERE attempt.run_id = run.run_id),
                      'successor_marker', EXISTS (
                         SELECT 1 FROM storyos.model_attempts AS attempt
                          WHERE attempt.run_id = run.run_id
                            AND attempt.payload ? 'unknown_create_successor'),
                      'source_snapshots', (SELECT count(*) FROM storyos.source_snapshots AS source
                                            WHERE source.run_id = run.run_id))::text
               FROM storyos.agent_runs AS run WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get::<_, String>(/*idx*/ 0)
        .parse()
        .unwrap()
}

/// The listed research calls with an unknown outcome, with no identifier.
async fn unknown_calls(store: &PostgresProjectReader, scope: &ProjectScope, run_id: &str) -> Value {
    let sources = store
        .list_research_sources(scope, run_id)
        .await
        .unwrap()
        .unwrap();
    sources
        .unknown_calls
        .into_iter()
        .map(|call| json!({"call": call.call, "late_result": call.late_result}))
        .collect()
}

fn unknown_search() -> Value {
    json!({"kind": "search", "query": "tide tables of 1890", "outcome": "unknown"})
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_cancel_after_the_claim_sends_nothing_and_the_next_message_asks_with_a_new_tool_call() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (model_base, _model) = destination(vec![
        text_and_call_stream("resp_081", "call_01", "web_search", SEARCH, "completed"),
        text_and_call_stream("resp_081_2", "call_01", "web_search", SEARCH, "completed"),
        advisory_stream("resp_081_3"),
    ])
    .await;
    let (scope, run_id) = real_run(&store, &admin, "081", &model_base).await;
    set_research_tool(&store, &scope, 0x081a, ToolEnablement::Enabled).await;
    // The first Run sends nothing; only the new ToolCall of the next message reaches the server.
    let (mcp_base, mcp) = destination(mcp_replies(exa_tools(|_| {}), Some(search_result()))).await;
    point_mcp(&admin, &scope, &mcp_base).await;
    let observer = FaultAt::new(
        ContractFaultPoint::ToolDispatchClaimed,
        /*nth*/ 1,
        Fault::Cancel {
            store: &store,
            call: Box::new(cancel_call(&store, &scope, 0x081b, &run_id).await),
        },
    );
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let cancelled = complete_agent_run_with_tools(
        &store,
        &adapter(),
        &client(),
        &observer,
        &RuntimeClock,
        &claim,
    )
    .await;
    let before_next = tool_facts(&admin, &run_id).await;
    let run: (String, String) = admin
        .query_one(
            "SELECT chapter_id::text, conversation_id::text FROM storyos.agent_runs
              WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .map(|row| (row.get(/*idx*/ 0), row.get(/*idx*/ 1)))
        .unwrap();
    let next_run = admit_run_in(
        &store,
        &scope,
        &run.0,
        "082",
        DeploymentDestination::VolcengineAgentPlan {
            qualification_authorization: None,
        },
        "Check the tide tables again.",
        ConversationSelection::Existing {
            conversation_id: run.1,
        },
    )
    .await;
    let claim = claim_run(&admin, scope.clone(), next_run.clone()).await;
    complete_agent_run_with_tools(
        &store,
        &adapter(),
        &client(),
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();
    // Each ToolCall has its own identity, Context Assembly, and dispatch claim.
    let identities: Vec<i64> = admin
        .query_one(
            "SELECT count(DISTINCT tool_call_id), count(DISTINCT destination_attempt_id),
                    count(DISTINCT attempt #> '{context_assembly}')
               FROM storyos.tool_calls WHERE run_id IN ($1::text::uuid, $2::text::uuid)",
            &[&run_id, &next_run],
        )
        .await
        .map(|row| (0..3).map(|index| row.get(index)).collect())
        .unwrap();
    let outcome = json!({"external_read": "unknown", "outbound_disclosure": "unknown"});

    assert_eq!(
        (
            matches!(cancelled, Err(CompleteAgentRunError::StaleFence)),
            before_next["run"]["status"].clone(),
            before_next["tool_call"].clone(),
            before_next["events"].clone(),
            unknown_calls(&store, &scope, &run_id).await,
            tool_facts(&admin, &run_id).await == before_next,
            tool_facts(&admin, &next_run).await["tool_call"]["attempt"].clone(),
            identities,
            mcp.await.unwrap(),
        ),
        (
            true,
            json!("cancelled"),
            json!({"state": "unknown", "refusal": null,
                   "attempt": interrupted_attempt("run_cancelled"), "has_attempt_id": true,
                   "effect_outcome": outcome, "result_text": null, "result_digest": null,
                   "result_bytes": null}),
            json!([settled_event("unknown", Value::Null, outcome, Value::Null)]),
            json!([{"call": unknown_search(), "late_result": false}]),
            true,
            attempt("terminal_result", "confirmed", /*released*/ true),
            vec![2, 2, 2],
            vec![POST.to_owned(); 4],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_cancel_during_the_exchange_drops_it_locally_and_keeps_the_tool_call_unknown() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (mcp_base, received, release, mcp) =
        held_mcp(mcp_replies(exa_tools(|_| {}), None), search_result()).await;
    let (scope, run_id) = tool_run(&store, &admin, "083", 0x083a, &mcp_base).await;
    let cancel = cancel_call(&store, &scope, 0x083b, &run_id).await;
    let clock = SteppedClock::new();
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let (model, tools) = (adapter(), client());
    let gateway =
        complete_agent_run_with_tools(&store, &model, &tools, &NoContractFaults, &clock, &claim);
    let fault = async {
        received.await.unwrap();
        store
            .cancel_agent_run(&cancel.envelope, &cancel.input)
            .await
            .unwrap();
        // The next lease renewal reads the committed fence.
        clock.step();
    };
    let (dropped, ()) = tokio::join!(gateway, fault);
    drop(release);
    let outcome = json!({"external_read": "unknown", "outbound_disclosure": "unknown"});
    let facts = tool_facts(&admin, &run_id).await;

    assert_eq!(
        (
            matches!(dropped, Err(CompleteAgentRunError::StaleFence)),
            facts["run"]["status"].clone(),
            facts["tool_call"].clone(),
            facts["events"].clone(),
            unknown_calls(&store, &scope, &run_id).await,
            mcp.await.unwrap(),
        ),
        (
            true,
            json!("cancelled"),
            json!({"state": "unknown", "refusal": null,
                   "attempt": interrupted_attempt("run_cancelled"), "has_attempt_id": true,
                   "effect_outcome": outcome, "result_text": null, "result_digest": null,
                   "result_bytes": null}),
            json!([settled_event("unknown", Value::Null, outcome, Value::Null)]),
            json!([{"call": unknown_search(), "late_result": false}]),
            vec![POST.to_owned(); 4],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_result_after_the_cancellation_fence_is_kept_as_evidence_only() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (mcp_base, mcp) = destination(mcp_replies(exa_tools(|_| {}), Some(search_result()))).await;
    let (scope, run_id) = tool_run(&store, &admin, "084", 0x084a, &mcp_base).await;
    let tools = CancelAfterExchange {
        store: &store,
        cancel: cancel_call(&store, &scope, 0x084b, &run_id).await,
    };
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    // No renewal runs during the exchange, so the result arrives before the Worker reads the fence.
    let completed = complete_agent_run_with_tools(
        &store,
        &adapter(),
        &tools,
        &NoContractFaults,
        &SteppedClock::new(),
        &claim,
    )
    .await;
    let unknown = json!({"external_read": "unknown", "outbound_disclosure": "unknown"});
    let confirmed = json!({"external_read": "confirmed", "outbound_disclosure": "confirmed"});
    let mut attempt = interrupted_attempt("run_cancelled");
    attempt["late_observation"] = json!({
        "settlement": "terminal_result", "outcome": confirmed,
        "disclosure_confirmation": {"state": "confirmed"},
        "result_text": RESULT,
        "result_digest": format!("sha256:{}", storyos_core::hex_sha256(RESULT.as_bytes())),
        "result_bytes": RESULT.len(),
    });
    let facts = tool_facts(&admin, &run_id).await;

    assert_eq!(
        (
            matches!(completed, Err(CompleteAgentRunError::StaleFence)),
            facts["tool_call"].clone(),
            facts["events"].clone(),
            run_counts(&admin, &run_id).await,
            unknown_calls(&store, &scope, &run_id).await,
            mcp.await.unwrap(),
        ),
        (
            true,
            json!({"state": "unknown", "refusal": null, "attempt": attempt,
                   "has_attempt_id": true, "effect_outcome": confirmed, "result_text": null,
                   "result_digest": null, "result_bytes": null}),
            json!([
                settled_event("unknown", Value::Null, unknown, Value::Null),
                {"kind": "tool_call_late_result", "position": 1, "call_id": "call_01",
                 "class": "partial", "outcome": confirmed, "result_bytes": RESULT.len()},
            ]),
            json!({"status": "cancelled", "settlement": null, "model_attempts": 1,
                   "successor_marker": false, "source_snapshots": 0}),
            json!([{"call": unknown_search(), "late_result": true}]),
            vec![POST.to_owned(); 4],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_cancel_between_two_tool_calls_keeps_the_settled_one_and_refuses_the_pending_one() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let calls = [
        ("call_01", "web_search", SEARCH),
        (
            "call_02",
            "web_search",
            r#"{"query":"harbour log","result_count":null}"#,
        ),
    ];
    let (model_base, _model) = destination(vec![calls_stream("resp_085", &calls)]).await;
    let (scope, run_id) = real_run(&store, &admin, "085", &model_base).await;
    set_research_tool(&store, &scope, 0x085a, ToolEnablement::Enabled).await;
    let (mcp_base, mcp) = destination(mcp_replies(exa_tools(|_| {}), Some(search_result()))).await;
    point_mcp(&admin, &scope, &mcp_base).await;
    let observer = FaultAt::new(
        ContractFaultPoint::ToolContextCommitted,
        /*nth*/ 2,
        Fault::Cancel {
            store: &store,
            call: Box::new(cancel_call(&store, &scope, 0x085b, &run_id).await),
        },
    );
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let cancelled = complete_agent_run_with_tools(
        &store,
        &adapter(),
        &client(),
        &observer,
        &RuntimeClock,
        &claim,
    )
    .await;
    let rows: Vec<Value> = admin
        .query(
            "SELECT jsonb_build_object('call_id', call_id, 'state', state, 'refusal', refusal,
                      'reservation', attempt -> 'reservation', 'fenced', attempt ? 'fence')::text
               FROM storyos.tool_calls WHERE run_id = $1::text::uuid ORDER BY call_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
        .collect();

    assert_eq!(
        (
            matches!(cancelled, Err(CompleteAgentRunError::StaleFence)),
            rows,
            unknown_calls(&store, &scope, &run_id).await,
            mcp.await.unwrap(),
        ),
        (
            true,
            vec![
                json!({"call_id": "call_01", "state": "settled", "refusal": null,
                       "reservation": attempt("", "", /*released*/ true)["reservation"],
                       "fenced": false}),
                json!({"call_id": "call_02", "state": "refused", "refusal": "run_cancelled",
                       "reservation": null, "fenced": null}),
            ],
            json!([]),
            vec![POST.to_owned(); 4],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_takeover_after_the_claim_settles_unknown_at_the_next_claim_with_no_successor() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (model_base, _model) = destination(vec![
        text_and_call_stream("resp_086", "call_01", "web_search", SEARCH, "completed"),
        broken_stream("resp_086_2"),
    ])
    .await;
    let (scope, run_id) = real_run(&store, &admin, "086", &model_base).await;
    set_research_tool(&store, &scope, 0x086a, ToolEnablement::Enabled).await;
    let (mcp_base, mcp) = destination(Vec::new()).await;
    point_mcp(&admin, &scope, &mcp_base).await;
    let observer = FaultAt::new(
        ContractFaultPoint::ToolDispatchClaimed,
        /*nth*/ 1,
        Fault::TakeOver {
            admin: &admin,
            run_id: run_id.clone(),
        },
    );
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let lost = complete_agent_run_with_tools(
        &store,
        &adapter(),
        &client(),
        &observer,
        &RuntimeClock,
        &claim,
    )
    .await;
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    complete_agent_run_with_tools(
        &store,
        &adapter(),
        &client(),
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();
    let outcome = json!({"external_read": "unknown", "outbound_disclosure": "unknown"});

    assert_eq!(
        (
            matches!(lost, Err(CompleteAgentRunError::StaleFence)),
            tool_facts(&admin, &run_id).await["tool_call"].clone(),
            run_counts(&admin, &run_id).await,
            mcp.await.unwrap(),
        ),
        (
            true,
            json!({"state": "unknown", "refusal": null,
                   "attempt": interrupted_attempt("worker_restart"), "has_attempt_id": true,
                   "effect_outcome": outcome, "result_text": null, "result_digest": null,
                   "result_bytes": null}),
            json!({"status": "waiting",
                   "settlement": {"kind": "outcome_unknown", "reason": "unknown_result"},
                   "model_attempts": 2, "successor_marker": false, "source_snapshots": 0}),
            Vec::<String>::new(),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_answer_with_no_result_after_the_fence_keeps_the_tool_call_unknown() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let failed = "HTTP/1.1 502 Bad Gateway\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
    let (mcp_base, mcp) =
        destination(mcp_replies(exa_tools(|_| {}), Some(failed.to_owned()))).await;
    let (scope, run_id) = tool_run(&store, &admin, "087", 0x087a, &mcp_base).await;
    let tools = CancelAfterExchange {
        store: &store,
        cancel: cancel_call(&store, &scope, 0x087b, &run_id).await,
    };
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let completed = complete_agent_run_with_tools(
        &store,
        &adapter(),
        &tools,
        &NoContractFaults,
        &SteppedClock::new(),
        &claim,
    )
    .await;
    let disclosed = json!({"external_read": "unknown", "outbound_disclosure": "confirmed"});
    let facts = tool_facts(&admin, &run_id).await;

    assert_eq!(
        (
            matches!(completed, Err(CompleteAgentRunError::StaleFence)),
            facts["tool_call"]["state"].clone(),
            facts["tool_call"]["attempt"]["late_observation"]["settlement"].clone(),
            facts["tool_call"]["effect_outcome"].clone(),
            facts["events"][1]["class"].clone(),
            unknown_calls(&store, &scope, &run_id).await,
            mcp.await.unwrap(),
        ),
        (
            true,
            json!("unknown"),
            json!("outcome_unknown"),
            disclosed,
            json!("unknown"),
            json!([{"call": unknown_search(), "late_result": false}]),
            vec![POST.to_owned(); 4],
        )
    );
}
