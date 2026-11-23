//! The cancellation, the pause, and the lost lease of the parent Run while its Subrun works, and
//! the selection of the Worker, at the dispatch seam with the fake destination (ADR 0103 section
//! 3A). Test prefix lead `76`.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::{FakeDestination, START_SUBRUN_SCRIPT};
use storyos_application::{
    AgentRunReadSelection, AgentRunReadStore, CancelAgentRunInput, CompleteAgentRunError,
    ContractFaultObserver, ContractFaultPoint, NoContractFaults, PauseAgentRunInput, ProjectScope,
    complete_agent_run,
};
use tokio_postgres::Client;

use super::tests::{parent, records, scripted, start, stopped_end, subrun_calls, subruns};
use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_finish_rule::conversation_of;
use crate::shared_tests::agent_run_steering::{cancellation, steering};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_agent_run::PAUSE_AGENT_RUN;
use crate::shared_tests::command_sequence_background_run::claim_in_project;
use crate::shared_tests::command_sequence_support::{CommandCall, issued, stores};
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::memory_work_schedule::claim_due;
use crate::shared_tests::model_gateway::{Admission, RuntimeClock, claim_run};

/// The command that the author sends to the parent Run while its Subrun works.
enum Command {
    Cancel(CommandCall<CancelAgentRunInput>),
    Pause(CommandCall<PauseAgentRunInput>),
}

/// The Contract Fault Point observer that settles `command` at the second dispatch claim: the
/// first dispatch claim of the Subrun.
struct CommandAtSubrun<'a> {
    store: &'a PostgresProjectReader,
    command: Command,
    reached: std::sync::Mutex<usize>,
}

impl ContractFaultObserver for CommandAtSubrun<'_> {
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
        match &self.command {
            Command::Cancel(call) => {
                (self.store.cancel_agent_run(&call.envelope, &call.input))
                    .await
                    .unwrap();
            }
            Command::Pause(call) => {
                (self.store.pause_agent_run(&call.envelope, &call.input))
                    .await
                    .unwrap();
            }
        }
    }
}

/// Runs the parent Run `run_id` of `scope` with `command` at the first dispatch claim of its
/// Subrun, and gives whether the claim of the parent stopped with a stale fence.
async fn commanded(
    store: &PostgresProjectReader,
    admin: &Client,
    (scope, run_id): (&ProjectScope, &str),
    command: Command,
) -> bool {
    let observer = CommandAtSubrun {
        store,
        command,
        reached: std::sync::Mutex::default(),
    };
    let claim = claim_run(admin, scope.clone(), run_id.to_owned()).await;
    let stopped = complete_agent_run(store, &FakeDestination, &observer, &RuntimeClock, &claim);
    matches!(stopped.await, Err(CompleteAgentRunError::StaleFence))
}

/// Completes the Run `run_id` of `scope` with the claim of the Worker, which must select it.
async fn by_worker(store: &PostgresProjectReader, scope: &ProjectScope, run_id: &str) {
    let claim = claim_in_project(store, scope).await.unwrap();
    assert_eq!(claim.run_id, run_id);
    let end = complete_agent_run(
        store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    );
    Box::pin(end).await.unwrap();
}

/// The status and the Run Wakeup of the Run, and the role and the dispatch state of each of its
/// Model Attempts of each role, in the order of their creation. A recorded response makes the
/// dispatch state `settled`. An `abort` Attempt gives whether it names the first Attempt.
async fn run_state(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT jsonb_build_array(run.status, run.wakeup_pending,
                  (SELECT jsonb_agg(jsonb_build_array(attempt.attempt_role, attempt.dispatch_state)
                            || CASE WHEN attempt.attempt_role = 'abort' THEN jsonb_build_array(
                                 attempt.payload->>'original_model_attempt_id' =
                                 (SELECT min(first.model_attempt_id::text)
                                    FROM storyos.model_attempts AS first
                                   WHERE first.run_id = run.run_id)) ELSE '[]' END
                            ORDER BY attempt.model_attempt_id)
                     FROM storyos.model_attempts AS attempt WHERE attempt.run_id = run.run_id))::text
           FROM storyos.agent_runs AS run WHERE run.run_id = $1::text::uuid",
        &[&run_id],
    )
    .await
}

/// Items 7 and 8 of #436: a cancellation of the parent while its Subrun works refuses the call of
/// the parent with `run_cancelled` and cancels the Subrun in the same transaction. The Model
/// Attempt of the Subrun in flight gets the wake-up. The Worker then selects the cancelled
/// Subrun and does the duties of a cancelled Run: one `abort` Attempt for the Attempt in flight,
/// and the wake-up is clear. No decision or successor Attempt of the Subrun starts after the
/// cancellation (criterion 5).
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_cancellation_of_the_parent_cancels_its_subrun() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, run_id) = scripted(&store, &admin, "760", "761", START_SUBRUN_SCRIPT).await;
    let cancel = Command::Cancel(cancellation(&store, &scope, 0x76f0, &run_id).await);
    let stopped = commanded(&store, &admin, (&scope, &run_id), cancel).await;
    let subrun = subruns(&admin, &run_id).await.remove(0);
    let cancelled = run_state(&admin, &subrun).await;
    let duties = claim_in_project(&store, &scope).await.map(|claim| {
        let selected = claim.run_id == subrun;
        (claim, selected)
    });
    if let Some((claim, _)) = &duties {
        (complete_agent_run(
            &store,
            &FakeDestination,
            &NoContractFaults,
            &RuntimeClock,
            claim,
        ))
        .await
        .unwrap();
    }

    assert_eq!(
        (
            stopped,
            subrun_calls(&admin, &run_id).await,
            records(&admin, &run_id).await[1].clone(),
            cancelled,
            duties.map(|(_, selected)| selected),
            run_state(&admin, &subrun).await,
        ),
        (
            true,
            json!([["refused", "run_cancelled", null]]),
            json!("cancelled"),
            json!(["cancelled", true, [["decision", "uncertain"]]]),
            Some(true),
            json!([
                "cancelled",
                false,
                [["decision", "uncertain"], ["abort", "settled", true]]
            ]),
        )
    );
}

/// Item 7 of #436: a pause of the parent while its Subrun works changes no record of the Subrun,
/// and the stopped work records no response. After a steering input, the next claim of the parent
/// continues the same Subrun, and the call settles with its answer (criterion 6).
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_pause_of_the_parent_continues_the_same_subrun_after_the_steering_input() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, run_id) = scripted(&store, &admin, "762", "763", START_SUBRUN_SCRIPT).await;
    let input = PauseAgentRunInput {
        run_id: run_id.clone(),
    };
    let pause = Command::Pause(issued(&store, &scope, 0x76f1, &PAUSE_AGENT_RUN, input).await);
    let stopped = commanded(&store, &admin, (&scope, &run_id), pause).await;
    let subrun = subruns(&admin, &run_id).await.remove(0);
    let paused = (
        run_state(&admin, &run_id).await,
        run_state(&admin, &subrun).await,
    );
    let steer = steering(&store, &scope, 0x76f2, &run_id, "继续。", None).await;
    (store.steer_agent_run(&steer.envelope, &steer.input))
        .await
        .unwrap();
    by_worker(&store, &scope, &run_id).await;
    // The fake destination answers the brief of the Subrun with its advice text.
    let answer = storyos_core::ADVISORY_TEXT;

    assert_eq!(
        (
            stopped,
            paused,
            subruns(&admin, &run_id).await,
            subrun_calls(&admin, &run_id).await,
            records(&admin, &run_id).await[1].clone(),
        ),
        (
            true,
            // The stopped work of the Subrun recorded no response.
            (
                json!(["paused", false, [["decision", "settled"]]]),
                json!(["claimed", false, [["decision", "uncertain"]]])
            ),
            vec![subrun],
            json!([["settled", null, answer]]),
            json!("completed"),
        )
    );
}

/// Items 2 and 7 of #436: when another Worker takes the parent over after a lost lease while its
/// fourth Subrun works, the work of the earlier claim stops, and the next claim of the parent
/// continues the same fourth Subrun. The limit of 4 Subrun Requests
/// does not refuse the call that has its Subrun Request, and the call settles with the answer.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_lost_lease_of_the_parent_continues_the_same_fourth_subrun() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // The parent claims its first request, and each of the first three Subruns claims three.
    let script = "SCRIPT:start_subrun 4";
    let end = stopped_end(&store, &admin, ["764", "765"], script, 11).await;
    // The fake destination answers the brief of each Subrun with its advice text.
    let answer = storyos_core::ADVISORY_TEXT;

    // The work of the fourth Subrun stopped after its first claim, and the next claim of the
    // parent continued the same Subrun.
    assert_eq!(
        end,
        json!([
            true,
            [
                ["completed", 3],
                ["completed", 3],
                ["completed", 3],
                ["claimed", 1]
            ],
            true,
            [
                ["settled", null, answer],
                ["settled", null, answer],
                ["settled", null, answer],
                ["settled", null, answer]
            ],
            "completed"
        ])
    );
}

/// Items 3 and 7 of #436: the Worker never selects a queued Subrun, or a claimed Subrun whose
/// lease expired, also when no other work exists (criterion 8). `getAgentRun` with the identity
/// of a Subrun answers as for a Run that does not exist, the earlier turns of the Conversation of
/// the parent and the input of its capture do not read the Subrun (criterion 11). Test
/// prefix lead `75`.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn no_worker_claim_no_run_read_and_no_earlier_turn_gives_a_subrun() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // A lease of one second, so that the lease of the claimed Subrun expires in the test.
    let store = store.with_readable_export_lease_ttl(std::time::Duration::from_secs(1));
    let (scope, run_id) = parent(&store, &admin, "750", "751").await;
    start(&store, &admin, &scope, &run_id, "读第一章").await;
    let queued_claim = claim_in_project(&store, &scope).await;
    // A pause of the parent keeps its Subrun `claimed`.
    let (scope, parent_run_id) = scripted(&store, &admin, "752", "753", START_SUBRUN_SCRIPT).await;
    let input = PauseAgentRunInput {
        run_id: parent_run_id.clone(),
    };
    let pause = Command::Pause(issued(&store, &scope, 0x75f0, &PAUSE_AGENT_RUN, input).await);
    commanded(&store, &admin, (&scope, &parent_run_id), pause).await;
    let subrun = subruns(&admin, &parent_run_id).await.remove(0);
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let lease = "SELECT to_jsonb(lease_expires_at < clock_timestamp())::text
                   FROM storyos.agent_runs WHERE run_id = $1::text::uuid";
    let expired = (
        run_state(&admin, &subrun).await,
        json_of(&admin, lease, &[&subrun]).await,
        claim_in_project(&store, &scope).await.is_none(),
    );
    let steer = steering(&store, &scope, 0x75f1, &parent_run_id, "继续。", None).await;
    (store.steer_agent_run(&steer.envelope, &steer.input))
        .await
        .unwrap();
    by_worker(&store, &scope, &parent_run_id).await;
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
    // The capture of the Conversation of the parent starts. Each message of its saved input
    // names its Run: the distinct Runs of those messages.
    claim_due(&store, &admin, &scope, &conversation).await;
    let capture = json_of(
        &admin,
        r#"SELECT jsonb_agg(DISTINCT found[1])::text
             FROM storyos.memory_captures AS capture
             JOIN storyos.context_assembly_manifests AS manifest ON manifest.run_id = capture.run_id,
                  jsonb_array_elements(manifest.payload->'selected') AS item,
                  regexp_matches(item->>'content', '"run_id":"([0-9a-f-]+)"', 'g') AS found
            WHERE capture.conversation_id = $1::text::uuid
              AND item->>'source_class' = 'memory_capture_input'"#,
        &[&conversation],
    )
    .await;
    let next = Admission {
        conversation: conversation_of("753"),
        ..Default::default()
    }
    .admit(&store, &scope, &chapter_id, "754")
    .await;
    let turns = crate::create_agent_run::earlier_turns::load(&admin, &scope, &next)
        .await
        .unwrap();

    assert_eq!(
        (
            queued_claim.is_none(),
            expired,
            read(subrun.clone()).await,
            read(parent_run_id.clone()).await,
            (turns.iter())
                .map(|turn| turn.messages[0].source.run_id.clone())
                .collect::<Vec<_>>(),
            capture,
        ),
        (
            true,
            (
                json!(["claimed", false, [["decision", "uncertain"]]]),
                json!(true),
                true
            ),
            false,
            true,
            vec![parent_run_id.clone()],
            // The input of the capture has only the parent: the Subrun is not in it.
            json!([parent_run_id]),
        )
    );
}
