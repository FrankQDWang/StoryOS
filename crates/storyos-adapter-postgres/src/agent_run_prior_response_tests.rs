//! Each request that names a prior response gives one output for each call of that response, in
//! the body of the tool loop wire (ADR 0085 section 3, ADR 0091 section 5). The fake model of
//! the tool loop profile answers each request.

use std::sync::Mutex;

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AgentRunReadSelection, AgentRunReadStore, ConversationSelection, CreatePurpose, CreateRequest,
    DestinationRequest, DispatchClaim, ModelProviderAdapter, ModelStreamSink, NoContractFaults,
    Observation, PreDispatchRefusal, PreparedRequest, RequestAttempt, RequestRoute,
    complete_agent_run,
};
use storyos_core::{
    ACTIVE_COMPACTION_REQUEST_PREFIX, AgentRunStatus, DeploymentDestination, RequestBounds,
    UNEXECUTED_CALL_OUTPUT,
};

use crate::agent_run_dispatch::tests::{RuntimeClock, admit_run_in, claim_run, stores};
use crate::agent_run_finish_rule::tests::tool_loop_run;
use crate::agent_run_steering::tests::{AuthorAction, AuthorAt, Moment, queued, settle, steering};

/// The adapter `inner`, which also keeps the input of each Create request in the body that the
/// tool loop wire of the Agent Plan route builds for it.
struct RealWire<A> {
    inner: A,
    bodies: Mutex<Vec<Value>>,
}

impl<A> RealWire<A> {
    fn new(inner: A) -> Self {
        Self {
            inner,
            bodies: Mutex::new(Vec::new()),
        }
    }

    /// For each Create request in order: its purpose, whether it names a prior response, and
    /// each input item as `message`, `[output, call_id]`, `[unexecuted, call_id]`, or
    /// `[call, call_id]`.
    fn requests(self) -> Value {
        Value::Array(self.bodies.into_inner().unwrap())
    }
}

impl<A: ModelProviderAdapter + Sync> ModelProviderAdapter for RealWire<A> {
    const ADAPTERS: &'static [storyos_core::ModelAdapter] = A::ADAPTERS;
    type Prepared = A::Prepared;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal> {
        if let DestinationRequest::Create(create) = request {
            let wire = CreateRequest {
                // A claimed Attempt needs no credential, and its prepare sends nothing.
                attempt: RequestAttempt::Claimed(DispatchClaim {
                    model_attempt_id: "018f0000-0000-7001-8000-000000057f00".to_owned(),
                }),
                route: RequestRoute {
                    mapping_revision: storyos_core::AGENT_PLAN_TOOL_LOOP_MAPPING_REVISION
                        .to_owned(),
                    endpoint: Some("http://127.0.0.1:9".to_owned()),
                    bounds: Some(RequestBounds {
                        max_output_tokens: 1024,
                        timeout: std::time::Duration::from_secs(/*secs*/ 60),
                    }),
                    ..create.route.clone()
                },
                ..create.clone()
            };
            let prepared = crate::agent_run_retrieval::tests::adapter()
                .prepare(&DestinationRequest::Create(wire))
                .await
                .ok()
                .unwrap();
            let body: Value =
                serde_json::from_str(&prepared.projection.serialized_payload.unwrap()).unwrap();
            let items: Vec<Value> = body["input"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| match item["type"].as_str() {
                    Some("function_call_output") if item["output"] == UNEXECUTED_CALL_OUTPUT => {
                        json!(["unexecuted", item["call_id"]])
                    }
                    Some("function_call_output") => json!(["output", item["call_id"]]),
                    Some("function_call") => json!(["call", item["call_id"]]),
                    _ => json!("message"),
                })
                .collect();
            let purpose = match create.purpose {
                CreatePurpose::CompactionSummary => "summary",
                CreatePurpose::Decision | CreatePurpose::ToolQualification(_) => "decision",
            };
            self.bodies.lock().unwrap().push(json!([
                purpose,
                body["previous_response_id"].is_string(),
                items
            ]));
        }
        self.inner.prepare(request).await
    }

    async fn exchange(
        &self,
        prepared: Self::Prepared,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        self.inner.exchange(prepared, sink).await
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_request_after_a_steering_input_between_two_requests_answers_each_call_of_its_prior_response()
 {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, run_id) = queued(&store, "570").await;
    let input = steering(&store, &scope, 0x57f0, &run_id, "Keep the voice.", None).await;
    // After the result of the `finish` call of RunStep 1, and before the closing request goes
    // out: the RunStep that reads the input has the next position, and the closing RunStep has
    // no Model Attempt.
    let author = AuthorAt::new(
        &store,
        Moment::BeforeRequest(2),
        AuthorAction::Steer(vec![input]),
    );
    let adapter = RealWire::new(author);
    settle(&store, &admin, &scope, &run_id, &adapter)
        .await
        .unwrap();
    let record = store
        .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();

    // The request of the RunStep that read the input names the response of RunStep 1, so it
    // answers the `finish` call of that response after the current input.
    assert_eq!(
        (adapter.requests(), record.status),
        (
            json!([
                ["decision", false, ["message"]],
                ["decision", true, ["message", ["output", "call_finish"]]],
                ["decision", true, [["output", "call_finish_1"]]],
            ]),
            AgentRunStatus::Completed,
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_next_run_answers_each_call_of_the_last_run_step_with_the_unexecuted_output() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let unexecuted = json!([
        ["unexecuted", "call_look_up_notes_23"],
        ["unexecuted", "call_finish_23"]
    ]);
    let cases = [
        // The response of RunStep 24 has a final answer and two calls. The first request of the
        // next Run sends the author request, then an output for each call.
        (
            ["571", "572", "573"],
            "SCRIPT:never_finish",
            "Help again.".to_owned(),
            json!([
                ["decision", true, ["message", unexecuted[0], unexecuted[1]]],
                ["decision", true, [["output", "call_finish"]]],
            ]),
        ),
        // The response of RunStep 24 has only the two calls. The summary request of the next
        // Run answers them before its instruction, and the later request starts a new
        // transport.
        (
            ["574", "575", "576"],
            "SCRIPT:never_finish_quietly",
            format!("{ACTIVE_COMPACTION_REQUEST_PREFIX} Help again."),
            json!([
                ["summary", true, [unexecuted[0], unexecuted[1], "message"]],
                ["decision", false, ["message"]],
                ["decision", true, [["output", "call_finish"]]],
            ]),
        ),
    ];
    for ([first, limited, next], script, message, expected) in cases {
        let (scope, chapter_id, _, _) =
            tool_loop_run(&store, &admin, [first, limited], script, None, false).await;
        let run_id = admit_run_in(
            &store,
            &scope,
            &chapter_id,
            next,
            DeploymentDestination::HostFakeToolLoop,
            &message,
            ConversationSelection::Existing {
                conversation_id: format!("018f0000-0000-7001-8000-00000006{limited}6"),
            },
            None,
        )
        .await;
        let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
        let adapter = RealWire::new(FakeDestination);
        complete_agent_run(&store, &adapter, &NoContractFaults, &RuntimeClock, &claim)
            .await
            .unwrap();
        let record = store
            .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            (adapter.requests(), record.status),
            (expected, AgentRunStatus::Completed),
            "{script}"
        );
    }
}
