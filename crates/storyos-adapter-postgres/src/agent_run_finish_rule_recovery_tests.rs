use std::sync::Mutex;

use serde_json::json;
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, CacheUse,
    ConversationSelection, CreateRequest, DestinationRequest, ModelProviderAdapter, ModelResponse,
    ModelStreamSink, ModelUsage, NoContractFaults, Observation, PreDispatchRefusal,
    PreparedRequest, ToolOutputOrigin, complete_agent_run,
};
use storyos_core::{
    ADVISORY_TEXT, AgentRunStatus, CallValidationRefusal, DeploymentDestination,
    FINISH_DOES_NOT_COUNT_RESULT, NativeStreamItem, StreamItemRole, StreamItemState, ToolChoice,
};

use super::tests::{calls, complete};
use crate::agent_run_dispatch::tests::{
    RuntimeClock, admit_run_in, claim_run, queued_run_offering, stores,
};

/// The RunStep whose first create the destination loses.
#[derive(Clone, Copy, Debug)]
enum LostStep {
    /// RunStep 2 (position 1) of `SCRIPT:refuse look_up_notes {}`: an ordinary RunStep.
    Ordinary,
    /// RunStep 2 (position 1) after a `finish` call that counted: the closing RunStep.
    Closing,
}

impl LostStep {
    fn loses(self, create: &CreateRequest) -> bool {
        let prior = |count| {
            create.tool_outputs.len() == count
                && create
                    .tool_outputs
                    .iter()
                    .all(|output| output.origin == ToolOutputOrigin::PriorResponse)
        };
        match self {
            Self::Ordinary => create.tool_choice == ToolChoice::Required && prior(2),
            Self::Closing => create.tool_choice == ToolChoice::None && prior(1),
        }
    }

    /// The author message of each create that the destination does not lose.
    fn script(self) -> &'static str {
        match self {
            Self::Ordinary => "SCRIPT:refuse look_up_notes {}",
            Self::Closing => "Help with this passage.",
        }
    }
}

/// The fake destination that loses the first create of one RunStep. The recovery script of the
/// author message selects the path. The Retrieve of the lost reference when `retrieved`, or the
/// successor create, answers with `recovered`. Each create keeps its `tool_choice` and whether
/// it is a successor.
struct LoseOneCreate {
    step: LostStep,
    retrieved: bool,
    recovered: Vec<NativeStreamItem>,
    creates: Mutex<Vec<(ToolChoice, bool)>>,
}

enum LostExchange {
    Fake(<FakeDestination as ModelProviderAdapter>::Prepared),
    Recovered,
}

impl ModelProviderAdapter for LoseOneCreate {
    const ADAPTERS: &'static [storyos_core::ModelAdapter] = FakeDestination::ADAPTERS;
    type Prepared = LostExchange;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal> {
        let request = match request {
            DestinationRequest::Create(create) => {
                self.creates
                    .lock()
                    .unwrap()
                    .push((create.tool_choice, create.successor_of.is_some()));
                match self.step.loses(create) {
                    true => request.clone(),
                    false => DestinationRequest::Create(CreateRequest {
                        author_message: self.step.script().to_owned(),
                        ..create.clone()
                    }),
                }
            }
            other => other.clone(),
        };
        let prepared = FakeDestination.prepare(&request).await?;
        let recovered = match &request {
            DestinationRequest::Retrieve(retrieve) => {
                self.retrieved
                    && retrieve.purpose == storyos_application::RetrievePurpose::OriginalResult
            }
            DestinationRequest::Create(create) => create.successor_of.is_some(),
            DestinationRequest::Abort(_) => false,
        };
        Ok(PreparedRequest {
            prepared: match recovered {
                true => LostExchange::Recovered,
                false => LostExchange::Fake(prepared.prepared),
            },
            projection: prepared.projection,
        })
    }

    async fn exchange(
        &self,
        prepared: Self::Prepared,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        let LostExchange::Fake(prepared) = prepared else {
            return Observation::Terminal(ModelResponse {
                state: StreamItemState::Complete,
                items: self.recovered.clone(),
                output: None,
                usage: ModelUsage::Unknown,
                cache_use: CacheUse::Unknown,
                response_reference: None,
            });
        };
        FakeDestination.exchange(prepared, sink).await
    }
}

const RECOVERED_ANSWER: &str = "The recovered final answer.";

fn text(text: &str) -> NativeStreamItem {
    NativeStreamItem {
        item_id: "msg_recovered".to_owned(),
        role: StreamItemRole::Assistant,
        state: StreamItemState::Complete,
        text: Some(text.to_owned()),
        summary: None,
        call_id: None,
        arguments: None,
        refusal: None,
        native_type: None,
        name: None,
    }
}

fn call(name: &str) -> NativeStreamItem {
    NativeStreamItem {
        item_id: format!("fc_{name}_recovered"),
        role: StreamItemRole::Tool,
        text: None,
        call_id: Some(format!("call_{name}_recovered")),
        arguments: Some("{}".to_owned()),
        native_type: Some(storyos_core::FUNCTION_CALL_NATIVE_TYPE.to_owned()),
        name: Some(name.to_owned()),
        ..text("")
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_recovered_response_gives_the_agent_decision_of_a_direct_response_at_each_run_step() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let unexposed = CallValidationRefusal::Unexposed;
    let refused = |position: u64| {
        json!([
            position,
            "look_up_notes",
            "refused",
            unexposed.label(),
            unexposed.result_text()
        ])
    };
    let finish = |position: u64, counts: bool| {
        json!([
            position,
            "finish",
            "settled",
            null,
            if counts {
                ""
            } else {
                FINISH_DOES_NOT_COUNT_RESULT
            }
        ])
    };
    // The refused call and the `finish` call of RunStep 1 come before each ordinary RunStep.
    let before = [refused(0), finish(0, false)];
    let cases = [
        // A text with no function call opens the next RunStep with the same `tool_choice`.
        (
            LostStep::Ordinary,
            vec![text(ADVISORY_TEXT)],
            [before.to_vec(), vec![finish(2, true)]].concat(),
            ADVISORY_TEXT,
        ),
        // The calls become ToolCalls, and the `finish` call counts.
        (
            LostStep::Ordinary,
            vec![call("finish")],
            [before.to_vec(), vec![finish(1, true)]].concat(),
            ADVISORY_TEXT,
        ),
        // A refused call is a result, the `finish` call beside it does not count, and the Run
        // continues.
        (
            LostStep::Ordinary,
            vec![call("look_up_notes"), call("finish")],
            [
                before.to_vec(),
                vec![refused(1), finish(1, false), finish(2, true)],
            ]
            .concat(),
            ADVISORY_TEXT,
        ),
        // The text of the closing response is the final answer.
        (
            LostStep::Closing,
            vec![text(RECOVERED_ANSWER)],
            vec![finish(0, true)],
            RECOVERED_ANSWER,
        ),
        // A closing response with no text completes the Run with no result.
        (
            LostStep::Closing,
            vec![text("")],
            vec![finish(0, true)],
            "Absent",
        ),
        // A call in the closing response gets its result, and the Run continues.
        (
            LostStep::Closing,
            vec![call("finish")],
            vec![finish(0, true), finish(1, true)],
            ADVISORY_TEXT,
        ),
    ];
    let prefixes = (0x3b0..).step_by(2).map(|lead: u32| format!("{lead:03x}"));
    let paths = [
        ("SCRIPT:retrieve-complete", true),
        ("SCRIPT:successor-once", false),
    ];
    let runs = cases
        .iter()
        .flat_map(|case| paths.iter().map(move |path| (case, path)));
    for (((step, recovered, expected_calls, expected_answer), (message, retrieved)), first) in
        runs.zip(prefixes)
    {
        let second = format!("{:03x}", u32::from_str_radix(&first, 16).unwrap() + 1);
        let destination = DeploymentDestination::HostFakeToolLoop;
        let (scope, chapter_id, first_run) =
            queued_run_offering(&store, &first, destination.clone(), None).await;
        complete(&store, &admin, &scope, &first_run).await;
        let run_id = admit_run_in(
            &store,
            &scope,
            &chapter_id,
            &second,
            destination,
            message,
            ConversationSelection::New,
            None,
        )
        .await;
        let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
        let adapter = LoseOneCreate {
            step: *step,
            retrieved: *retrieved,
            recovered: recovered.clone(),
            creates: Mutex::new(Vec::new()),
        };
        // A recovered response that opens a new RunStep each time does not end.
        tokio::time::timeout(
            std::time::Duration::from_secs(60),
            complete_agent_run(&store, &adapter, &NoContractFaults, &RuntimeClock, &claim),
        )
        .await
        .expect("The Run ends")
        .unwrap();
        let record = store
            .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
            .await
            .unwrap()
            .unwrap();
        // The recovered response of RunStep 2 is the prior response of the RunStep or the Run
        // after it.
        let bound: i64 = admin
            .query_one(
                "SELECT count(*) FROM storyos.model_attempts
                  WHERE run_id = $1::text::uuid AND decision_position = 1
                    AND continuation_binding_id IS NOT NULL",
                &[&run_id],
            )
            .await
            .unwrap()
            .get(0);
        let answer = match record.decision {
            AgentRunDecisionInspect::Advisory { text, .. } => json!(text),
            other => json!(format!("{other:?}")),
        };
        // The successor request keeps the `tool_choice` of its RunStep.
        let successor_choices: Vec<_> = adapter
            .creates
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, successor)| *successor)
            .map(|(choice, _)| choice.as_str())
            .collect();
        let expected_successor = match (retrieved, step) {
            (true, _) => vec![],
            (false, LostStep::Ordinary) => vec!["required"],
            (false, LostStep::Closing) => vec!["none"],
        };
        assert_eq!(
            (
                calls(&admin, &run_id).await,
                record.status,
                answer,
                successor_choices,
                bound
            ),
            (
                expected_calls.clone(),
                AgentRunStatus::Completed,
                json!(expected_answer),
                expected_successor,
                1
            ),
            "{step:?} {message} {recovered:?}"
        );
    }
}
