use serde_json::{Value, json};
use storyos_adapter_fake_destination::{FakeDestination, LIMIT_ANSWER};
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, CacheUse,
    ConversationSelection, CreateRequest, DestinationRequest, ModelProviderAdapter, ModelResponse,
    ModelStreamSink, ModelUsage, NoContractFaults, Observation, PreDispatchRefusal,
    PreparedRequest, complete_agent_run,
};
use storyos_core::{
    ADVISORY_TEXT, AgentRunStatus, CallValidationRefusal, DeploymentDestination,
    FINISH_DOES_NOT_COUNT_RESULT, IntakeRefusal, NativeStreamItem, RESEARCH_TOOL_OFFER,
    StreamItemRole, StreamItemState, ToolChoice,
};
use tokio_postgres::Client;

use crate::shared_tests::agent_run_finish_rule::{
    calls, complete, conversation_of, steps, tool_loop_run,
};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::model_gateway::{
    RuntimeClock, admit_run_in, claim_run, queued_run_offering,
};

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_ends_only_with_the_text_of_the_closing_response_after_a_finish_call() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (required, none) = ("required", "none");
    let cases = [
        (
            ["1f3", "1f4"],
            "Search the web: SCRIPT:text_before_finish",
            Some(&RESEARCH_TOOL_OFFER),
            json!([
                ["tool_request", required],
                [null, required],
                ["tool_request", required],
                ["advisory", none]
            ]),
            json!(ADVISORY_TEXT),
        ),
        (
            ["1f5", "1f6"],
            "SCRIPT:finish_twice",
            None,
            json!([
                ["tool_request", required],
                ["tool_request", none],
                ["advisory", none]
            ]),
            json!(ADVISORY_TEXT),
        ),
        (
            ["1f7", "1f8"],
            "SCRIPT:empty_final_answer",
            None,
            json!([["tool_request", required], [null, none]]),
            Value::Null,
        ),
    ];
    for (prefixes, message, research, expected_steps, expected_answer) in cases {
        let (scope, _, _, run_id) =
            tool_loop_run(&store, &admin, prefixes, message, research, false).await;
        let steps = steps(&admin, &run_id).await;
        let record = store
            .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
            .await
            .unwrap()
            .unwrap();
        let answer = match record.decision {
            AgentRunDecisionInspect::Advisory { text, .. } => json!(text),
            AgentRunDecisionInspect::Absent => Value::Null,
            other => panic!("{message}: {other:?}"),
        };
        assert_eq!(
            (
                json!(
                    steps
                        .iter()
                        .map(|step| json!([step[0], step[1]]))
                        .collect::<Vec<_>>()
                ),
                record.status,
                answer,
            ),
            (expected_steps, AgentRunStatus::Completed, expected_answer),
            "{message}"
        );
        // Each RunStep continues from the response of the RunStep before it, also from a text.
        for pair in steps.windows(2) {
            assert_eq!((&pair[1][3], pair[0][2].is_string()), (&pair[0][2], true));
        }
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_next_run_of_the_conversation_continues_from_the_closing_response() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // The second Run ends with a final answer with no text, and the third Run follows it.
    let message = "SCRIPT:empty_final_answer";
    let (scope, chapter_id, first, second) =
        tool_loop_run(&store, &admin, ["1f1", "1f2"], message, None, true).await;
    let record = store
        .read_agent_run(&scope, &first, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        record.decision,
        AgentRunDecisionInspect::Advisory {
            continuation_binding_id: Some(_),
            ..
        }
    ));
    let third = admit_run_in(
        &store,
        &scope,
        &chapter_id,
        "1fb",
        DeploymentDestination::HostFakeToolLoop,
        "Help again.",
        conversation_of("1f1"),
        None,
    )
    .await;
    complete(&store, &admin, &scope, &third).await;

    for (run, next) in [(&first, &second), (&second, &third)] {
        let closing = steps(&admin, run).await[1][2].clone();
        assert!(closing.is_string());
        assert_eq!(steps(&admin, next).await[0][3], closing);
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_rebuilt_run_records_its_reference_recovery_once_across_run_steps() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // The destination lost the closing reference of the second Run.
    let message = "Help with this passage. SCRIPT:reference-expires";
    let (scope, chapter_id, _, _) =
        tool_loop_run(&store, &admin, ["8f1", "8f2"], message, None, true).await;
    let third = admit_run_in(
        &store,
        &scope,
        &chapter_id,
        "8f3",
        DeploymentDestination::HostFakeToolLoop,
        "I changed my mind: keep the voice.",
        conversation_of("8f1"),
        None,
    )
    .await;
    complete(&store, &admin, &scope, &third).await;

    // The rebuild opens the first RunStep. The closing RunStep continues from that RunStep.
    let record = store
        .read_agent_run(&scope, &third, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    let recoveries: Vec<String> = admin
        .query(
            "SELECT disposition FROM storyos.context_reference_recoveries
              WHERE successor_run_id = $1::text::uuid",
            &[&third],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(0))
        .collect();
    assert_eq!(
        (record.status, recoveries, steps(&admin, &third).await.len()),
        (AgentRunStatus::Completed, vec!["rebuilt".to_owned()], 2)
    );
}

/// The function output of the call `call_id` that the request of RunStep 1 sent.
async fn second_step_output(admin: &Client, run_id: &str, call_id: &str) -> Value {
    json_of(
        admin,
        "SELECT (SELECT output -> 'output' FROM jsonb_array_elements(
                       (payload #>> '{wire,serialized_payload}')::jsonb -> 'tool_outputs')
                       AS output WHERE output ->> 'call_id' = $2)::text
               FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'
                AND decision_position = 1",
        &[&run_id, &call_id],
    )
    .await
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_refused_call_gives_its_reason_and_the_model_corrects_it_before_finish_counts() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let unexposed = CallValidationRefusal::Unexposed;
    let malformed = CallValidationRefusal::Intake(IntakeRefusal::Malformed);
    let query = CallValidationRefusal::Intake(IntakeRefusal::Query);
    let cases = [
        (["2d1", "2d2"], "look_up_notes", "{}", None, unexposed),
        (["2d3", "2d4"], "finish", r#"{"now":true}"#, None, malformed),
        (
            ["2d5", "2d6"],
            "web_search",
            r#"{"query":""}"#,
            Some(&RESEARCH_TOOL_OFFER),
            query,
        ),
    ];
    for (prefixes, name, arguments, research, refusal) in cases {
        let message = format!("SCRIPT:refuse {name} {arguments}");
        let (scope, _, _, run_id) =
            tool_loop_run(&store, &admin, prefixes, &message, research, false).await;
        let record = store
            .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
            .await
            .unwrap()
            .unwrap();
        let refused_call = format!("call_{name}_refused");

        // The refused call does not end the Run, and the `finish` call beside it does not
        // count. The next RunStep is an ordinary RunStep, and its `finish` call counts.
        assert_eq!(
            (
                calls(&admin, &run_id).await,
                json!(
                    steps(&admin, &run_id)
                        .await
                        .iter()
                        .map(|step| json!([step[0], step[1]]))
                        .collect::<Vec<_>>()
                ),
                second_step_output(&admin, &run_id, &refused_call).await,
                record.status,
                matches!(record.decision, AgentRunDecisionInspect::Advisory { ref text, .. }
                    if text == ADVISORY_TEXT),
            ),
            (
                vec![
                    json!([0, name, "refused", refusal.label(), refusal.result_text()]),
                    json!([0, "finish", "settled", null, FINISH_DOES_NOT_COUNT_RESULT]),
                    json!([1, "finish", "settled", null, ""]),
                ],
                json!([
                    ["tool_request", "required"],
                    ["tool_request", "required"],
                    ["advisory", "none"]
                ]),
                json!(format!(
                    r#"<tool_result source="{refused_call}" state="rejected">{}</tool_result>"#,
                    refusal.result_text()
                )),
                AgentRunStatus::Completed,
                true,
            ),
            "{message}"
        );
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_last_of_24_run_steps_gives_its_text_as_the_final_answer_and_executes_no_call() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let cases = [
        (["2d7", "2d8"], "SCRIPT:never_finish", json!(LIMIT_ANSWER)),
        (["2d9", "2da"], "SCRIPT:never_finish_quietly", Value::Null),
    ];
    for (prefixes, message, expected_answer) in cases {
        let (scope, _, _, run_id) =
            tool_loop_run(&store, &admin, prefixes, message, None, false).await;
        let steps = steps(&admin, &run_id).await;
        let record = store
            .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
            .await
            .unwrap()
            .unwrap();
        let answer = match record.decision {
            AgentRunDecisionInspect::Advisory { text, .. } => json!(text),
            AgentRunDecisionInspect::Absent => Value::Null,
            other => panic!("{message}: {other:?}"),
        };
        let calls = calls(&admin, &run_id).await;
        let mut expected_choices = vec![json!("required"); 23];
        expected_choices.push(json!("none"));

        // The calls of RunStep 24 (position 23) have no ToolCall.
        assert_eq!(
            (
                steps.iter().map(|step| step[1].clone()).collect::<Vec<_>>(),
                (calls.len(), calls.last().map(|call| call[0].clone())),
                record.status,
                answer,
            ),
            (
                expected_choices,
                (46, Some(json!(22))),
                AgentRunStatus::Completed,
                expected_answer
            ),
            "{message}"
        );
    }
}

/// The fake destination with the `SCRIPT:never_finish` script for each create before the last
/// RunStep. The recovery script of the author message loses the first create of the last
/// RunStep. A `finish` call, with a text unless `quiet`, answers the Retrieve of its reference
/// when `retrieved`, and the successor create.
struct LoseLastCreate {
    retrieved: bool,
    quiet: bool,
}

enum LastStepExchange {
    Fake(<FakeDestination as ModelProviderAdapter>::Prepared),
    Scripted,
}

impl ModelProviderAdapter for LoseLastCreate {
    const ADAPTERS: &'static [storyos_core::ModelAdapter] = FakeDestination::ADAPTERS;
    type Prepared = LastStepExchange;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal> {
        let request = match request {
            DestinationRequest::Create(create) if create.tool_choice != ToolChoice::None => {
                DestinationRequest::Create(CreateRequest {
                    author_message: "SCRIPT:never_finish".to_owned(),
                    ..create.clone()
                })
            }
            other => other.clone(),
        };
        let prepared = FakeDestination.prepare(&request).await?;
        let scripted = match &request {
            DestinationRequest::Retrieve(_) => self.retrieved,
            DestinationRequest::Create(create) => create.successor_of.is_some(),
            DestinationRequest::Abort(_) => false,
        };
        Ok(PreparedRequest {
            prepared: match scripted {
                true => LastStepExchange::Scripted,
                false => LastStepExchange::Fake(prepared.prepared),
            },
            projection: prepared.projection,
        })
    }

    async fn exchange(
        &self,
        prepared: Self::Prepared,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        let LastStepExchange::Fake(prepared) = prepared else {
            let text = NativeStreamItem {
                item_id: "msg_01".to_owned(),
                role: StreamItemRole::Assistant,
                state: StreamItemState::Complete,
                text: Some(LIMIT_ANSWER.to_owned()),
                summary: None,
                call_id: None,
                arguments: None,
                refusal: None,
                native_type: None,
                name: None,
            };
            let call = NativeStreamItem {
                item_id: "fc_01".to_owned(),
                role: StreamItemRole::Tool,
                text: None,
                call_id: Some("call_finish_lost".to_owned()),
                arguments: Some("{}".to_owned()),
                native_type: Some(storyos_core::FUNCTION_CALL_NATIVE_TYPE.to_owned()),
                name: Some("finish".to_owned()),
                ..text.clone()
            };
            return Observation::Terminal(ModelResponse {
                state: StreamItemState::Complete,
                items: match self.quiet {
                    true => vec![call],
                    false => vec![text, call],
                },
                usage: ModelUsage::Unknown,
                cache_use: CacheUse::Unknown,
                response_reference: None,
            });
        };
        FakeDestination.exchange(prepared, sink).await
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_recovered_response_of_the_last_run_step_executes_no_call() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // The author message selects the recovery path. The adapter scripts each response.
    let cases = [
        (["2db", "2dc"], "SCRIPT:retrieve-complete", true, false),
        (["2dd", "2de"], "SCRIPT:successor-once", false, false),
        (["2df", "2e0"], "SCRIPT:retrieve-complete", true, true),
        (["2e1", "2e2"], "SCRIPT:successor-once", false, true),
    ];
    for (prefixes, message, retrieved, quiet) in cases {
        let destination = DeploymentDestination::HostFakeToolLoop;
        let (scope, chapter_id, first) =
            queued_run_offering(&store, prefixes[0], destination.clone(), None).await;
        complete(&store, &admin, &scope, &first).await;
        let run_id = admit_run_in(
            &store,
            &scope,
            &chapter_id,
            prefixes[1],
            destination,
            message,
            ConversationSelection::New,
            None,
        )
        .await;
        let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
        complete_agent_run(
            &store,
            &LoseLastCreate { retrieved, quiet },
            &NoContractFaults,
            &RuntimeClock,
            &claim,
        )
        .await
        .unwrap();
        let calls = calls(&admin, &run_id).await;
        let record = store
            .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
            .await
            .unwrap()
            .unwrap();

        let answer = match record.decision {
            AgentRunDecisionInspect::Advisory { text, .. } => json!(text),
            AgentRunDecisionInspect::Absent => Value::Null,
            other => panic!("{message}: {other:?}"),
        };

        // The `finish` call of the recovered response of RunStep 24 has no ToolCall, and no
        // RunStep 25 opens. A response with no text completes the Run with no result.
        assert_eq!(
            (
                steps(&admin, &run_id).await.len(),
                (calls.len(), calls.last().map(|call| call[0].clone())),
                record.status,
                answer,
            ),
            (
                24,
                (46, Some(json!(22))),
                AgentRunStatus::Completed,
                if quiet {
                    Value::Null
                } else {
                    json!(LIMIT_ANSWER)
                },
            ),
            "{message} {quiet}"
        );
    }
}
