//! The recovery of a capture (ADR 0086, ADR 0088, ADR 0094 section 5): a lost Run Lease, a second
//! delivery of the settled output call, a stale fence, a request with an unknown result, and a
//! rejected request. A capture ends with one set of observations or with no observation.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    CacheUse, ContractFaultPoint, CreatePurpose, CreateRequest, DestinationRequest,
    MemoryCaptureSource, ModelProviderAdapter, ModelResponse, ModelStreamSink, ModelUsage,
    NoContractFaults, Observation, PreDispatchRefusal, PreparedRequest, ProjectScope,
    RetrievePurpose, ToolDispatchStore, complete_agent_run,
};
use storyos_core::{NativeStreamItem, StreamItemRole, StreamItemState};
use tokio_postgres::Client;

use super::loop_tests::{record, script, settlement};
use super::tests::{project, run_capture, start};
use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{RuntimeClock, claim_run, stores};
use crate::agent_run_tool_recovery::tests::{Fault, FaultAt};
use crate::create_agent_run::context::chapter_list_tests::replace_text;

/// What the destination does with the first create of one RunStep of a capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaptureFault {
    /// The result is unknown, and the Retrieve of the reference gives the response.
    Retrieve,
    /// The result is unknown, and the one bounded successor gives the response.
    Successor,
    /// The destination rejects the request.
    Reject,
}

/// The fake destination with `fault` at the RunStep `step` of a capture. The RunStep of a create
/// of a capture is the count of its Tool outputs.
struct FaultyCapture {
    step: usize,
    fault: CaptureFault,
    recovered: Vec<NativeStreamItem>,
}

enum FaultyExchange {
    Fake(<FakeDestination as ModelProviderAdapter>::Prepared),
    Recovered,
    Rejected,
}

impl ModelProviderAdapter for FaultyCapture {
    const ADAPTERS: &'static [storyos_core::ModelAdapter] = FakeDestination::ADAPTERS;
    type Prepared = FaultyExchange;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal> {
        let at_step = matches!(request, DestinationRequest::Create(create)
            if matches!(create.purpose, CreatePurpose::MemoryCapture(_))
                && create.tool_outputs.len() == self.step);
        let marker = match self.fault {
            CaptureFault::Retrieve => "SCRIPT:retrieve-complete",
            CaptureFault::Successor => "SCRIPT:successor-once",
            CaptureFault::Reject => "",
        };
        let request = match request {
            // The original create and its successor send the same marker, so that the successor
            // is the same request.
            DestinationRequest::Create(create) if at_step => {
                DestinationRequest::Create(CreateRequest {
                    author_message: marker.to_owned(),
                    ..create.clone()
                })
            }
            other => other.clone(),
        };
        let prepared = FakeDestination.prepare(&request).await?;
        let exchange = match (&request, self.fault) {
            (DestinationRequest::Create(_), CaptureFault::Reject) if at_step => {
                FaultyExchange::Rejected
            }
            (DestinationRequest::Retrieve(retrieve), CaptureFault::Retrieve)
                if retrieve.purpose == RetrievePurpose::OriginalResult =>
            {
                FaultyExchange::Recovered
            }
            _ => FaultyExchange::Fake(prepared.prepared),
        };
        Ok(PreparedRequest {
            prepared: exchange,
            projection: prepared.projection,
        })
    }

    async fn exchange(
        &self,
        prepared: Self::Prepared,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        match prepared {
            FaultyExchange::Fake(prepared) => FakeDestination.exchange(prepared, sink).await,
            FaultyExchange::Rejected => Observation::Rejected {
                reason: "invalid_request".to_owned(),
            },
            FaultyExchange::Recovered => Observation::Terminal(ModelResponse {
                state: StreamItemState::Complete,
                items: self.recovered.clone(),
                usage: ModelUsage::Unknown,
                cache_use: CacheUse::Unknown,
                response_reference: None,
            }),
        }
    }
}

/// The output call of a capture with `observations`, as a response gives it.
fn output_call(observations: &Value) -> NativeStreamItem {
    NativeStreamItem {
        item_id: "fc_capture_recovered".to_owned(),
        role: StreamItemRole::Tool,
        state: StreamItemState::Complete,
        text: None,
        summary: None,
        call_id: Some("call_capture_recovered".to_owned()),
        arguments: Some(record(observations.clone()).1.to_string()),
        refusal: None,
        native_type: Some(storyos_core::FUNCTION_CALL_NATIVE_TYPE.to_owned()),
        name: Some("record_memory_observations".to_owned()),
    }
}

/// The role of each Model Attempt of the Run, in order.
async fn attempt_roles(admin: &Client, run_id: &str) -> Vec<String> {
    admin
        .query(
            "SELECT attempt_role || ':' || decision_position::text FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid ORDER BY model_attempt_id",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(/*idx*/ 0))
        .collect()
}

/// The Worker loses the Run Lease in a capture of each source of `projects`: the first after the
/// response committed, the second before the Tool Gateway resolves the output call. Returns, for
/// each, the stale results, the settlement before and after the next claim, whether a second
/// delivery of the settled output call found no ToolCall, and the roles of the Model Attempts.
async fn lost_lease_ends(
    store: &PostgresProjectReader,
    admin: &Client,
    projects: [(ProjectScope, MemoryCaptureSource); 2],
) -> Vec<Value> {
    let item = json!({"type": "foreshadowing", "title": "铜铃", "text": "铜铃会再响。"});
    let points = [
        ContractFaultPoint::StreamCommitted,
        ContractFaultPoint::ToolCallsCommitted,
    ];
    let mut ended = Vec::new();
    for ((scope, source), point) in projects.into_iter().zip(points) {
        let started = start(store, &scope, &source, &script(&[record(json!([item]))]))
            .await
            .unwrap();
        let run_id = started.run_id.clone();
        let stale = claim_run(admin, scope.clone(), run_id.clone()).await;
        let fault = Fault::TakeOver {
            admin,
            run_id: run_id.clone(),
        };
        let observer = FaultAt::new(point, 1, fault);
        let lost =
            complete_agent_run(store, &FakeDestination, &observer, &RuntimeClock, &stale).await;
        let stale_delivery = store.next_tool_call(&stale).await;
        let before = settlement(admin, &run_id).await;
        let current = claim_run(admin, scope.clone(), run_id.clone()).await;
        complete_agent_run(
            store,
            &FakeDestination,
            &NoContractFaults,
            &RuntimeClock,
            &current,
        )
        .await
        .unwrap();
        let second_delivery = store.next_tool_call(&current).await.unwrap();
        ended.push(json!([
            lost.is_err() && stale_delivery.is_err(),
            before,
            second_delivery.is_none(),
            settlement(admin, &run_id).await,
            attempt_roles(admin, &run_id).await
        ]));
    }
    ended
}

/// The result of `lost_lease_ends` for one point: the capture ends with one set of observations.
fn one_set() -> Value {
    let observation = json!([1, "foreshadowing", "铜铃", "铜铃会再响。", "waiting"]);
    json!([
        true,
        ["claimed", "running", null, null, []],
        true,
        [
            "completed",
            "settled",
            "observations_recorded",
            null,
            [observation]
        ],
        ["decision:0"]
    ])
}

/// When the Worker loses the Run Lease after the response of the capture committed (a Model
/// Gateway point) or before the Tool Gateway resolves its output call (a Tool Gateway point), its
/// stale fence stores nothing. The next claim continues the same RunStep, and the capture ends
/// with one set of observations. A second delivery of the settled output call changes nothing.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_lost_lease_and_a_second_delivery_give_one_set_of_observations() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut projects = Vec::new();
    for prefix in ["fe1", "fe5"] {
        let (scope, _, source) = project(&store, &admin, prefix).await;
        projects.push((scope, source));
    }
    let projects: [_; 2] = projects.try_into().unwrap();

    assert_eq!(
        lost_lease_ends(&store, &admin, projects).await,
        [one_set(), one_set()]
    );
}

/// S7-E1b: the same two lost Run Leases in a capture of a Chapter. A stop in the Run of the capture
/// leaves no part of a result, and the next claim completes the capture with one set of
/// observations.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_chapter_capture_that_loses_its_lease_ends_with_one_set_of_observations() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut projects = Vec::new();
    for prefix in ["a22", "a23"] {
        let (scope, chapter_id, _) = project(&store, &admin, prefix).await;
        let revision_id: String = admin
            .query_one(
                "SELECT current_revision_id::text FROM storyos.authoritative_heads
                  WHERE manuscript_object_id = $1::text::uuid",
                &[&chapter_id],
            )
            .await
            .unwrap()
            .get(/*idx*/ 0);
        let source = MemoryCaptureSource::Chapter {
            chapter_id,
            revision_id,
        };
        projects.push((scope, source));
    }
    let projects: [_; 2] = projects.try_into().unwrap();

    assert_eq!(
        lost_lease_ends(&store, &admin, projects).await,
        [one_set(), one_set()]
    );
}

/// A request of a capture with an unknown result recovers by the Retrieve of its reference or by
/// the one bounded successor, at the first RunStep and at a later RunStep, as a Run of the author
/// does. A rejected request fails the capture with the reason of its Run.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_unknown_result_recovers_and_a_rejected_request_fails_the_capture() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "fe2").await;
    let item = json!({"type": "character", "title": "林舟", "text": "撑船的人。"});
    let refused = ("propose_edit", json!({"reason": "改写"}));
    let cases = [
        (0, CaptureFault::Retrieve),
        (0, CaptureFault::Successor),
        (1, CaptureFault::Retrieve),
        (1, CaptureFault::Successor),
        (0, CaptureFault::Reject),
    ];
    let mut ended = Vec::new();
    for (step, fault) in cases {
        let mut lines = vec![refused.clone(); step];
        lines.push(record(json!([item])));
        let mut input = script(&lines);
        if fault == CaptureFault::Successor {
            input.push_str("\nSCRIPT:successor-once");
        }
        let started = start(&store, &scope, &source, &input).await.unwrap();
        let adapter = FaultyCapture {
            step,
            fault,
            recovered: vec![output_call(&json!([item]))],
        };
        for _ in 0..=storyos_core::MEMORY_CAPTURE_MAX_RUN_STEPS {
            let claim = claim_run(&admin, scope.clone(), started.run_id.clone()).await;
            complete_agent_run(&store, &adapter, &NoContractFaults, &RuntimeClock, &claim)
                .await
                .unwrap();
            if settlement(&admin, &started.run_id).await[0] != "queued" {
                break;
            }
        }
        let outcome = settlement(&admin, &started.run_id).await;
        let roles = attempt_roles(&admin, &started.run_id).await;
        ended.push(json!([
            outcome[0],
            outcome[2],
            outcome[3],
            outcome[4].as_array().map(Vec::len),
            roles
        ]));
    }

    let recorded = |roles: &[&str]| json!(["completed", "observations_recorded", null, 1, roles]);
    assert_eq!(
        ended,
        [
            recorded(&["decision:0", "retrieval:0"]),
            recorded(&[
                "decision:0",
                "retrieval:0",
                "successor:0",
                "late_retrieval:0"
            ]),
            recorded(&["decision:0", "decision:1", "retrieval:1"]),
            recorded(&[
                "decision:0",
                "decision:1",
                "retrieval:1",
                "successor:1",
                "late_retrieval:1"
            ]),
            json!([
                "refused",
                "failed",
                "destination_refused",
                0,
                ["decision:0"]
            ]),
        ]
    );
}

/// An edit of a Chapter during a capture does not change its pinned input: each request sends the
/// input and the Chapter list of the start. A later manuscript read gives the current text.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_edit_of_a_chapter_does_not_change_a_capture() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, _, session) =
        crate::agent_run_read_chapter::tests::project(&store, &admin, "fe3", 0xfe40, &["Lin ran."])
            .await;
    let source = super::tools_tests::conversation_source(&admin, &scope).await;
    let read = json!({"chapter": 1, "first_block": null, "last_block": null});
    let input = script(&[("read_chapter", read)]);
    let started = start(&store, &scope, &source, &input).await.unwrap();
    let edit = (session.as_str(), 2, "fe43");
    let edit = replace_text(&store, &admin, &scope, edit, &chapter_id, (3, "Wei")).await;
    store.apply_author_edit(&edit).await.unwrap();
    run_capture(&store, &admin, &scope, &started.run_id).await;

    let inputs: Vec<Value> = admin
        .query(
            "SELECT jsonb_path_query_array(payload,
                      '$.selected[*] ? (@.source_class == \"memory_capture_input\"
                                       || @.source_class == \"chapter_list\").content')::text
               FROM storyos.context_assembly_manifests
              WHERE run_id = $1::text::uuid AND manifest_role = 'decision'
              ORDER BY decision_position",
            &[&started.run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
        .collect();
    // The pinned input keeps the Chapter list of the start: 8 characters before the edit.
    let chapters = r#"{"volume":"Volume A"}
[1,8,"Chapter A"]
[2,0,"Fog"]"#;
    assert_eq!(inputs, vec![json!([input, chapters]); 2]);
    let read: String = admin
        .query_one(
            "SELECT result_text FROM storyos.tool_calls
              WHERE run_id = $1::text::uuid AND function_name = 'read_chapter'",
            &[&started.run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    // The start had "Lin ran."; the read gives the current authoritative text.
    assert!(read.ends_with("\n[block 1]\nWei ran."));
    assert_eq!(
        settlement(&admin, &started.run_id).await,
        json!(["completed", "settled", "no_observation", null, []])
    );
}
