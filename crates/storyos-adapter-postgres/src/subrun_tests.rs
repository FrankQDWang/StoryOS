//! The records and the requests of a Subrun at the dispatch seam with the fake destination (ADR
//! 0103 section 3A). Test prefix lead `67`, and `766` through `769` and `76a` through `76d`.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    CancelAgentRunInput, ClaimedAgentRun, CompleteAgentRunError, ContractFaultObserver,
    ContractFaultPoint, NoContractFaults, ProjectScope, complete_agent_run,
};
use storyos_core::{DeploymentDestination, ToolCallState, ToolResultProjection};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_finish_rule::{complete, conversation_of};
use crate::shared_tests::agent_run_retrieval::{
    AfterExchange, adapter, broken_stream, destination, stored,
};
use crate::shared_tests::agent_run_steering::{cancellation, queued};
use crate::shared_tests::agent_run_tool_exposure::{finish_stream, finish_then_advisory, sse};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_background_run::claim_in_project;
use crate::shared_tests::command_sequence_support::{CommandCall, stores};
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::memory_entries::write_versions;
use crate::shared_tests::model_gateway::{Admission, RuntimeClock, claim_run, queued_run};
use crate::shared_tests::project_instruction::{applied, save};

/// The Character Intent of the entry 2 of the Project of `scripted`.
const INTENT: &str = "我明天一早去渡口。";

/// A queued Run of the author of `prefix` whose author message is `message`, in a Project with a
/// Project Instruction, one current Memory entry of the author, and one character entry of the
/// background work with a current Character Intent at the Memory revision 2. `next` gives the
/// identity of the Run.
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
    let project = scope.project_id.as_ref();
    admin
        .batch_execute(&format!(
            "INSERT INTO storyos.memory_entry_versions
                 (owner_user_id, project_id, memory_entry_id, entry_number, version,
                  memory_revision, state, entry_type, origin, title, description, text, cause,
                  source_references, intent, intent_questions)
             SELECT owner_user_id, project_id, md5('{project}:2')::uuid, 2, version, version,
                    'current', 'character', 'chapter', '沈月', '渡口的人。', '渡口的人。',
                    cause::jsonb, '[]', intent, questions::jsonb
               FROM storyos.project_memory_heads,
                    (VALUES (1, '{{}}', NULL, NULL),
                            (2, '{{\"kind\": \"intent_pass\"}}', '{INTENT}', '[]'))
                      AS row(version, cause, intent, questions)
              WHERE project_id = '{project}'::uuid;
             UPDATE storyos.project_memory_heads SET memory_revision = 2, next_entry_number = 3
              WHERE project_id = '{project}'::uuid"
        ))
        .await
        .unwrap();
    let suffix = u16::from_str_radix(&format!("{prefix}f"), 16).unwrap();
    applied(&save(store, &scope, suffix, "用第三人称。", 0).await);
    let run_id = Admission {
        message,
        ..Default::default()
    }
    .admit(store, &scope, &chapter_id, next)
    .await;
    (scope, run_id)
}

/// A complete Run of the author of `prefix` that reads Chapter 1, so it has a ToolCall.
pub(super) async fn parent(
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
/// that call does in the transaction of the parent. The parent then has a claim with its lease,
/// so the claim of the Subrun is valid.
pub(super) async fn start(
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
    admin
        .execute(
            "UPDATE storyos.agent_runs
                SET status = 'claimed', lease_expires_at = clock_timestamp() + interval '1 hour'
              WHERE run_id = $1::text::uuid",
            &[&parent_run_id],
        )
        .await
        .unwrap();
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

/// Whether the item of the source class `class` of the first RunStep of the Run has `text`.
async fn item_has(admin: &Client, run_id: &str, class: &str, text: &str) -> Value {
    json_of(
        admin,
        "SELECT to_jsonb(strpos(item ->> 'content', $2) > 0)::text
           FROM storyos.context_assembly_manifests AS manifest,
                jsonb_array_elements(manifest.payload -> 'selected') AS item
          WHERE manifest.run_id = $1::text::uuid AND manifest.manifest_role = 'decision'
            AND manifest.decision_position = 0 AND item ->> 'source_class' = $3",
        &[&run_id, &text, &class],
    )
    .await
}

/// ADR 0103 section 3A, item 5 of #436: a Subrun is one Run of the kind `subrun` with its own
/// RunSteps and ToolCalls. Each of its requests has the purpose of a Subrun, the Project
/// Instruction of its parent Run, the brief, the Chapter list, and the Memory Index of the pin of
/// its parent Run with no Character Intent, declares only the read Tools and `finish`, and
/// follows the finish rule: its closing request has `tool_choice: "none"`. Each later request
/// sends its full input with no prior response reference, and the projection of each stored Tool
/// result under the call identity of its stored ToolCall. A Subrun whose parent has no Memory use
/// has no Memory Index and no Memory Tool.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_subrun_runs_to_its_end_with_its_own_context_tools_and_records() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, parent_run_id) = parent(&store, &admin, "670", "671").await;
    // A later Memory revision and a later Project Instruction do not change the Subrun.
    write_versions(
        &admin,
        &scope,
        &[(1, 2, 3, "current", "character", "林舟", "年老的船夫。")],
    )
    .await;
    applied(&save(&store, &scope, 0x67e0, "用第一人称。", 1).await);
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
    let items = json!([
        "instruction_binding",
        "subrun_brief",
        "chapter_list",
        "memory_index"
    ]);
    let tools = json!([
        "read_chapter",
        "search_manuscript",
        "read_memory_entry",
        "search_memory",
        "finish"
    ]);

    // A parent with no Memory use: its Conversation, which has no working Run, gets settings
    // with `use_enabled = false`.
    admin
        .batch_execute(&format!(
            "UPDATE storyos.agent_runs SET status = 'completed'
              WHERE run_id = '{parent_run_id}'::uuid;
             UPDATE storyos.conversation_memory_settings SET is_current = FALSE
              WHERE project_id = '{project}'::uuid;
             INSERT INTO storyos.conversation_memory_settings
             SELECT (jsonb_populate_record(NULL::storyos.conversation_memory_settings,
               (to_jsonb(settings) - 'is_current') || jsonb_build_object(
                 'memory_settings_revision', gen_random_uuid(), 'use_enabled', false))).*
               FROM storyos.conversation_memory_settings AS settings
              WHERE project_id = '{project}'::uuid",
            project = scope.project_id.as_ref()
        ))
        .await
        .unwrap();
    let chapter_id: String = (admin.query_one(
        "SELECT chapter_id::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
        &[&parent_run_id],
    ))
    .await
    .unwrap()
    .get(0);
    let no_memory = Admission {
        message: "SCRIPT:read_chapter 1",
        conversation: conversation_of("671"),
        ..Default::default()
    }
    .admit(&store, &scope, &chapter_id, "768")
    .await;
    complete(&store, &admin, &scope, &no_memory).await;
    let no_memory = start(&store, &admin, &scope, &no_memory, "读第一章").await;
    let claim = claim_run(&admin, scope.clone(), no_memory.clone()).await;
    (complete_agent_run(
        &store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    ))
    .await
    .unwrap();

    assert_eq!(
        (
            item_has(&admin, &parent_run_id, "memory_index", INTENT).await,
            item_has(&admin, &subrun, "memory_index", INTENT).await,
            item_has(&admin, &subrun, "memory_index", "沈月").await,
            item_has(&admin, &subrun, "memory_index", "沉默的船夫。").await,
            item_has(&admin, &subrun, "instruction_binding", "用第三人称。").await,
            requests(&admin, &no_memory).await[0].clone(),
        ),
        (
            json!(true),
            json!(false),
            json!(true),
            json!(true),
            json!(true),
            json!([
                0,
                "subrun",
                "",
                ["instruction_binding", "subrun_brief", "chapter_list"],
                ["read_chapter", "search_manuscript", "finish"],
                "required",
                null,
                []
            ]),
        )
    );
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

/// Item 2 of #436: a call with a bad title or a bad brief is refused with the sentence of
/// that rule, and it has no Subrun and no Subrun Request. Of five calls in one response, four start a Subrun
/// each, and the fifth is refused by the limit.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_call_that_breaks_a_rule_is_refused_with_no_subrun() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let title = r#"SCRIPT:refuse start_subrun {"title":" ","brief":"读第一章"}"#;
    let (scope, titled) = scripted(&store, &admin, "674", "675", title).await;
    complete(&store, &admin, &scope, &titled).await;
    let brief = r#"SCRIPT:refuse start_subrun {"title":"读","brief":" "}"#;
    let (scope, briefed) = scripted(&store, &admin, "766", "767", brief).await;
    complete(&store, &admin, &scope, &briefed).await;
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
            subrun_calls(&admin, &briefed).await,
            subruns(&admin, &briefed).await.len(),
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
            json!([[
                "refused",
                "subrun_brief",
                "The brief must have 1 through 4000 characters."
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

/// The Contract Fault Point observer that, at the `nth` dispatch claim, makes the lease of the
/// parent Run expire, and lets the Worker take the parent over with a new claim.
struct AtDispatch<'a> {
    store: &'a PostgresProjectReader,
    admin: &'a Client,
    scope: ProjectScope,
    run_id: String,
    nth: usize,
    reached: std::sync::Mutex<usize>,
    takeover: std::sync::Mutex<Option<ClaimedAgentRun>>,
}

impl storyos_application::ContractFaultObserver for AtDispatch<'_> {
    async fn reached(&self, point: storyos_application::ContractFaultPoint) {
        if point != storyos_application::ContractFaultPoint::DispatchClaimed {
            return;
        }
        let count = {
            let mut reached = self.reached.lock().unwrap();
            *reached += 1;
            *reached
        };
        if count != self.nth {
            return;
        }
        let expire = "UPDATE storyos.agent_runs
                         SET lease_expires_at = clock_timestamp() - interval '1 second'
                       WHERE run_id = $1::text::uuid";
        self.admin.execute(expire, &[&self.run_id]).await.unwrap();
        let takeover = claim_in_project(self.store, &self.scope).await.unwrap();
        assert_eq!(takeover.run_id, self.run_id);
        *self.takeover.lock().unwrap() = Some(takeover);
    }
}

/// Runs a parent Run of `prefixes` with the author message `script` while the Worker takes the
/// parent over at the `nth` dispatch claim, then completes the parent with the claim of that
/// takeover.
/// Gives whether the first claim of the parent stopped with a stale fence. Then it gives the
/// status and the decision Model Attempt count of each Subrun at that stop, in the call order.
/// Last, it gives whether the completion kept the same Subruns, the calls, and the parent status.
pub(super) async fn stopped_end(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    script: &str,
    nth: usize,
) -> Value {
    let (scope, run_id) = scripted(store, admin, prefixes[0], prefixes[1], script).await;
    let observer = AtDispatch {
        store,
        admin,
        scope: scope.clone(),
        run_id: run_id.clone(),
        nth,
        reached: std::sync::Mutex::default(),
        takeover: std::sync::Mutex::default(),
    };
    let claim = claim_run(admin, scope.clone(), run_id.clone()).await;
    let stopped = matches!(
        complete_agent_run(store, &FakeDestination, &observer, &RuntimeClock, &claim).await,
        Err(storyos_application::CompleteAgentRunError::StaleFence)
    );
    let (before, states) = (
        subruns(admin, &run_id).await,
        subrun_states(admin, &run_id).await,
    );
    let takeover = observer.takeover.into_inner().unwrap();
    if let Some(takeover) = &takeover {
        let end = complete_agent_run(
            store,
            &FakeDestination,
            &NoContractFaults,
            &RuntimeClock,
            takeover,
        );
        Box::pin(end).await.unwrap();
    }
    json!([
        stopped,
        states,
        subruns(admin, &run_id).await == before,
        subrun_calls(admin, &run_id).await,
        records(admin, &run_id).await[1],
    ])
}

/// The status and the count of the decision Model Attempts of each Subrun of the Run, in the
/// order of the calls.
async fn subrun_states(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT COALESCE(jsonb_agg(jsonb_build_array(run.status,
                  (SELECT count(*) FROM storyos.model_attempts AS attempt
                    WHERE attempt.run_id = run.run_id AND attempt.attempt_role = 'decision'))
                  ORDER BY call.decision_position, call.call_position), '[]')::text
           FROM storyos.subrun_requests AS request
           JOIN storyos.tool_calls AS call ON call.tool_call_id = request.parent_tool_call_id
           JOIN storyos.agent_runs AS run ON run.run_id = request.subrun_run_id
          WHERE request.parent_run_id = $1::text::uuid",
        &[&run_id],
    )
    .await
}

/// A parent Run of `lead` on the Agent Plan Registration with a scripted model. The model calls
/// `start_subrun`, serves `subrun` to the Subrun, and then ends the parent with `finish` and its
/// answer. Gives the scope, the Run, and the request lines that the model served.
pub(super) async fn plan_parent(
    store: &PostgresProjectReader,
    admin: &Client,
    lead: &str,
    subrun: Vec<String>,
) -> (ProjectScope, String, tokio::task::JoinHandle<Vec<String>>) {
    let call = json!({"id": "fc_01", "type": "function_call", "status": "completed",
        "call_id": "call_start_subrun", "name": "start_subrun",
        "arguments": r#"{"title":"读","brief":"读第一章。"}"#});
    let parent_call = sse(&[
        json!({"type": "response.created",
               "response": {"id": format!("resp_{lead}"), "status": "in_progress", "output": []}}),
        json!({"type": "response.output_item.done", "output_index": 0, "item": call}),
        json!({"type": "response.completed", "response": {"id": format!("resp_{lead}"),
               "status": "completed", "output": [call],
               "usage": {"input_tokens": 40, "output_tokens": 9}}}),
    ]);
    let mut replies = vec![parent_call];
    replies.extend(subrun);
    replies.extend(finish_then_advisory(&format!("resp_{lead}_end")));
    let (base, served) = destination(replies).await;
    let plan = DeploymentDestination::VolcengineAgentPlan {
        qualification_authorization: None,
    };
    let (scope, _, run_id) = queued_run(store, lead, plan).await;
    admin
        .execute(
            "UPDATE storyos.processing_destination_identity_evidence_revisions SET endpoint = $2
              WHERE project_id = $1::text::uuid
                AND evidence_kind = 'volcengine_agent_plan_boundary'",
            &[&scope.project_id.as_ref(), &base],
        )
        .await
        .unwrap();
    (scope, run_id, served)
}

/// Item 8 of #436, ADR 0056: on the Agent Plan Registration, a Subrun whose request is too long
/// for the window is refused by the check before its dispatch, and the call of the parent
/// settles with the sentence of that cause. After the first exchange of the Subrun, the adapter
/// makes the first item of the Subrun fill the window after the output bound (1,015,808 tokens),
/// so the check refuses its closing request. The model serves four responses: the call of the
/// parent, the `finish` call of the Subrun, and the end of the parent.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_subrun_too_long_for_the_window_of_agent_plan_is_refused_before_its_dispatch() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let subrun = vec![finish_stream("resp_769_subrun")];
    let (scope, run_id, served) = plan_parent(&store, &admin, "769", subrun).await;
    let fill = AfterExchange {
        admin: &admin,
        statement: format!(
            "UPDATE storyos.context_assembly_manifests
                SET payload = jsonb_set(payload, '{{selected,0,token_count}}', '1015808')
              WHERE run_id IN (SELECT subrun_run_id FROM storyos.subrun_requests
                                WHERE parent_run_id = '{run_id}')"
        ),
    };
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    complete_agent_run(&store, &fill, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let subrun = subruns(&admin, &run_id).await.remove(0);
    let sentence = "The Subrun ended with no result, because its reading was too long for one \
                    Subrun. Give a smaller task.";

    assert_eq!(
        (
            subrun_calls(&admin, &run_id).await,
            records(&admin, &run_id).await[1].clone(),
            json_of(
                &admin,
                "SELECT jsonb_build_array(status, settlement->>'capability')::text
                   FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
                &[&subrun],
            )
            .await,
            served.await.unwrap().len(),
        ),
        (
            json!([["settled", null, sentence]]),
            json!("completed"),
            json!(["refused", "conversation_too_long"]),
            4,
        )
    );
}

/// Item 7 of #436, ADR 0088 section 3: the recovery of the first Model Attempt of a Subrun stops
/// the Subrun as `paused` or as `waiting`, and the call of the parent settles with the sentence of
/// no result (criterion 3). The parent then continues to its end. For `paused`, the stream of the
/// Subrun commits a function call item and breaks, so its successor has an unresolved effect. For
/// `waiting`, the stream of the successor also breaks, so the allowance is spent with an unknown
/// outcome. Test prefix lead `be`.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_subrun_that_the_recovery_stops_settles_the_call_with_no_result() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let call = json!({"id": "fc_read", "type": "function_call", "status": "completed",
        "call_id": "call_read", "name": "read_chapter", "arguments": r#"{"chapter":1,"first_block":null,"last_block":null}"#});
    let committed_call = sse(&[
        json!({"type": "response.created",
               "response": {"id": "resp_be1_subrun", "status": "in_progress", "output": []}}),
        json!({"type": "response.output_item.done", "output_index": 0, "item": call}),
    ]);
    let cases = [
        (
            "be1",
            vec![committed_call, stored("resp_be1_subrun", "in_progress")],
        ),
        (
            "be2",
            vec![
                broken_stream("resp_be2_subrun"),
                stored("resp_be2_subrun", "in_progress"),
                broken_stream("resp_be2_successor"),
                stored("resp_be2_subrun", "in_progress"),
            ],
        ),
    ];
    let mut ends = Vec::new();
    for (lead, subrun) in cases {
        let (scope, run_id, served) = plan_parent(&store, &admin, lead, subrun).await;
        let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
        (complete_agent_run(&store, &adapter(), &NoContractFaults, &RuntimeClock, &claim))
            .await
            .unwrap();
        let subrun = subruns(&admin, &run_id).await.remove(0);
        let reason = json_of(
            &admin,
            "SELECT to_jsonb(settlement->>'reason')::text FROM storyos.agent_runs
              WHERE run_id = $1::text::uuid",
            &[&subrun],
        )
        .await;
        ends.push(json!([
            records(&admin, &subrun).await[1],
            reason,
            subrun_calls(&admin, &run_id).await,
            records(&admin, &run_id).await[1],
            served.await.unwrap().len(),
        ]));
    }
    let no_result = json!([["settled", null, "The Subrun ended with no result."]]);

    assert_eq!(
        ends,
        [
            json!(["paused", "unresolved_effect", no_result, "completed", 5]),
            json!(["waiting", "unknown_result", no_result, "completed", 7]),
        ]
    );
}

/// The Contract Fault Point observer that cancels the parent Run of `cancel` at the first
/// `point` of a Subrun. With no `point`, the observer of the parent cancels it.
struct CancelAt<'a> {
    store: &'a PostgresProjectReader,
    cancel: CommandCall<CancelAgentRunInput>,
    point: Option<ContractFaultPoint>,
    done: std::sync::atomic::AtomicBool,
}

impl ContractFaultObserver for CancelAt<'_> {
    async fn reached(&self, point: ContractFaultPoint) {
        if Some(point) != self.point || self.done.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let (envelope, input) = (&self.cancel.envelope, &self.cancel.input);
        self.store.cancel_agent_run(envelope, input).await.unwrap();
    }
}

/// The Contract Fault Point observer of the parent Run. At its second dispatch claim, it starts a
/// Subrun of its first ToolCall. With no `point`, it cancels the parent while the Subrun is
/// queued. With a `point`, it claims and runs the Subrun, which cancels the parent at that point.
struct ParentWithSubrun<'a> {
    store: &'a PostgresProjectReader,
    admin: &'a Client,
    scope: ProjectScope,
    run_id: String,
    cancel: std::sync::Mutex<Option<CancelAt<'a>>>,
    subrun: tokio::sync::Mutex<Option<String>>,
    reached: std::sync::Mutex<usize>,
}

impl ContractFaultObserver for ParentWithSubrun<'_> {
    async fn reached(&self, point: ContractFaultPoint) {
        if point != ContractFaultPoint::DispatchClaimed {
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
        let (store, admin, scope) = (self.store, self.admin, &self.scope);
        let subrun = start(store, admin, scope, &self.run_id, "SCRIPT:read_chapter 1").await;
        *self.subrun.lock().await = Some(subrun.clone());
        let observer = self.cancel.lock().unwrap().take().unwrap();
        if observer.point.is_none() {
            let (envelope, input) = (&observer.cancel.envelope, &observer.cancel.input);
            store.cancel_agent_run(envelope, input).await.unwrap();
            return;
        }
        let claim = claim_run(admin, scope.clone(), subrun).await;
        // The nested run of the Subrun is a large future, so it goes on the heap.
        let stopped = complete_agent_run(store, &FakeDestination, &observer, &RuntimeClock, &claim);
        assert!(matches!(
            Box::pin(stopped).await,
            Err(CompleteAgentRunError::StaleFence)
        ));
    }
}

/// Admits a parent Run of `next` that reads Chapter 1, starts its Subrun, and cancels the parent
/// at `point` of the Subrun, as `ParentWithSubrun` gives. Gives one row for the parent and one
/// row for the Subrun. Each row has the status, the wake-up, and whether the lease is clear. It
/// also has the state and the refusal of each ToolCall of the Run.
async fn cancelled_with_subrun(
    store: &PostgresProjectReader,
    admin: &Client,
    (scope, chapter_id): (&ProjectScope, &str),
    (next, suffix): (&str, u16),
    point: Option<ContractFaultPoint>,
) -> Value {
    let run_id = Admission {
        message: "SCRIPT:read_chapter 1",
        ..Default::default()
    }
    .admit(store, scope, chapter_id, next)
    .await;
    let cancel = CancelAt {
        store,
        cancel: cancellation(store, scope, suffix, &run_id).await,
        point,
        done: std::sync::atomic::AtomicBool::default(),
    };
    let observer = ParentWithSubrun {
        store,
        admin,
        scope: scope.clone(),
        run_id: run_id.clone(),
        cancel: std::sync::Mutex::new(Some(cancel)),
        subrun: tokio::sync::Mutex::default(),
        reached: std::sync::Mutex::default(),
    };
    let claim = claim_run(admin, scope.clone(), run_id.clone()).await;
    let stopped = complete_agent_run(store, &FakeDestination, &observer, &RuntimeClock, &claim);
    assert!(matches!(
        Box::pin(stopped).await,
        Err(CompleteAgentRunError::StaleFence)
    ));
    let subrun = observer.subrun.into_inner().unwrap();
    json_of(
        admin,
        "SELECT jsonb_agg(jsonb_build_array(run.status, run.wakeup_pending,
                  run.lease_expires_at IS NULL,
                  (SELECT jsonb_agg(jsonb_build_array(call.state, call.refusal))
                     FROM storyos.tool_calls AS call WHERE call.run_id = run.run_id))
                  ORDER BY run.run_id = $2::text::uuid)::text
           FROM storyos.agent_runs AS run WHERE run.run_id IN ($1::text::uuid, $2::text::uuid)",
        &[&run_id, &subrun],
    )
    .await
}

/// Item 7 of #436, its first sentence: the cancel transaction of the parent Run also cancels its
/// Subrun that is queued or claimed, with the steps of a cancelled Run. Only a Run with a Model
/// Attempt in flight gets the wake-up, and each ToolCall that did not end is refused with
/// `run_cancelled`. In each case, the parent has its second Model Attempt in flight and its
/// settled `read_chapter` call. Test prefix lead `76`.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_cancellation_of_the_parent_cancels_its_subrun_that_did_not_end() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "76a").await;
    complete(&store, &admin, &scope, &first).await;
    let project = (&scope, chapter_id.as_str());
    let mut ends = Vec::new();
    for (next, suffix, point) in [
        ("76b", 0x76f3, None),
        ("76c", 0x76f4, Some(ContractFaultPoint::DispatchClaimed)),
        ("76d", 0x76f5, Some(ContractFaultPoint::ToolCallsCommitted)),
    ] {
        ends.push(cancelled_with_subrun(&store, &admin, project, (next, suffix), point).await);
    }
    let parent = json!(["cancelled", true, true, [["settled", null]]]);

    assert_eq!(
        ends,
        [
            // The queued Subrun.
            json!([parent, ["cancelled", false, true, null]]),
            // The claimed Subrun with its first Model Attempt in flight.
            json!([parent, ["cancelled", true, true, null]]),
            // The claimed Subrun with its `read_chapter` call that did not end.
            json!([
                parent,
                ["cancelled", false, true, [["refused", "run_cancelled"]]]
            ]),
        ]
    );
}
