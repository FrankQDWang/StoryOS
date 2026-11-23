//! The records and the requests of a Subrun at the dispatch seam with the fake destination (ADR
//! 0103 section 3A). Test prefix lead `67`.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AgentRunReadSelection, AgentRunReadStore, ClaimedAgentRun, NoContractFaults, ProjectScope,
    complete_agent_run,
};
use storyos_core::{ToolCallState, ToolResultProjection};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_finish_rule::{complete, conversation_of};
use crate::shared_tests::agent_run_steering::queued;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_background_run::claim_in_project;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::memory_entries::write_versions;
use crate::shared_tests::model_gateway::{Admission, RuntimeClock, claim_run};

/// A queued Run of the author of `prefix` whose author message is `message`, in a Project with
/// one current Memory entry at the Memory revision 1. `next` gives the identity of the Run.
pub(super) async fn scripted(
    store: &PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    next: &str,
    message: &str,
) -> (ProjectScope, String) {
    let (scope, chapter_id, first) = queued(store, prefix).await;
    complete(store, admin, &scope, &first).await;
    write_versions(
        admin,
        &scope,
        &[(1, 1, 1, "current", "character", "林舟", "沉默的船夫。")],
    )
    .await;
    let run_id = Admission {
        message,
        ..Default::default()
    }
    .admit(store, &scope, &chapter_id, next)
    .await;
    (scope, run_id)
}

/// A complete Run of the author of `prefix` that reads Chapter 1, so it has a ToolCall.
async fn parent(
    store: &PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    next: &str,
) -> (ProjectScope, String) {
    let (scope, run_id) = scripted(store, admin, prefix, next, "SCRIPT:read_chapter 1").await;
    complete(store, admin, &scope, &run_id).await;
    (scope, run_id)
}

/// Starts a Subrun of the first ToolCall of the Run `parent_run_id` with `brief`, as the work of
/// that call does in the transaction of the parent.
async fn start(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    parent_run_id: &str,
    brief: &str,
) -> String {
    let tool_call_id: String = admin
        .query_one(
            "SELECT tool_call_id::text FROM storyos.tool_calls WHERE run_id = $1::text::uuid
              ORDER BY decision_position, call_position LIMIT 1",
            &[&parent_run_id],
        )
        .await
        .unwrap()
        .get(0);
    let transaction = store
        .begin_serializable_project_command_transaction(scope)
        .await
        .unwrap();
    let parent = ClaimedAgentRun {
        project_scope: scope.clone(),
        run_id: parent_run_id.to_owned(),
        fence_token: 0,
    };
    let subrun = super::start(
        &transaction.client,
        &parent,
        &tool_call_id,
        "Lin Zhou",
        brief,
    )
    .await
    .unwrap();
    transaction.commit().await.unwrap();
    subrun
}

/// For each Model Attempt of the Run in the order of its creation: its RunStep, the purpose of
/// the request, the author message and the source class of each item that the fake destination
/// received, the declared Tools, the `tool_choice`, the prior response reference, and each Tool
/// output with its call identity.
async fn requests(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT jsonb_agg(jsonb_build_array(decision_position, wire->'purpose',
                      wire->'author_message',
                      jsonb_path_query_array(wire, '$.context[*].source_class'), wire->'tools',
                      wire->'tool_choice', wire->'previous_response_reference',
                      wire->'tool_outputs')
                      ORDER BY model_attempt_id)::text
               FROM storyos.model_attempts,
                    LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
              WHERE run_id = $1::text::uuid",
        &[&run_id],
    )
    .await
}

/// The kind and the status of the Run, the function name, the RunStep, and the state of each of
/// its ToolCalls, and its Subrun Request.
pub(super) async fn records(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT jsonb_build_array(run.run_kind, run.status,
                  (SELECT jsonb_agg(jsonb_build_array(call.function_name,
                                                      call.decision_position, call.state)
                                    ORDER BY call.decision_position, call.call_position)
                     FROM storyos.tool_calls AS call WHERE call.run_id = run.run_id),
                  (SELECT jsonb_build_array(request.title, request.brief)
                     FROM storyos.subrun_requests AS request
                    WHERE request.subrun_run_id = run.run_id))::text
               FROM storyos.agent_runs AS run WHERE run.run_id = $1::text::uuid",
        &[&run_id],
    )
    .await
}

/// ADR 0103 section 3A: a Subrun is one Run of the kind `subrun` with its own RunSteps and
/// ToolCalls. Each of its requests has the purpose of a Subrun, the brief only as its Context
/// item, the Chapter list, and the Memory Index of the pin of its parent Run, declares only the
/// read Tools and `finish`, and follows the finish rule: its closing request has
/// `tool_choice: "none"`. Each later request sends its full input with no prior response
/// reference, and the projection of each stored Tool result under the call identity of its
/// stored ToolCall.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_subrun_runs_to_its_end_with_its_own_context_tools_and_records() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, parent_run_id) = parent(&store, &admin, "670", "671").await;
    let subrun = start(
        &store,
        &admin,
        &scope,
        &parent_run_id,
        "SCRIPT:read_chapter 1",
    )
    .await;
    let claim = claim_run(&admin, scope.clone(), subrun.clone()).await;
    complete_agent_run(
        &store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();
    // The output of each settled ToolCall of the Subrun, projected from its stored result.
    let outputs: Vec<Value> = (admin.query(
        "SELECT call_id, result_text FROM storyos.tool_calls WHERE run_id = $1::text::uuid
          ORDER BY decision_position, call_position",
        &[&subrun],
    ))
    .await
    .unwrap()
    .iter()
    .map(|row| {
        let call_id: String = row.get(0);
        let text: Option<String> = row.get(1);
        let projection = ToolResultProjection::of(ToolCallState::Settled, text.as_deref());
        json!({"call_id": call_id, "output": projection.unwrap().continuation_text(&call_id)})
    })
    .collect();
    let call_ids: Vec<&Value> = outputs.iter().map(|output| &output["call_id"]).collect();
    let items = json!(["subrun_brief", "chapter_list", "memory_index"]);
    let tools = json!([
        "read_chapter",
        "search_manuscript",
        "read_memory_entry",
        "search_memory",
        "finish"
    ]);

    assert_eq!(
        (
            requests(&admin, &subrun).await,
            records(&admin, &subrun).await,
            call_ids,
        ),
        (
            json!([
                [0, "subrun", "", items, tools, "required", null, []],
                [
                    1,
                    "subrun",
                    "",
                    items,
                    tools,
                    "required",
                    null,
                    [outputs[0]]
                ],
                [2, "subrun", "", items, tools, "none", null, outputs],
            ]),
            json!([
                "subrun",
                "completed",
                [["read_chapter", 0, "settled"], ["finish", 1, "settled"]],
                ["Lin Zhou", "SCRIPT:read_chapter 1"]
            ]),
            // The call identities of the replies of the fake destination.
            vec![&json!("call_read_chapter_0"), &json!("call_finish")],
        )
    );
}

/// The final answer text of the Run `run_id`, or `None`.
pub(super) async fn final_answer(admin: &Client, run_id: &str) -> Option<String> {
    admin
        .query_one(
            "SELECT (SELECT attempt.payload #>> '{decision,text}'
                       FROM storyos.model_attempts AS attempt
                      WHERE attempt.run_id = run.run_id
                        AND attempt.attempt_role IN ('decision', 'successor')
                        AND attempt.decision_position = run.active_decision_position
                        AND attempt.payload #>> '{decision,kind}' = 'advisory'
                        AND attempt.payload #> '{decision,selected}' = 'true'::jsonb
                      LIMIT 1)
               FROM storyos.agent_runs AS run WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(0)
}

/// The Subruns of the Run `parent_run_id`, in the order of their Subrun Requests.
pub(super) async fn subruns(admin: &Client, parent_run_id: &str) -> Vec<String> {
    (admin.query(
        "SELECT subrun_run_id::text FROM storyos.subrun_requests
          WHERE parent_run_id = $1::text::uuid ORDER BY subrun_run_id",
        &[&parent_run_id],
    ))
    .await
    .unwrap()
    .iter()
    .map(|row| row.get(0))
    .collect()
}

/// The state, the refusal, and the result text of each `start_subrun` call of the Run.
pub(super) async fn subrun_calls(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT COALESCE(jsonb_agg(jsonb_build_array(state, refusal, result_text)
                                   ORDER BY decision_position, call_position), '[]')::text
           FROM storyos.tool_calls
          WHERE run_id = $1::text::uuid AND function_name = 'start_subrun'",
        &[&run_id],
    )
    .await
}

/// The Contract Fault Point observer that keeps, at each dispatch claim, the status and the count
/// of Model Attempts of the parent Run, the state of its `start_subrun` call, and the status of
/// its Subrun.
struct ParentAtDispatch<'a> {
    admin: &'a Client,
    run_id: String,
    seen: tokio::sync::Mutex<Vec<Value>>,
}

impl storyos_application::ContractFaultObserver for ParentAtDispatch<'_> {
    async fn reached(&self, point: storyos_application::ContractFaultPoint) {
        if point != storyos_application::ContractFaultPoint::DispatchClaimed {
            return;
        }
        let seen = json_of(
            self.admin,
            "SELECT jsonb_build_array(run.status,
                      (SELECT count(*) FROM storyos.model_attempts AS attempt
                        WHERE attempt.run_id = run.run_id),
                      (SELECT call.state FROM storyos.tool_calls AS call
                        WHERE call.run_id = run.run_id AND call.function_name = 'start_subrun'),
                      (SELECT subrun.status FROM storyos.subrun_requests AS request
                         JOIN storyos.agent_runs AS subrun ON subrun.run_id = request.subrun_run_id
                        WHERE request.parent_run_id = run.run_id))::text
               FROM storyos.agent_runs AS run WHERE run.run_id = $1::text::uuid",
            &[&self.run_id],
        )
        .await;
        self.seen.lock().await.push(seen);
    }
}

/// Item 11 of #436: the scripted message `SCRIPT:start_subrun` starts one Subrun that reads the
/// Current Chapter. The ToolCall of the parent settles with the final answer of the Subrun, and
/// the final answer of the parent contains it. The Subrun is one Run of the kind `subrun` with its own
/// RunSteps and ToolCalls, and one Subrun Request names the two. While the Subrun works, the
/// parent is `claimed`, has no new Model Attempt, and its call is `pending`.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_scripted_message_runs_a_subrun_and_settles_the_call_with_its_answer() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let script = storyos_adapter_fake_destination::START_SUBRUN_SCRIPT;
    let (scope, run_id) = scripted(&store, &admin, "672", "673", script).await;
    let observer = ParentAtDispatch {
        admin: &admin,
        run_id: run_id.clone(),
        seen: tokio::sync::Mutex::default(),
    };
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    complete_agent_run(&store, &FakeDestination, &observer, &RuntimeClock, &claim)
        .await
        .unwrap();
    let subrun = subruns(&admin, &run_id).await;
    // The fake destination answers the brief of the Subrun with its advice text, and the parent
    // answers with its prefix and the tagged result of its call.
    let answer = storyos_core::ADVISORY_TEXT;
    let parent_answer = format!(
        "{}\n{}",
        storyos_adapter_fake_destination::SUBRUN_ANSWER_PREFIX,
        ToolResultProjection::Ok(answer).continuation_text("call_start_subrun")
    );

    assert_eq!(
        (
            records(&admin, &run_id).await,
            records(&admin, &subrun[0]).await,
            final_answer(&admin, &subrun[0]).await,
            subrun_calls(&admin, &run_id).await,
            final_answer(&admin, &run_id).await,
            observer.seen.into_inner(),
        ),
        (
            json!([
                "author",
                "completed",
                [["start_subrun", 0, "settled"], ["finish", 1, "settled"]],
                null
            ]),
            json!([
                "subrun",
                "completed",
                [["read_chapter", 0, "settled"], ["finish", 1, "settled"]],
                ["读当前章", "SCRIPT:read_chapter 1"]
            ]),
            Some(answer.to_owned()),
            json!([["settled", null, answer]]),
            Some(parent_answer),
            vec![
                json!(["claimed", 1, null, null]),
                json!(["claimed", 1, "pending", "claimed"]),
                json!(["claimed", 1, "pending", "claimed"]),
                json!(["claimed", 1, "pending", "claimed"]),
                json!(["claimed", 2, "settled", "completed"]),
                json!(["claimed", 3, "settled", "completed"]),
            ],
        )
    );
}

/// Item 2 of #436: a call that breaks a rule is refused with the sentence of the first rule, and
/// it has no Subrun and no Subrun Request. Of five calls in one response, four start a Subrun
/// each, and the fifth is refused by the limit.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_call_that_breaks_a_rule_is_refused_with_no_subrun() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let title = r#"SCRIPT:refuse start_subrun {"title":" ","brief":"读第一章"}"#;
    let (scope, titled) = scripted(&store, &admin, "674", "675", title).await;
    complete(&store, &admin, &scope, &titled).await;
    let (scope, five) = scripted(&store, &admin, "676", "677", "SCRIPT:start_subrun 5").await;
    complete(&store, &admin, &scope, &five).await;
    let subrun = subruns(&admin, &five).await;
    // The fake destination answers the brief of each Subrun with its advice text.
    let answer = storyos_core::ADVISORY_TEXT;
    let limit = "One author request can start at most 4 Subruns.";

    assert_eq!(
        (
            subrun_calls(&admin, &titled).await,
            subruns(&admin, &titled).await.len(),
            subrun_calls(&admin, &five).await,
            subrun.len(),
        ),
        (
            json!([[
                "refused",
                "subrun_title",
                "The title must have 1 through 40 characters on one line."
            ]]),
            0,
            json!([
                ["settled", null, answer],
                ["settled", null, answer],
                ["settled", null, answer],
                ["settled", null, answer],
                ["refused", "subrun_request_count", limit]
            ]),
            4,
        )
    );
}

/// Item 8 of #436: a Subrun that ends with no final answer text settles the call with the
/// sentence of no result, and the parent continues to its end.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_subrun_with_no_final_answer_settles_the_call_with_no_result() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let message =
        r#"SCRIPT:refuse start_subrun {"title":"读","brief":"SCRIPT:empty_final_answer"}"#;
    let (scope, run_id) = scripted(&store, &admin, "678", "679", message).await;
    complete(&store, &admin, &scope, &run_id).await;
    let subrun = subruns(&admin, &run_id).await;

    assert_eq!(
        (
            subrun_calls(&admin, &run_id).await,
            records(&admin, &subrun[0]).await[1].clone(),
            records(&admin, &run_id).await[1].clone(),
        ),
        (
            json!([["settled", null, "The Subrun ended with no result."]]),
            json!("completed"),
            json!("completed"),
        )
    );
}

/// The Contract Fault Point observer that, at the first dispatch claim of the Subrun, stops the
/// Subrun as the recovery of a Model Attempt does: a new fence, the status `status`, and the
/// settlement `settlement`.
struct StopSubrun<'a> {
    admin: &'a Client,
    status: &'static str,
    settlement: Option<Value>,
    reached: std::sync::Mutex<usize>,
}

impl storyos_application::ContractFaultObserver for StopSubrun<'_> {
    async fn reached(&self, point: storyos_application::ContractFaultPoint) {
        if point != storyos_application::ContractFaultPoint::DispatchClaimed {
            return;
        }
        let count = {
            let mut reached = self.reached.lock().unwrap();
            *reached += 1;
            *reached
        };
        if count != 2 {
            return;
        }
        let settlement = self.settlement.as_ref().map(ToString::to_string);
        self.admin
            .execute(
                "UPDATE storyos.agent_runs
                    SET fence_token = fence_token + 1, status = $1::text,
                        settlement = $2::text::jsonb
                  WHERE run_kind = 'subrun' AND status = 'claimed'",
                &[&self.status, &settlement],
            )
            .await
            .unwrap();
    }
}

/// Runs a parent Run of `prefixes` with the scripted message while the observer stops its Subrun
/// with `status` and `settlement`, then completes the parent. Gives whether the first claim of the
/// parent stopped with a stale fence, the `start_subrun` calls, the status of the parent, and the
/// count of its Subruns.
pub(super) async fn stopped_end(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    status: &'static str,
    settlement: Option<Value>,
) -> Value {
    let script = storyos_adapter_fake_destination::START_SUBRUN_SCRIPT;
    let (scope, run_id) = scripted(store, admin, prefixes[0], prefixes[1], script).await;
    let observer = StopSubrun {
        admin,
        status,
        settlement,
        reached: std::sync::Mutex::default(),
    };
    let claim = claim_run(admin, scope.clone(), run_id.clone()).await;
    let stopped =
        complete_agent_run(store, &FakeDestination, &observer, &RuntimeClock, &claim).await;
    complete(store, admin, &scope, &run_id).await;
    json!([
        matches!(
            stopped,
            Err(storyos_application::CompleteAgentRunError::StaleFence)
        ),
        subrun_calls(admin, &run_id).await,
        records(admin, &run_id).await[1],
        subruns(admin, &run_id).await.len(),
    ])
}

/// Item 8 of #436: a Subrun refused because its request was too long settles the call of the
/// parent with the sentence of that cause. The parent stops with its claim, and its next claim
/// settles the call and continues to its end.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_subrun_refused_as_too_long_settles_the_call_with_the_sentence_of_that_cause() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let too_long = json!({"kind": "execution_refused", "capability": "conversation_too_long"});
    let end = stopped_end(&store, &admin, ["67e", "67f"], "refused", Some(too_long)).await;
    let sentence = "The Subrun ended with no result, because its reading was too long for one \
                    Subrun. Give a smaller task.";

    assert_eq!(
        end,
        json!([true, [["settled", null, sentence]], "completed", 1])
    );
}

/// Items 3 and 7 of #436: the Worker never selects a queued Subrun, or a claimed Subrun whose
/// lease expired, also when no other work exists. `getAgentRun` with the identity of a Subrun
/// answers as for a Run that does not exist, the earlier turns of the Conversation of the parent
/// do not read the Subrun, and the Subrun gets no Memory work. Test prefix lead `75`.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn no_worker_claim_no_run_read_and_no_earlier_turn_gives_a_subrun() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, run_id) = parent(&store, &admin, "750", "751").await;
    let queued_subrun = start(&store, &admin, &scope, &run_id, "读第一章").await;
    let queued_claim = claim_in_project(&store, &scope).await;
    admin
        .execute(
            "UPDATE storyos.agent_runs
                SET status = 'claimed', claim_generation = 1, fence_token = 1,
                    lease_expires_at = clock_timestamp() - interval '1 second'
              WHERE run_id = $1::text::uuid",
            &[&queued_subrun],
        )
        .await
        .unwrap();
    let expired_claim = claim_in_project(&store, &scope).await;
    let script = storyos_adapter_fake_destination::START_SUBRUN_SCRIPT;
    let (scope, parent_run_id) = scripted(&store, &admin, "752", "753", script).await;
    complete(&store, &admin, &scope, &parent_run_id).await;
    let subrun = subruns(&admin, &parent_run_id).await.remove(0);
    let read = |run_id: String| {
        let (store, scope) = (&store, &scope);
        async move {
            (store.read_agent_run(scope, &run_id, &AgentRunReadSelection::Current))
                .await
                .unwrap()
                .is_some()
        }
    };
    let parent = admin
        .query_one(
            "SELECT chapter_id::text, conversation_id::text FROM storyos.agent_runs
              WHERE run_id = $1::text::uuid",
            &[&parent_run_id],
        )
        .await
        .unwrap();
    let (chapter_id, conversation): (String, String) = (parent.get(0), parent.get(1));
    let next = Admission {
        conversation: conversation_of("753"),
        ..Default::default()
    }
    .admit(&store, &scope, &chapter_id, "754")
    .await;
    let turns = crate::create_agent_run::earlier_turns::load(&admin, &scope, &next)
        .await
        .unwrap();
    // The Memory work of the Conversation of the parent, and each Memory work with no
    // Conversation: the kind of each schedule row.
    let memory_work = json_of(
        &admin,
        "SELECT jsonb_agg(work_kind)::text FROM storyos.memory_work_schedules
          WHERE project_id = $1::text::uuid
            AND (conversation_id = $2::text::uuid OR conversation_id IS NULL)",
        &[&scope.project_id.as_ref(), &conversation],
    )
    .await;

    assert_eq!(
        (
            queued_claim.is_none(),
            expired_claim.is_none(),
            read(subrun.clone()).await,
            read(parent_run_id.clone()).await,
            (turns.iter())
                .map(|turn| turn.messages[0].source.run_id.clone())
                .collect::<Vec<_>>(),
            memory_work,
        ),
        (
            true,
            true,
            false,
            true,
            vec![parent_run_id],
            // Only the capture of the Conversation of the parent: the Subrun has no Memory work.
            json!(["conversation_capture"]),
        )
    );
}
