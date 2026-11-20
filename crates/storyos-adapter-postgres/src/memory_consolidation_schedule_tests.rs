//! The schedule of a consolidation pass in the store (ADR 0094 section 6): the due write at the
//! settlement of a capture and of a pass, one test for each row of the decision table, and the
//! path through `run_once` from two Runs of the author to the entries. The tests move the stored
//! times with the test administrator connection, and each test completes each Run that it queued.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    ConversationSelection, MemoryCaptureSource, MemoryWorkOutcome, MemoryWorkScheduleStore,
    MemoryWorkSource, NoContractFaults, ProjectScope,
};
use storyos_core::{ConsolidationSkip, MemoryEntryOrigin};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::chapter_list_context::admitted;
use crate::shared_tests::export_work::admit_readable_export;
use crate::shared_tests::memory_capture::{project, run_capture};
use crate::shared_tests::memory_capture_loop::{record, script};
use crate::shared_tests::memory_consolidation::{capture, create, scripted, settled};
use crate::shared_tests::memory_consolidation_schedule::{claim_pass, pass_due};
use crate::shared_tests::memory_entries::write_versions;
use crate::shared_tests::memory_work_schedule::make_due;
use crate::shared_tests::model_gateway::NoEmbeddings;
use crate::shared_tests::model_gateway::stores;

/// Claims the due consolidation row of the Project and requires a started pass. Returns its Run.
async fn started_pass(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
) -> String {
    match claim_pass(store, admin, scope).await {
        MemoryWorkOutcome::PassStarted(started) => started.run_id,
        outcome => panic!("the claim must start a pass: {outcome:?}"),
    }
}

/// A capture that stores observations makes the Project due now when it has no earlier pass. When
/// the last pass started 10 minutes ago, the next capture makes it due at that start and 30
/// minutes. The row names no Conversation, and its source is the Project.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_capture_with_observations_makes_the_project_due_after_the_last_pass() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "780").await;
    let observation = json!([{ "type": "other", "title": "渡口", "text": "渡口在河边。" }]);
    capture(&store, &admin, &scope, &source, observation.clone()).await;
    let first = pass_due(&admin, &scope).await;
    let run_id = started_pass(&store, &admin, &scope).await;
    run_capture(&store, &admin, &scope, &run_id).await;
    admin
        .execute(
            "UPDATE storyos.memory_consolidation_passes
                SET started_at = clock_timestamp() - interval '10 minutes'
              WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    capture(&store, &admin, &scope, &source, observation).await;

    assert_eq!(
        (first, pass_due(&admin, &scope).await),
        (
            Some(json!([true, true, "now"])),
            Some(json!([true, true, "00:30:00"]))
        )
    );
}

/// Table rows 1 and 2: a pass in progress, and then no waiting observation after the pass
/// consumed each observation. Each claim makes the due time empty and starts no pass.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_pass_in_progress_and_no_waiting_observation_start_no_pass() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "781").await;
    let observation = json!([{ "type": "other", "title": "渡口", "text": "渡口在河边。" }]);
    capture(&store, &admin, &scope, &source, observation).await;
    let started = crate::shared_tests::memory_consolidation::start_pass(&store, &scope)
        .await
        .unwrap();
    let in_progress = claim_pass(&store, &admin, &scope).await;
    run_capture(&store, &admin, &scope, &started.run_id).await;
    let nothing_waits = claim_pass(&store, &admin, &scope).await;

    assert_eq!(
        (
            in_progress,
            settled(&admin, &started.run_id).await,
            nothing_waits
        ),
        (
            MemoryWorkOutcome::PassSkipped(ConsolidationSkip::PassInProgress),
            json!(["completed", "no_operation", null, [], ["consumed"]]),
            MemoryWorkOutcome::PassSkipped(ConsolidationSkip::NothingWaits)
        )
    );
}

/// Table row 3: the admission of S7-D3b refuses the start, so the claim writes no pass and makes
/// the due time empty. The next claim of the same step order takes the due capture row of a
/// Conversation of another Project and starts its capture.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_refused_start_does_not_stop_the_next_row() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "782").await;
    let observation = json!([{ "type": "other", "title": "渡口", "text": "渡口在河边。" }]);
    capture(&store, &admin, &scope, &source, observation).await;
    let (other, _, other_source) = project(&store, &admin, "783").await;
    let MemoryCaptureSource::Conversation {
        conversation_id, ..
    } = &other_source
    else {
        unreachable!()
    };
    admin
        .execute(
            "UPDATE storyos.project_destination_grants
                SET grant_scope = 'project_writing_assistance_v4' WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    make_due(&admin, conversation_id).await;
    admin
        .execute(
            "UPDATE storyos.memory_work_schedules
                SET due_at = clock_timestamp() - interval '2 seconds'
              WHERE project_id = $1::text::uuid AND work_kind = 'consolidation'",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let first = store.claim_due_memory_work().await.unwrap().unwrap();
    let second = store.claim_due_memory_work().await.unwrap().unwrap();
    let MemoryWorkOutcome::Started(capture_started) = &second.outcome else {
        panic!("the second claim must start a capture: {second:?}");
    };
    run_capture(&store, &admin, &other, &capture_started.run_id).await;
    let passes: i64 = admin
        .query_one(
            "SELECT count(*) FROM storyos.memory_consolidation_passes
              WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);

    assert_eq!(
        (
            (first.project_scope, first.source, first.outcome),
            (second.project_scope, second.source),
            passes,
            pass_due(&admin, &scope).await
        ),
        (
            (
                scope.clone(),
                MemoryWorkSource::Project,
                MemoryWorkOutcome::PassSkipped(ConsolidationSkip::StartRefused)
            ),
            (
                other,
                MemoryWorkSource::Conversation(conversation_id.clone())
            ),
            0,
            Some(json!([true, true, null]))
        )
    );
}

/// A pass that settles `stale` makes the Project due again at its start and 30 minutes, because
/// its observation still waits. The next pass runs with the current entries: its create takes the
/// number after the entry of the author.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_stale_pass_makes_the_project_due_again() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "784").await;
    let script = json!([scripted("灯", &[create("foreshadowing", "灯", json!([1]))])]);
    capture(&store, &admin, &scope, &source, script).await;
    let stale = started_pass(&store, &admin, &scope).await;
    write_versions(
        &admin,
        &scope,
        &[(1, 1, 1, "current", "other", "阿青", "船夫")],
    )
    .await;
    run_capture(&store, &admin, &scope, &stale).await;
    let (stale, due) = (
        settled(&admin, &stale).await,
        pass_due(&admin, &scope).await,
    );
    let again = started_pass(&store, &admin, &scope).await;
    run_capture(&store, &admin, &scope, &again).await;

    assert_eq!(
        (stale, due, settled(&admin, &again).await),
        (
            json!([
                "completed",
                "stale",
                null,
                [["not_applied", null, null]],
                ["waiting"]
            ]),
            Some(json!([true, true, "00:30:00"])),
            json!([
                "completed",
                "applied",
                null,
                [["applied", null, 2]],
                ["consumed"]
            ])
        )
    );
}

/// A pass that settles `failed` makes the Project due again at its start and 30 minutes, because
/// its observation still waits.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_failed_pass_makes_the_project_due_again() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "785").await;
    let observation = json!([{ "type": "other", "title": "空", "text": "SCRIPT:no-call" }]);
    capture(&store, &admin, &scope, &source, observation).await;
    let failed = started_pass(&store, &admin, &scope).await;
    run_capture(&store, &admin, &scope, &failed).await;

    assert_eq!(
        (
            settled(&admin, &failed).await,
            pass_due(&admin, &scope).await
        ),
        (
            json!(["completed", "failed", "no_operation_call", [], ["waiting"]]),
            Some(json!([true, true, "00:30:00"]))
        )
    );
}

/// The settled export work, the status of the Run `run_id`, and the outcome of the capture and of
/// the pass of the Project after one step of the Worker.
async fn step_state(admin: &Client, scope: &ProjectScope, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_build_array(
                      (SELECT count(*) FROM storyos.human_readable_manuscript_export_operations
                        WHERE project_id = $1::text::uuid AND settled_result IS NOT NULL),
                      (SELECT status FROM storyos.agent_runs WHERE run_id = $2::text::uuid),
                      (SELECT jsonb_agg(COALESCE(outcome, state) ORDER BY started_at)
                         FROM storyos.memory_captures
                        WHERE project_id = $1::text::uuid AND last_run_id = $2::text::uuid),
                      (SELECT jsonb_agg(COALESCE(outcome, 'running'))
                         FROM storyos.memory_consolidation_passes
                        WHERE project_id = $1::text::uuid))::text",
            &[&scope.project_id.as_ref(), &run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get(/*idx*/ 0)).unwrap()
}

/// From two completed Runs of a Conversation on the fake destination to the entries through
/// `run_once`. The work of the Project changes in the loop order: the export work, then the queued
/// Run of the author, then the due schedule row. The capture of the two Runs stores the
/// observation, the pass that its settlement made due creates the entry of the script, and the
/// entry is current with the Conversation as its origin. The test moves the due time of the
/// Conversation after the completion of the second Run.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn run_once_takes_two_runs_through_a_capture_and_a_pass_to_the_entries() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "786").await;
    let MemoryCaptureSource::Conversation {
        conversation_id, ..
    } = &source
    else {
        unreachable!()
    };
    let observation = scripted("林舟", &[create("character", "林舟", json!([1]))]);
    let message = format!("记住林舟。\n{}", script(&[record(json!([observation]))]));
    let existing = ConversationSelection::Existing {
        conversation_id: conversation_id.clone(),
    };
    let second = admitted(&store, &scope, &chapter_id, "787", &message, existing).await;
    admit_readable_export(&store, &scope).await;
    make_due(&admin, conversation_id).await;
    let destination = storyos_worker::ModelDestination {
        adapter: FakeDestination,
        observer: NoContractFaults,
        tools: storyos_application::NoToolDestination,
        embeddings: NoEmbeddings,
    };
    // A step can claim work that other tests left. The test records the state of its Project
    // only when it changes.
    let mut states = vec![step_state(&admin, &scope, &second).await];
    for _ in 0..24 {
        storyos_worker::run_once(&store, &destination)
            .await
            .unwrap();
        let state = step_state(&admin, &scope, &second).await;
        if states.last() != Some(&state) {
            if state[1] == "completed" && state[2].is_null() {
                make_due(&admin, conversation_id).await;
            }
            states.push(state);
        }
        if states.last().unwrap()[3] == json!(["applied"]) {
            break;
        }
    }
    let due = pass_due(&admin, &scope).await;
    let origins: Vec<_> = storyos_application::list_memory_entries(&store, &scope)
        .await
        .unwrap()
        .unwrap()
        .into_iter()
        .map(|entry| (entry.number, entry.origin))
        .collect();

    assert_eq!(
        (Value::from(states), due, origins),
        (
            json!([
                [0, "queued", null, null],
                [1, "queued", null, null],
                [1, "completed", null, null],
                [1, "completed", ["running"], null],
                [1, "completed", ["observations_recorded"], null],
                [1, "completed", ["observations_recorded"], ["running"]],
                [1, "completed", ["observations_recorded"], ["applied"]]
            ]),
            Some(json!([true, true, null])),
            vec![(1, MemoryEntryOrigin::Conversation)]
        )
    );
}
