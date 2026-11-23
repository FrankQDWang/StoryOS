//! The records and the requests of a Subrun at the dispatch seam with the fake destination (ADR
//! 0103 section 3A). Test prefix lead `67`.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    ClaimedAgentRun, CompleteAgentRunError, NoContractFaults, ProjectScope, complete_agent_run,
};
use storyos_core::{ToolCallState, ToolResultProjection};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_steering::{cancellation, queued};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::memory_entries::write_versions;
use crate::shared_tests::model_gateway::{Admission, RuntimeClock, claim_run};

/// A complete Run of the author of `prefix` whose author message is `message`, in a Project with
/// one current Memory entry at the Memory revision 1. The Run reads Chapter 1, so it has a
/// ToolCall.
async fn parent(
    store: &PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    next: &str,
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
        message: "SCRIPT:read_chapter 1",
        ..Default::default()
    }
    .admit(store, &scope, &chapter_id, next)
    .await;
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
async fn records(admin: &Client, run_id: &str) -> Value {
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

/// The Contract Fault Point observer that, at the second dispatch claim of the parent Run, starts
/// a Subrun of its first ToolCall, claims the Subrun, and cancels the parent.
struct CancelWithSubrun<'a> {
    store: &'a PostgresProjectReader,
    admin: &'a Client,
    scope: ProjectScope,
    run_id: String,
    subrun: tokio::sync::Mutex<Option<String>>,
    reached: std::sync::Mutex<usize>,
}

impl storyos_application::ContractFaultObserver for CancelWithSubrun<'_> {
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
        let (store, admin, scope) = (self.store, self.admin, &self.scope);
        let subrun = start(store, admin, scope, &self.run_id, "读第一章").await;
        claim_run(admin, scope.clone(), subrun.clone()).await;
        let cancel = cancellation(store, scope, 0x76f3, &self.run_id).await;
        (store.cancel_agent_run(&cancel.envelope, &cancel.input))
            .await
            .unwrap();
        *self.subrun.lock().await = Some(subrun);
    }
}

/// Item 7 of #436, its first sentence: the cancel transaction of the parent Run also cancels
/// its Subrun that did not end, with the steps of a cancelled Run. Only a Run with a Model Attempt
/// in flight gets the wake-up. Test prefix lead `76`.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_cancellation_of_the_parent_cancels_its_subrun_that_did_not_end() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "76a").await;
    complete(&store, &admin, &scope, &first).await;
    let run_id = Admission {
        message: "SCRIPT:read_chapter 1",
        ..Default::default()
    }
    .admit(&store, &scope, &chapter_id, "76b")
    .await;
    let observer = CancelWithSubrun {
        store: &store,
        admin: &admin,
        scope: scope.clone(),
        run_id: run_id.clone(),
        subrun: tokio::sync::Mutex::default(),
        reached: std::sync::Mutex::default(),
    };
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let stopped = complete_agent_run(&store, &FakeDestination, &observer, &RuntimeClock, &claim);
    let stopped = matches!(stopped.await, Err(CompleteAgentRunError::StaleFence));
    let subrun = observer.subrun.into_inner().unwrap();
    // The status, the wake-up, and whether the lease is clear, of each Run.
    let state = |run_id: String| {
        let admin = &admin;
        async move {
            json_of(
                admin,
                "SELECT jsonb_build_array(status, wakeup_pending, lease_expires_at IS NULL)::text
                   FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
                &[&run_id],
            )
            .await
        }
    };

    assert_eq!(
        (stopped, state(run_id).await, state(subrun).await),
        (
            true,
            // The parent had its second Model Attempt in flight.
            json!(["cancelled", true, true]),
            json!(["cancelled", false, true]),
        )
    );
}
