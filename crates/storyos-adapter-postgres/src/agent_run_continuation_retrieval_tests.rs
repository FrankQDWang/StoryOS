//! The retrieval and the one bounded successor of an unknown continuation Attempt at Decision
//! position 1 (ADR 0088).

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use storyos_adapter_volcengine_responses::AgentPlanExchange;
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, AgentRunRecord,
    CancelAgentRunInput, CompleteAgentRunError, DestinationRequest, ModelProviderAdapter,
    ModelStreamSink, NoContractFaults, Observation, OriginalResultRetrievalDisposition,
    PreDispatchRefusal, PreparedRequest, ProjectScope, complete_agent_run_with_tools,
};
use storyos_core::{ModelAdapter, ToolEnablement};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{RuntimeClock, claim_run, stores};
use crate::agent_run_retrieval::tests::{adapter, broken_stream, destination, not_found, real_run};
use crate::agent_run_tool_call::tests::set_research_tool;
use crate::agent_run_tool_dispatch::tests::{client, exa_tools, mcp_replies, point_mcp, reply};
use crate::command_sequence::tests::{CANCEL_AGENT_RUN, issued};
use crate::research_artifact::tests::{Reply, computed, message_stream, tagged};
use crate::research_source::tests::{SEARCH_TEXT, calls_stream};

const SEARCH: &str = r#"{"query":"tide tables of 1890","result_count":null}"#;

/// A change that the test makes before one exchange of the Run.
enum Fault {
    /// An admin statement.
    Sql(String),
    /// The author disables the research Tool with a fresh challenge of this suffix.
    Disable(u16),
    /// The author cancels the Run with a fresh challenge of this suffix.
    Cancel(u16),
}

/// The real adapter. Before exchange number `fault.0`, the test makes the change `fault.1`. After
/// exchange number `crash_after`, the fence moves, as after a crash of the Worker. Before each
/// Retrieve exchange, it reads `getAgentRun` of the Run. Before each exchange, it reads the
/// successor marker, the Model Attempts, and the Research Artifacts of the Run, and it keeps the
/// Wire Payload Projection of each Create.
struct Hooks<'a> {
    store: &'a PostgresProjectReader,
    admin: &'a Client,
    scope: ProjectScope,
    run_id: String,
    crash_after: Option<usize>,
    fault: Option<(usize, Fault)>,
    exchanges: Mutex<usize>,
    retrieve: Mutex<bool>,
    during: Mutex<Option<AgentRunRecord>>,
    before: Mutex<Vec<Value>>,
    payloads: Mutex<Vec<Value>>,
}

impl ModelProviderAdapter for Hooks<'_> {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = AgentPlanExchange;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<AgentPlanExchange>, PreDispatchRefusal> {
        *self.retrieve.lock().unwrap() = matches!(request, DestinationRequest::Retrieve(_));
        let prepared = adapter().prepare(request).await;
        if let (DestinationRequest::Create(_), Ok(found)) = (request, &prepared) {
            let payload = found.projection.serialized_payload.as_deref();
            self.payloads
                .lock()
                .unwrap()
                .push(serde_json::from_str(payload.unwrap_or("null")).unwrap());
        }
        prepared
    }

    async fn exchange(
        &self,
        prepared: AgentPlanExchange,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        let count = {
            let mut exchanges = self.exchanges.lock().unwrap();
            *exchanges += 1;
            *exchanges
        };
        match &self.fault {
            Some((at, Fault::Sql(statement))) if *at == count => {
                self.admin.execute(statement.as_str(), &[]).await.unwrap();
            }
            Some((at, Fault::Disable(suffix))) if *at == count => {
                set_research_tool(self.store, &self.scope, *suffix, ToolEnablement::Disabled).await;
            }
            Some((at, Fault::Cancel(suffix))) if *at == count => {
                let input = CancelAgentRunInput {
                    run_id: self.run_id.clone(),
                };
                let call = issued(self.store, &self.scope, *suffix, &CANCEL_AGENT_RUN, input).await;
                self.store
                    .cancel_agent_run(&call.envelope, &call.input)
                    .await
                    .unwrap();
            }
            _ => {}
        }
        if *self.retrieve.lock().unwrap() {
            let record = read(self.store, &self.scope, &self.run_id).await;
            *self.during.lock().unwrap() = Some(record);
        }
        let before = json!({"marker": marker(self.admin, &self.run_id).await,
            "attempts": attempts(self.admin, &self.run_id).await,
            "artifacts": artifacts(self.admin, &self.run_id).await});
        self.before.lock().unwrap().push(before);
        let observation = adapter().exchange(prepared, sink).await;
        if self.crash_after == Some(count) {
            self.admin
                .execute(
                    "UPDATE storyos.agent_runs SET fence_token = fence_token + 1
                      WHERE run_id = $1::text::uuid",
                    &[&self.run_id],
                )
                .await
                .unwrap();
        }
        observation
    }
}

async fn read(store: &PostgresProjectReader, scope: &ProjectScope, run_id: &str) -> AgentRunRecord {
    store
        .read_agent_run(scope, run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap()
}

/// The `getAgentRun` facts of the recovery: the status, the Decision kind, the dispatch state of
/// the shown Attempt, and the disposition, keep reason, and reservation of the retrieval.
fn inspect(record: &AgentRunRecord) -> Value {
    let retrieval = record.original_result_retrieval.as_ref();
    json!({
        "status": format!("{:?}", record.status),
        "decision": match &record.decision {
            AgentRunDecisionInspect::Absent => "absent",
            AgentRunDecisionInspect::ResearchSynthesis { .. } => "research_synthesis",
            AgentRunDecisionInspect::ToolRequest { .. } => "tool_request",
            _ => "other",
        },
        "dispatch_state": record.model.as_ref().map(|model| model.dispatch_state.clone()),
        "disposition": retrieval.map(|found| match found.disposition {
            OriginalResultRetrievalDisposition::KeptUnknown => "kept_unknown",
            OriginalResultRetrievalDisposition::EvidenceOnly => "evidence_only",
            OriginalResultRetrievalDisposition::Settled => "settled",
        }),
        "keep_reason": retrieval.and_then(|found| found.keep_reason.clone()),
        "reservation_released": retrieval.map(|found| found.reservation_released),
        "usage_kind": retrieval.map(|found| found.usage_kind.clone()),
    })
}

/// Each Model Attempt of the Run in position and role order: its role, position, dispatch state,
/// usage, reservation, retrieval result, and whether it has its own Destination Attempt, Outbound
/// Disclosure Event, and Wire Payload Projection.
async fn attempts(admin: &Client, run_id: &str) -> Vec<Value> {
    admin
        .query(
            "SELECT jsonb_build_object(
                      'role', attempt_role, 'position', decision_position::text,
                      'dispatch_state', dispatch_state, 'usage', payload->'usage',
                      'reservation', payload->'reservation', 'result', payload->>'result',
                      'decided', decision_id IS NOT NULL,
                      'own_evidence', count(*) OVER (PARTITION BY destination_attempt_id) = 1
                        AND count(*) OVER (PARTITION BY outbound_disclosure_event_id) = 1
                        AND count(*) OVER (PARTITION BY wire_payload_projection_id) = 1)::text
               FROM storyos.model_attempts WHERE run_id = $1::text::uuid
              ORDER BY decision_position, attempt_role",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
        .collect()
}

/// The copied Context Assembly rows of the Run: the role, the manifest or the Operation
/// Requirement, and the position of each.
async fn retrieval_context(admin: &Client, run_id: &str) -> Vec<String> {
    admin
        .query(
            "SELECT manifest_role || ' manifest:' || decision_position::text
               FROM storyos.context_assembly_manifests
              WHERE run_id = $1::text::uuid AND manifest_role IN ('retrieval', 'successor')
             UNION ALL
             SELECT requirement_role || ' requirement:' || decision_position::text
               FROM storyos.operation_requirements
              WHERE run_id = $1::text::uuid AND requirement_role IN ('retrieval', 'successor')
             ORDER BY 1",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(/*idx*/ 0))
        .collect()
}

/// The successor marker of the Attempt at position 1: its disposition, pause reason, allowance,
/// fence, and late evidence.
async fn marker(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT COALESCE((
               SELECT jsonb_build_object(
                        'disposition', marker->'disposition',
                        'pause_reason', marker->'pause_reason',
                        'allowance_consumed', marker->'allowance_consumed',
                        'predecessor_fenced', marker->'predecessor_fenced',
                        'dispatch_prohibited', marker->'dispatch_prohibited',
                        'late_evidence_applied', marker->'late_evidence_applied')
                 FROM (SELECT payload->'unknown_create_successor' AS marker
                         FROM storyos.model_attempts WHERE run_id = $1::text::uuid
                          AND attempt_role = 'decision' AND decision_position = 1) AS found
                WHERE marker IS NOT NULL), 'null')::text",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get(/*idx*/ 0)).unwrap()
}

/// The Research Artifacts of the Run: the role and position of the producing Attempt, whether
/// the Artifact names the Decision of that Attempt, and the summary.
async fn artifacts(admin: &Client, run_id: &str) -> Vec<Value> {
    admin
        .query(
            "SELECT jsonb_build_object(
                      'role', attempt.attempt_role, 'position', artifact.decision_position::text,
                      'decision', artifact.decision_id = attempt.decision_id,
                      'summary', artifact.content->>'summary')::text
               FROM storyos.research_artifacts AS artifact
               JOIN storyos.model_attempts AS attempt
                 ON attempt.model_attempt_id = artifact.model_attempt_id
              WHERE artifact.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
        .collect()
}

/// A Retrieve reply with a stored response of one message of `text` in `status`.
fn stored_message(reference: &str, text: &str, status: &str) -> String {
    let message = json!({"id": "msg_01", "type": "message", "role": "assistant",
        "status": status, "content": [{"type": "output_text", "text": text}]});
    let body = json!({"id": reference, "status": status, "output": [message],
        "usage": {"input_tokens": 40, "output_tokens": 12}})
    .to_string();
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\
         connection: close\r\n\r\n{body}",
        body.len()
    )
}

/// One later reply of the model destination, from the continuation reference and the Snapshot
/// identities that the continuation request tagged.
type Later = fn(&str, &[String]) -> String;

/// A research synthesis with `summary` whose one claim cites the first tagged Snapshot.
fn synthesis_text(summary: &str, sources: &[String]) -> String {
    json!({"kind": "research_synthesis", "summary": summary,
        "claims": [{"text": "High water came at 10.42.", "citations": [sources[0]]}]})
    .to_string()
}

/// Admits one research Run. Its first response requests one search, the fake MCP server answers
/// it, and the continuation stream of position 1 reports `resp_<prefix>_2` and then breaks. Each
/// `later` reply answers the next request in order.
async fn continuation_run(
    store: &PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    suffix: u16,
    later: &[Later],
) -> (ProjectScope, String, tokio::task::JoinHandle<Vec<String>>) {
    let (scope, run_id) = research_run(store, admin, prefix, "http://127.0.0.1:9/api/plan/v3").await;
    let continuation = format!("resp_{prefix}_2");
    let sources = Arc::new(Mutex::new(Vec::new()));
    let tagged_sources = Arc::clone(&sources);
    let first = calls_stream(
        &format!("resp_{prefix}"),
        &[("call_01", "web_search", SEARCH)],
    );
    let broken = broken_stream(&continuation);
    let mut replies: Vec<Reply> = vec![
        Box::new(move |_| first),
        Box::new(move |body| {
            *tagged_sources.lock().unwrap() = tagged(body, "<source id=\"");
            broken
        }),
    ];
    for reply in later.iter().copied() {
        let (continuation, sources) = (continuation.clone(), Arc::clone(&sources));
        replies.push(Box::new(move |_| {
            reply(&continuation, &sources.lock().unwrap())
        }));
    }
    let (model_base, served) = computed(replies).await;
    admin
        .execute(
            "UPDATE storyos.processing_destination_identity_evidence_revisions
                SET endpoint = $2
              WHERE project_id = $1::text::uuid AND evidence_kind = 'volcengine_agent_plan_boundary'",
            &[&scope.project_id.as_ref(), &model_base],
        )
        .await
        .unwrap();
    let result = reply(
        /*id*/ 0,
        json!({"content": [{"type": "text", "text": SEARCH_TEXT}], "isError": false}),
    );
    // The fake MCP server answers up to two searches, each in its own session.
    let mcp = [result.clone(), result]
        .into_iter()
        .flat_map(|call| mcp_replies(exa_tools(|_| {}), Some(call)))
        .collect();
    let (mcp_base, _mcp) = destination(mcp).await;
    point_mcp(admin, &scope, &mcp_base).await;
    (scope, run_id, served)
}

/// Claims the Run and settles it with `model`, as one Worker does. A claim keeps a cancelled Run
/// cancelled, as the claim of the Worker does.
async fn drain(
    store: &PostgresProjectReader,
    admin: &Client,
    model: &Hooks<'_>,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<(), CompleteAgentRunError> {
    let cancelled: bool = admin
        .query_one(
            "SELECT status = 'cancelled' FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let claim = claim_run(admin, scope.clone(), run_id.to_owned()).await;
    if cancelled {
        admin
            .execute(
                "UPDATE storyos.agent_runs SET status = 'cancelled' WHERE run_id = $1::text::uuid",
                &[&run_id],
            )
            .await
            .unwrap();
    }
    complete_agent_run_with_tools(
        store,
        model,
        &client(),
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .map(|_| ())
}

fn hooks<'a>(
    store: &'a PostgresProjectReader,
    admin: &'a Client,
    scope: &ProjectScope,
    run_id: &str,
    crash_after: Option<usize>,
    fault: Option<(usize, Fault)>,
) -> Hooks<'a> {
    Hooks {
        store,
        admin,
        scope: scope.clone(),
        run_id: run_id.to_owned(),
        crash_after,
        fault,
        exchanges: Mutex::new(/*value*/ 0),
        retrieve: Mutex::new(/*value*/ false),
        during: Mutex::new(/*value*/ None),
        before: Mutex::new(Vec::new()),
        payloads: Mutex::new(Vec::new()),
    }
}

fn post() -> String {
    "POST /api/plan/v3/responses HTTP/1.1".to_owned()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_restart_retrieves_the_stored_synthesis_of_a_continuation_once_and_records_the_artifact()
{
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let synthesis: Later = |reference, sources| {
        let text = synthesis_text("The tides of 1890.", sources);
        stored_message(reference, &text, "completed")
    };
    let (scope, run_id, served) =
        continuation_run(&store, &admin, "d81", 0xd81a, &[synthesis]).await;
    // The Worker crashes after the continuation stream broke, before its record.
    let crashed = drain(
        &store,
        &admin,
        &hooks(
            &store,
            &admin,
            &scope,
            &run_id,
            /*crash_after*/ Some(2),
            /*fault*/ None,
        ),
        &scope,
        &run_id,
    )
    .await;
    let restart = hooks(
        &store, &admin, &scope, &run_id, /*crash_after*/ None, /*fault*/ None,
    );
    drain(&store, &admin, &restart, &scope, &run_id)
        .await
        .unwrap();
    let after = read(&store, &scope, &run_id).await;
    // A second drain finds the Run settled and sends nothing.
    let again = hooks(
        &store, &admin, &scope, &run_id, /*crash_after*/ None, /*fault*/ None,
    );
    drain(&store, &admin, &again, &scope, &run_id)
        .await
        .unwrap();
    let lineage = admin
        .query_one(
            "SELECT jsonb_build_object(
                      'position', artifact.decision_position::text,
                      'attempt', artifact.model_attempt_id = attempt.model_attempt_id,
                      'decision', artifact.decision_id = attempt.decision_id,
                      'tool_calls', artifact.tool_call_ids = ARRAY[call.tool_call_id],
                      'claims', artifact.content->'claims'->0->>'text')::text
               FROM storyos.research_artifacts AS artifact
               JOIN storyos.model_attempts AS attempt
                 ON attempt.run_id = artifact.run_id AND attempt.attempt_role = 'decision'
                AND attempt.decision_position = 1
               JOIN storyos.tool_calls AS call ON call.run_id = artifact.run_id
              WHERE artifact.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .map(|row| serde_json::from_str::<Value>(row.get(/*idx*/ 0)).unwrap())
        .unwrap();
    let reserved = json!({"kind": "worst_case", "released": false});
    let released = json!({"kind": "worst_case", "released": true});
    let during = restart.during.lock().unwrap().as_ref().map(inspect);
    let retrieved_again = again.during.lock().unwrap().is_some();

    assert_eq!(
        (
            matches!(crashed, Err(CompleteAgentRunError::StaleFence)),
            during,
            inspect(&after),
            read(&store, &scope, &run_id).await == after,
            retrieved_again,
            attempts(&admin, &run_id).await,
            retrieval_context(&admin, &run_id).await,
            lineage,
            served.await.unwrap(),
        ),
        (
            true,
            Some(json!({"status": "Claimed", "decision": "absent",
                        "dispatch_state": "uncertain", "disposition": null,
                        "keep_reason": null, "reservation_released": null, "usage_kind": null})),
            json!({"status": "Completed", "decision": "research_synthesis",
                   "dispatch_state": "settled", "disposition": "settled", "keep_reason": null,
                   "reservation_released": true, "usage_kind": "reported"}),
            true,
            false,
            vec![
                json!({"role": "decision", "position": "0", "dispatch_state": "settled",
                       "usage": {"kind": "reported", "input_tokens": 90, "output_tokens": 30},
                       "reservation": null, "result": null, "decided": true,
                       "own_evidence": true}),
                json!({"role": "decision", "position": "1", "dispatch_state": "settled",
                       "usage": {"kind": "reported", "input_tokens": 40, "output_tokens": 12},
                       "reservation": released, "result": null, "decided": true,
                       "own_evidence": true}),
                // The Retrieve keeps no usage of its own, so the Run counts the response once.
                json!({"role": "retrieval", "position": "1", "dispatch_state": "settled",
                       "usage": {"kind": "unknown"}, "reservation": reserved,
                       "result": "complete_selected", "decided": false, "own_evidence": true}),
            ],
            vec![
                "retrieval manifest:1".to_owned(),
                "retrieval requirement:1".to_owned(),
            ],
            json!({"position": "1", "attempt": true, "decision": true, "tool_calls": true,
                   "claims": "High water came at 10.42."}),
            vec![
                post(),
                post(),
                "GET /api/plan/v3/responses/resp_d81_2 HTTP/1.1".to_owned(),
            ],
        )
    );
}

/// The request of a Wire Payload Projection: the prior response, and the type and call of each
/// input item.
fn request_items(payload: &Value) -> Value {
    json!({
        "previous_response_id": payload["previous_response_id"],
        "input": payload["input"].as_array().into_iter().flatten()
            .map(|item| json!([item["type"], item["call_id"]]))
            .collect::<Vec<_>>(),
    })
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_not_found_retrieve_of_a_continuation_sends_one_successor_that_settles_the_synthesis() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let successor: Later = |reference, sources| {
        let text = synthesis_text("The tides of 1890.", sources);
        message_stream(&format!("{reference}_s"), &text, "completed")
    };
    // The predecessor finished after its stream broke, and the late Retrieve reads it.
    let late: Later = |reference, sources| {
        stored_message(
            reference,
            &synthesis_text("A late answer.", sources),
            "completed",
        )
    };
    let (scope, run_id, served) = continuation_run(
        &store,
        &admin,
        "a91",
        0xa91a,
        &[|_, _| not_found(), successor, late],
    )
    .await;
    let model = hooks(
        &store, &admin, &scope, &run_id, /*crash_after*/ None, /*fault*/ None,
    );
    drain(&store, &admin, &model, &scope, &run_id)
        .await
        .unwrap();
    let after = read(&store, &scope, &run_id).await;
    // A restart finds the Run settled and sends no second successor.
    let restart = hooks(
        &store, &admin, &scope, &run_id, /*crash_after*/ None, /*fault*/ None,
    );
    drain(&store, &admin, &restart, &scope, &run_id)
        .await
        .unwrap();
    let digests: Value = admin
        .query_one(
            "SELECT jsonb_build_array(decision.payload #> '{wire,digest}',
                                      successor.payload #> '{projection,digest}')::text
               FROM storyos.model_attempts AS decision
               JOIN storyos.model_attempts AS successor
                 ON successor.run_id = decision.run_id AND successor.attempt_role = 'successor'
              WHERE decision.run_id = $1::text::uuid AND decision.attempt_role = 'decision'
                AND decision.decision_position = 1",
            &[&run_id],
        )
        .await
        .map(|row| serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
        .unwrap();
    let payloads = model.payloads.into_inner().unwrap();
    let before = model.before.into_inner().unwrap();
    let reserved = json!({"kind": "worst_case", "released": false});
    let released = json!({"kind": "worst_case", "released": true});
    let reported = json!({"kind": "reported", "input_tokens": 40, "output_tokens": 12});
    let attempt = |role: &str, state: &str, usage: &Value, reservation: &Value, result: Value| {
        json!({"role": role, "position": "1", "dispatch_state": state, "usage": usage,
               "reservation": reservation, "result": result,
               "decided": role == "successor", "own_evidence": true})
    };
    let unknown = json!({"kind": "unknown"});
    let first = json!({"role": "decision", "position": "0", "dispatch_state": "settled",
        "usage": {"kind": "reported", "input_tokens": 90, "output_tokens": 30},
        "reservation": null, "result": null, "decided": true, "own_evidence": true});
    let artifact = vec![
        json!({"role": "successor", "position": "1", "decision": true,
                               "summary": "The tides of 1890."}),
    ];

    assert_eq!(
        (
            inspect(&after),
            read(&store, &scope, &run_id).await == after,
            restart.before.into_inner().unwrap(),
            // The successor sends the stored continuation input again (ADR 0085).
            payloads[1] == payloads[2],
            request_items(&payloads[2]),
            digests[0] == digests[1],
            // The fence is persisted before the successor starts.
            before[3]["marker"].clone(),
            // During the late Retrieve, the predecessor keeps its unknown usage and its
            // worst-case reservation, and the successor has its own.
            before[4]["attempts"].clone(),
            before[4]["artifacts"].clone(),
            (
                marker(&admin, &run_id).await,
                attempts(&admin, &run_id).await,
                artifacts(&admin, &run_id).await,
                retrieval_context(&admin, &run_id).await,
                served.await.unwrap(),
            ),
        ),
        (
            // The Decision is the one of the successor; the retrieval kept the unknown outcome.
            json!({"status": "Completed", "decision": "research_synthesis",
                   "dispatch_state": "uncertain", "disposition": "kept_unknown",
                   "keep_reason": "unknown_result", "reservation_released": false,
                   "usage_kind": "unknown"}),
            true,
            Vec::<Value>::new(),
            true,
            // The function call item is in the stored response that the request continues.
            json!({"previous_response_id": "resp_a91",
                   "input": [["function_call_output", "call_01"]]}),
            true,
            json!({"disposition": "fenced", "pause_reason": null, "allowance_consumed": true,
                   "predecessor_fenced": true, "dispatch_prohibited": false,
                   "late_evidence_applied": false}),
            json!([
                first,
                attempt("decision", "uncertain", &unknown, &reserved, Value::Null),
                attempt(
                    "late_retrieval",
                    "uncertain",
                    &unknown,
                    &reserved,
                    Value::Null
                ),
                attempt(
                    "retrieval",
                    "uncertain",
                    &unknown,
                    &reserved,
                    json!("unknown")
                ),
                attempt("successor", "settled", &reported, &released, Value::Null),
            ]),
            json!(artifact),
            // The late result is evidence only: it settles the usage and the reservation of the
            // predecessor, and it changes neither the Run, the Decision, nor the Artifact.
            (
                json!({"disposition": "dispatched", "pause_reason": null, "allowance_consumed": true,
                   "predecessor_fenced": true, "dispatch_prohibited": false,
                   "late_evidence_applied": true}),
                vec![
                    first.clone(),
                    attempt("decision", "uncertain", &reported, &released, Value::Null),
                    attempt(
                        "late_retrieval",
                        "settled",
                        &unknown,
                        &reserved,
                        json!("complete_selected")
                    ),
                    attempt(
                        "retrieval",
                        "uncertain",
                        &unknown,
                        &reserved,
                        json!("unknown")
                    ),
                    attempt("successor", "settled", &reported, &released, Value::Null),
                ],
                artifact,
                // The successor copies the Context Assembly of position 1, as the Retrieve does.
                [
                    "retrieval manifest:1",
                    "retrieval requirement:1",
                    "successor manifest:1",
                    "successor requirement:1",
                ]
                .map(str::to_owned)
                .to_vec(),
                vec![
                    post(),
                    post(),
                    "GET /api/plan/v3/responses/resp_a91_2 HTTP/1.1".to_owned(),
                    post(),
                    "GET /api/plan/v3/responses/resp_a91_2 HTTP/1.1".to_owned(),
                ]
            ),
        )
    );
}

/// The status, the settlement, the successor marker, and the Attempt roles of a Run.
async fn outcome(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_build_object('status', run.status, 'settlement', run.settlement,
                      'roles', (SELECT string_agg(attempt_role, ',' ORDER BY model_attempt_id)
                                  FROM storyos.model_attempts WHERE run_id = run.run_id))::text
               FROM storyos.agent_runs AS run WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    let mut outcome: Value = serde_json::from_str(row.get(/*idx*/ 0)).unwrap();
    outcome["marker"] = marker(admin, run_id).await;
    outcome
}

/// Settles one continuation Run whose Retrieve answers `404`, with the change `fault` of the Run
/// before exchange `at`, and then drains it again as after a restart. Returns whether the first
/// Worker lost its claim, the outcome, the number of exchanges of the restart, and the request
/// lines.
async fn failed_condition(
    store: &PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    suffix: u16,
    at: usize,
    fault: impl FnOnce(&str) -> Fault,
) -> (bool, Value, usize, Vec<String>) {
    let (scope, run_id, served) =
        continuation_run(store, admin, prefix, suffix, &[|_, _| not_found()]).await;
    let fault = Some((at, fault(&run_id)));
    let model = hooks(
        store, admin, &scope, &run_id, /*crash_after*/ None, fault,
    );
    // A cancellation moves the fence, so the Worker of the cancelled Run loses its claim.
    let first = drain(store, admin, &model, &scope, &run_id).await;
    let restart = hooks(
        store, admin, &scope, &run_id, /*crash_after*/ None, /*fault*/ None,
    );
    drain(store, admin, &restart, &scope, &run_id)
        .await
        .unwrap();
    (
        matches!(first, Err(CompleteAgentRunError::StaleFence)),
        outcome(admin, &run_id).await,
        restart.before.into_inner().unwrap().len(),
        served.await.unwrap(),
    )
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_failed_condition_or_a_cancellation_stops_the_continuation_with_no_successor() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // The Host recorded that the budget does not cover both Attempts. A real route admits a Run
    // only with a known request bound, and the Retrieve needs it too, so the test writes the fact.
    let budget = failed_condition(&store, &admin, "a92", 0xa92a, 3, |run_id| {
        Fault::Sql(format!(
            "UPDATE storyos.model_attempts
                SET payload = jsonb_set(payload,
                      '{{unknown_create_successor,budget_covers_both}}', 'false')
              WHERE run_id = '{run_id}' AND attempt_role = 'decision'
                AND decision_position = 1"
        ))
    })
    .await;
    // The author disables the research Tool during the Retrieve: the Tool Exposure changes.
    let exposure =
        failed_condition(&store, &admin, "a93", 0xa93a, 3, |_| Fault::Disable(0xa93b)).await;
    // The author cancels the Run during the Retrieve.
    let cancelled =
        failed_condition(&store, &admin, "a94", 0xa94a, 3, |_| Fault::Cancel(0xa94b)).await;

    let marker = |disposition: &str, reason: Option<&str>, fenced: bool| {
        json!({"disposition": disposition, "pause_reason": reason, "allowance_consumed": fenced,
               "predecessor_fenced": fenced, "dispatch_prohibited": reason.is_none(),
               "late_evidence_applied": false})
    };
    let paused = |reason: &str, fenced: bool| {
        json!({"status": "paused", "roles": "decision,decision,retrieval",
               "settlement": {"kind": "unknown_create_successor", "reason": reason},
               "marker": marker("paused", Some(reason), fenced)})
    };
    let lines = |prefix: &str| {
        vec![
            post(),
            post(),
            format!("GET /api/plan/v3/responses/resp_{prefix}_2 HTTP/1.1"),
        ]
    };

    assert_eq!(
        (budget, exposure, cancelled),
        (
            (false, paused("budget_insufficient", false), 0, lines("a92")),
            // The fence came first, so the allowance stays spent after the pause and a restart.
            (false, paused("request_changed", true), 0, lines("a93")),
            // The restart re-observes the claimed Retrieve and sends nothing (ADR 0057).
            (
                true,
                json!({"status": "cancelled", "roles": "decision,decision,retrieval",
                       "settlement": null, "marker": marker("prohibited", None, false)}),
                1,
                lines("a94"),
            ),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_interrupted_successor_of_a_continuation_waits_with_an_unknown_outcome() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let broken: Later = |reference, _| broken_stream(&format!("{reference}_s"));
    let (scope, run_id, served) = continuation_run(
        &store,
        &admin,
        "a95",
        0xa95a,
        &[|_, _| not_found(), broken, |_, _| not_found()],
    )
    .await;
    // The Worker crashes after the successor stream broke, before its record.
    let crashed = drain(
        &store,
        &admin,
        &hooks(
            &store,
            &admin,
            &scope,
            &run_id,
            /*crash_after*/ Some(4),
            /*fault*/ None,
        ),
        &scope,
        &run_id,
    )
    .await;
    let restart = hooks(
        &store, &admin, &scope, &run_id, /*crash_after*/ None, /*fault*/ None,
    );
    drain(&store, &admin, &restart, &scope, &run_id)
        .await
        .unwrap();
    let again = hooks(
        &store, &admin, &scope, &run_id, /*crash_after*/ None, /*fault*/ None,
    );
    drain(&store, &admin, &again, &scope, &run_id)
        .await
        .unwrap();

    assert_eq!(
        (
            matches!(crashed, Err(CompleteAgentRunError::StaleFence)),
            outcome(&admin, &run_id).await,
            attempts(&admin, &run_id).await.pop(),
            restart.before.into_inner().unwrap().len(),
            again.before.into_inner().unwrap().len(),
            served.await.unwrap(),
        ),
        (
            true,
            // The allowance is spent, so the Run stops with the unknown outcome (ADR 0088).
            json!({"status": "waiting",
                   "roles": "decision,decision,retrieval,successor,late_retrieval",
                   "settlement": {"kind": "outcome_unknown", "reason": "unknown_result"},
                   "marker": {"disposition": "dispatched", "pause_reason": null,
                              "allowance_consumed": true, "predecessor_fenced": true,
                              "dispatch_prohibited": false, "late_evidence_applied": false}}),
            // The successor keeps its unknown usage and its worst-case reservation.
            Some(
                json!({"role": "successor", "position": "1", "dispatch_state": "uncertain",
                        "usage": {"kind": "unknown"},
                        "reservation": {"kind": "worst_case", "released": false},
                        "result": null, "decided": false, "own_evidence": true})
            ),
            // The restart re-observes the claimed successor with no egress, and then reads the
            // late predecessor result once.
            2,
            0,
            vec![
                post(),
                post(),
                "GET /api/plan/v3/responses/resp_a95_2 HTTP/1.1".to_owned(),
                post(),
                "GET /api/plan/v3/responses/resp_a95_2 HTTP/1.1".to_owned(),
            ],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_successor_function_request_continues_to_a_synthesis_at_the_next_position() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let search: Later = |reference, _| {
        calls_stream(
            &format!("{reference}_s"),
            &[("call_02", "web_search", SEARCH)],
        )
    };
    let synthesis: Later = |reference, sources| {
        let text = synthesis_text("The tides of 1890.", sources);
        message_stream(&format!("{reference}_3"), &text, "completed")
    };
    let (scope, run_id, served) = continuation_run(
        &store,
        &admin,
        "a96",
        0xa96a,
        &[|_, _| not_found(), search, |_, _| not_found(), synthesis],
    )
    .await;
    let model = hooks(
        &store, &admin, &scope, &run_id, /*crash_after*/ None, /*fault*/ None,
    );
    drain(&store, &admin, &model, &scope, &run_id)
        .await
        .unwrap();
    let payloads = model.payloads.into_inner().unwrap();

    assert_eq!(
        (
            inspect(&read(&store, &scope, &run_id).await),
            outcome(&admin, &run_id).await,
            artifacts(&admin, &run_id).await,
            payloads.last().map(request_items),
            served.await.unwrap(),
        ),
        (
            json!({"status": "Completed", "decision": "research_synthesis",
                   "dispatch_state": "settled", "disposition": "kept_unknown",
                   "keep_reason": "unknown_result", "reservation_released": false,
                   "usage_kind": "unknown"}),
            json!({"status": "completed", "settlement": null,
                   "roles": "decision,decision,retrieval,successor,late_retrieval,decision",
                   "marker": {"disposition": "dispatched", "pause_reason": null,
                              "allowance_consumed": true, "predecessor_fenced": true,
                              "dispatch_prohibited": false, "late_evidence_applied": false}}),
            vec![
                json!({"role": "decision", "position": "2", "decision": true,
                        "summary": "The tides of 1890."})
            ],
            // The next RunStep continues the successor response with the result of its call.
            Some(json!({"previous_response_id": "resp_a96_2_s",
                        "input": [["function_call_output", "call_02"]]})),
            vec![
                post(),
                post(),
                "GET /api/plan/v3/responses/resp_a96_2 HTTP/1.1".to_owned(),
                post(),
                "GET /api/plan/v3/responses/resp_a96_2 HTTP/1.1".to_owned(),
                post(),
            ],
        )
    );
}
