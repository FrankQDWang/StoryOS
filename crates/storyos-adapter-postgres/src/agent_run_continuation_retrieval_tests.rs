//! The retrieval of an unknown continuation Attempt at Decision position 1 (ADR 0088).

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use storyos_adapter_volcengine_responses::AgentPlanExchange;
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, AgentRunRecord,
    CompleteAgentRunError, DestinationRequest, ModelProviderAdapter, ModelStreamSink,
    NoContractFaults, Observation, OriginalResultRetrievalDisposition, PreDispatchRefusal,
    PreparedRequest, ProjectScope, complete_agent_run_with_tools,
};
use storyos_core::{ModelAdapter, ToolEnablement};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{RuntimeClock, claim_run, stores};
use crate::agent_run_retrieval::tests::{adapter, broken_stream, destination, not_found, real_run};
use crate::agent_run_tool_call::tests::set_research_tool;
use crate::agent_run_tool_dispatch::tests::{client, exa_tools, mcp_replies, point_mcp, reply};
use crate::research_artifact::tests::{Reply, computed, tagged};
use crate::research_source::tests::{SEARCH_TEXT, calls_stream};

const SEARCH: &str = r#"{"query":"tide tables of 1890","result_count":null}"#;

/// The real adapter. After exchange number `crash_after`, the fence moves, as after a crash of the
/// Worker. Before each Retrieve exchange, it reads `getAgentRun` of the Run.
struct Hooks<'a> {
    store: &'a PostgresProjectReader,
    admin: &'a Client,
    scope: ProjectScope,
    run_id: String,
    crash_after: Option<usize>,
    exchanges: Mutex<usize>,
    retrieve: Mutex<bool>,
    during: Mutex<Option<AgentRunRecord>>,
}

impl ModelProviderAdapter for Hooks<'_> {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = AgentPlanExchange;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<AgentPlanExchange>, PreDispatchRefusal> {
        *self.retrieve.lock().unwrap() = matches!(request, DestinationRequest::Retrieve(_));
        adapter().prepare(request).await
    }

    async fn exchange(
        &self,
        prepared: AgentPlanExchange,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        if *self.retrieve.lock().unwrap() {
            let record = read(self.store, &self.scope, &self.run_id).await;
            *self.during.lock().unwrap() = Some(record);
        }
        let observation = adapter().exchange(prepared, sink).await;
        let count = {
            let mut exchanges = self.exchanges.lock().unwrap();
            *exchanges += 1;
            *exchanges
        };
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

/// The retrieval rows of the Run: the Context Assembly manifest and the Operation Requirement,
/// each with its position.
async fn retrieval_context(admin: &Client, run_id: &str) -> Vec<String> {
    admin
        .query(
            "SELECT 'manifest:' || decision_position::text FROM storyos.context_assembly_manifests
              WHERE run_id = $1::text::uuid AND manifest_role = 'retrieval'
             UNION ALL
             SELECT 'requirement:' || decision_position::text FROM storyos.operation_requirements
              WHERE run_id = $1::text::uuid AND requirement_role = 'retrieval'
             ORDER BY 1",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(/*idx*/ 0))
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

/// Admits one research Run. Its first response requests one search, the fake MCP server answers
/// it, and the continuation stream of position 1 reports `resp_<prefix>_2` and then breaks. The
/// Retrieve reply `retrieved` gets the Snapshot identities that the continuation request tagged.
async fn continuation_run(
    store: &PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    suffix: u16,
    retrieved: fn(&str, &[String]) -> String,
) -> (ProjectScope, String, tokio::task::JoinHandle<Vec<String>>) {
    let (scope, run_id) = real_run(store, admin, prefix, "http://127.0.0.1:9/api/plan/v3").await;
    let continuation = format!("resp_{prefix}_2");
    let sources = Arc::new(Mutex::new(Vec::new()));
    let tagged_sources = Arc::clone(&sources);
    let first = calls_stream(
        &format!("resp_{prefix}"),
        &[("call_01", "web_search", SEARCH)],
    );
    let broken = broken_stream(&continuation);
    let replies: Vec<Reply> = vec![
        Box::new(move |_| first),
        Box::new(move |body| {
            *tagged_sources.lock().unwrap() = tagged(body, "<source id=\"");
            broken
        }),
        Box::new(move |_| retrieved(&continuation, &sources.lock().unwrap())),
    ];
    let (model_base, served) = computed(replies).await;
    admin
        .execute(
            "UPDATE storyos.processing_destination_identity_evidence_revisions
                SET endpoint = $2 WHERE project_id = $1::text::uuid AND endpoint IS NOT NULL",
            &[&scope.project_id.as_ref(), &model_base],
        )
        .await
        .unwrap();
    set_research_tool(store, &scope, suffix, ToolEnablement::Enabled).await;
    let result = reply(
        /*id*/ 0,
        json!({"content": [{"type": "text", "text": SEARCH_TEXT}], "isError": false}),
    );
    let (mcp_base, _mcp) = destination(mcp_replies(exa_tools(|_| {}), Some(result))).await;
    point_mcp(admin, &scope, &mcp_base).await;
    (scope, run_id, served)
}

/// Claims the Run and settles it with `model`, as one Worker does.
async fn drain(
    store: &PostgresProjectReader,
    admin: &Client,
    model: &Hooks<'_>,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<(), CompleteAgentRunError> {
    let claim = claim_run(admin, scope.clone(), run_id.to_owned()).await;
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
) -> Hooks<'a> {
    Hooks {
        store,
        admin,
        scope: scope.clone(),
        run_id: run_id.to_owned(),
        crash_after,
        exchanges: Mutex::new(/*value*/ 0),
        retrieve: Mutex::new(/*value*/ false),
        during: Mutex::new(/*value*/ None),
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
    let synthesis = |reference: &str, sources: &[String]| {
        let text = json!({"kind": "research_synthesis", "summary": "The tides of 1890.",
            "claims": [{"text": "High water came at 10.42.", "citations": [sources[0]]}]});
        stored_message(reference, &text.to_string(), "completed")
    };
    let (scope, run_id, served) = continuation_run(&store, &admin, "d81", 0xd81a, synthesis).await;
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
        ),
        &scope,
        &run_id,
    )
    .await;
    let restart = hooks(&store, &admin, &scope, &run_id, /*crash_after*/ None);
    drain(&store, &admin, &restart, &scope, &run_id)
        .await
        .unwrap();
    let after = read(&store, &scope, &run_id).await;
    // A second drain finds the Run settled and sends nothing.
    let again = hooks(&store, &admin, &scope, &run_id, /*crash_after*/ None);
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

    assert_eq!(
        (
            matches!(crashed, Err(CompleteAgentRunError::StaleFence)),
            restart.during.lock().unwrap().as_ref().map(inspect),
            inspect(&after),
            read(&store, &scope, &run_id).await == after,
            again.during.lock().unwrap().is_none(),
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
            true,
            vec![
                json!({"role": "decision", "position": "0", "dispatch_state": "settled",
                       "usage": {"kind": "reported", "input_tokens": 90, "output_tokens": 30},
                       "reservation": null, "result": null, "own_evidence": true}),
                json!({"role": "decision", "position": "1", "dispatch_state": "settled",
                       "usage": {"kind": "reported", "input_tokens": 40, "output_tokens": 12},
                       "reservation": released, "result": null, "own_evidence": true}),
                // The Retrieve keeps no usage of its own, so the Run counts the response once.
                json!({"role": "retrieval", "position": "1", "dispatch_state": "settled",
                       "usage": {"kind": "unknown"}, "reservation": reserved,
                       "result": "complete_selected", "own_evidence": true}),
            ],
            vec!["manifest:1".to_owned(), "requirement:1".to_owned()],
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

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_not_found_retrieve_of_a_continuation_keeps_the_unknown_outcome_and_the_run_waits() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, run_id, served) =
        continuation_run(&store, &admin, "d82", 0xd82a, |_, _| not_found()).await;
    let model = hooks(&store, &admin, &scope, &run_id, /*crash_after*/ None);
    drain(&store, &admin, &model, &scope, &run_id)
        .await
        .unwrap();
    let after = read(&store, &scope, &run_id).await;
    // A restart finds the retrieval reconciled and sends no second Retrieve.
    let restart = hooks(&store, &admin, &scope, &run_id, /*crash_after*/ None);
    drain(&store, &admin, &restart, &scope, &run_id)
        .await
        .unwrap();
    let settlement: Option<String> = admin
        .query_one(
            "SELECT settlement::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let reserved = json!({"kind": "worst_case", "released": false});

    assert_eq!(
        (
            model.during.lock().unwrap().as_ref().map(inspect),
            inspect(&after),
            read(&store, &scope, &run_id).await == after,
            restart.during.lock().unwrap().is_none(),
            settlement,
            attempts(&admin, &run_id).await,
            retrieval_context(&admin, &run_id).await,
            served.await.unwrap(),
        ),
        (
            Some(json!({"status": "Claimed", "decision": "absent",
                        "dispatch_state": "uncertain", "disposition": null,
                        "keep_reason": null, "reservation_released": null, "usage_kind": null})),
            // A waiting Run with no Decision shows the stopped-request sentence (ADR 0057).
            json!({"status": "Waiting", "decision": "absent", "dispatch_state": "uncertain",
                   "disposition": "kept_unknown", "keep_reason": "unknown_result",
                   "reservation_released": false, "usage_kind": "unknown"}),
            true,
            true,
            Some(r#"{"kind": "outcome_unknown", "reason": "unknown_result"}"#.to_owned()),
            vec![
                json!({"role": "decision", "position": "0", "dispatch_state": "settled",
                       "usage": {"kind": "reported", "input_tokens": 90, "output_tokens": 30},
                       "reservation": null, "result": null, "own_evidence": true}),
                json!({"role": "decision", "position": "1", "dispatch_state": "uncertain",
                       "usage": {"kind": "unknown"}, "reservation": reserved,
                       "result": null, "own_evidence": true}),
                json!({"role": "retrieval", "position": "1", "dispatch_state": "uncertain",
                       "usage": {"kind": "unknown"}, "reservation": reserved,
                       "result": "unknown", "own_evidence": true}),
            ],
            vec!["manifest:1".to_owned(), "requirement:1".to_owned()],
            vec![
                post(),
                post(),
                "GET /api/plan/v3/responses/resp_d82_2 HTTP/1.1".to_owned(),
            ],
        )
    );
}
