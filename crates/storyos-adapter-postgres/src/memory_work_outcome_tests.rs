//! The input and the outcomes of a Conversation capture in the store (ADR 0094 section 5, writer
//! 2): the Runs that the input reads, the Context item limit, the due time at the settlement of a
//! capture, the Project Scope, the actions of the author during a capture, and a stopped Worker.
//! The tests move the stored due time with the test administrator connection, and each test
//! completes or parks each Run that it queued.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    ConversationSelection, MemoryCaptureSource, MemoryWorkOutcome, MemoryWorkScheduleStore,
    NoContractFaults, StartedMemoryCapture, complete_agent_run,
};
use storyos_core::ConversationCaptureSkip;
use tokio_postgres::Client;

use super::tests::{claim_due, make_due};
use crate::agent_run_dispatch::tests::{RuntimeClock, stores};
use crate::agent_run_finish_rule::tests::complete;
use crate::agent_run_steering::tests::{
    AuthorAction, AuthorAt, Moment, cancellation, claim_duties, queued, settle, steering,
};
use crate::create_agent_run::context::chapter_list_tests::{admitted, rename};
use crate::memory_capture::loop_tests::{record, script, settlement};
use crate::memory_capture::tests::{project, run_capture};

/// The state of the schedule row of the Conversation: `none` (no row), `empty`, `now` (due), or
/// `quiet` (due at the end of the quiet time).
async fn due(admin: &Client, conversation_id: &str) -> String {
    let row = admin
        .query_opt(
            "SELECT CASE WHEN due_at IS NULL THEN 'empty'
                         WHEN due_at <= clock_timestamp() THEN 'now'
                         WHEN due_at > clock_timestamp() + interval '29 minutes' THEN 'quiet'
                    END
               FROM storyos.memory_work_schedules WHERE conversation_id = $1::text::uuid",
            &[&conversation_id],
        )
        .await
        .unwrap();
    row.map_or("none".to_owned(), |row| row.get(/*idx*/ 0))
}

/// The input of the first RunStep of the capture Run `run_id`: its projection mode, each message
/// as its role, its Run, and its text (a text above 60 characters as its character count), and
/// the first and the last Run of the capture row.
async fn read_runs(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT item ->> 'projection_mode', item ->> 'content', capture.first_run_id::text,
                    capture.last_run_id::text
               FROM storyos.context_assembly_manifests AS manifest
              CROSS JOIN LATERAL jsonb_array_elements(manifest.payload -> 'selected') AS item
               JOIN storyos.memory_captures AS capture USING (owner_user_id, project_id, run_id)
              WHERE manifest.run_id = $1::text::uuid AND manifest.manifest_role = 'decision'
                AND manifest.decision_position = 0
                AND item ->> 'source_class' = 'memory_capture_input'",
            &[&run_id],
        )
        .await
        .unwrap();
    let content: String = row.get(/*idx*/ 1);
    let messages: Vec<Value> = content
        .split("\n\n{\"role\"")
        .enumerate()
        .map(|(index, block)| {
            let block = if index == 0 {
                block.to_owned()
            } else {
                format!("{{\"role\"{block}")
            };
            let (header, text) = block.split_once('\n').unwrap();
            let header: Value = serde_json::from_str(header).unwrap();
            let count = text.chars().count();
            let text = if count > 60 {
                json!(count)
            } else {
                json!(text)
            };
            json!([header["role"], header["run_id"], text])
        })
        .collect();
    json!([
        row.get::<_, String>(/*idx*/ 0),
        messages,
        row.get::<_, String>(/*idx*/ 2),
        row.get::<_, String>(/*idx*/ 3)
    ])
}

fn started(outcome: MemoryWorkOutcome) -> StartedMemoryCapture {
    match outcome {
        MemoryWorkOutcome::Started(started) => started,
        other => panic!("the claim must start a capture: {other:?}"),
    }
}

fn source_runs(source: &MemoryCaptureSource) -> (String, String) {
    match source {
        MemoryCaptureSource::Conversation {
            conversation_id,
            first_run_id,
            ..
        } => (conversation_id.clone(), first_run_id.clone()),
        MemoryCaptureSource::Chapter { .. } => panic!("the source is a Conversation"),
    }
}

fn existing(conversation_id: &str) -> ConversationSelection {
    ConversationSelection::Existing {
        conversation_id: conversation_id.to_owned(),
    }
}

/// The input has the messages of each completed Run after the last Run that a capture read, also
/// a steering input that a RunStep read. A cancelled Run gives no message. A capture with no
/// later Run settles with `no_observation`, and the due time stays empty.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_input_reads_each_completed_run_with_its_steering_input_and_no_cancelled_run() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // A capture gives way to each queued Run of the author, as in `project`.
    admin
        .execute(
            "UPDATE storyos.agent_runs
                SET status = 'paused', lease_expires_at = NULL, wakeup_pending = false
              WHERE run_kind = 'author' AND status = 'queued'",
            &[],
        )
        .await
        .unwrap();
    let (scope, chapter_id, first) = queued(&store, "580").await;
    let input = steering(&store, &scope, 0x5800, &first, "Keep the voice.", None).await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::Steer(vec![input]),
    );
    settle(&store, &admin, &scope, &first, &author)
        .await
        .unwrap();
    let conversation_id: String = admin
        .query_one(
            "SELECT conversation_id::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&first],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let cancelled = admitted(
        &store,
        &scope,
        &chapter_id,
        "581",
        "取消这一条。",
        existing(&conversation_id),
    )
    .await;
    let cancel = cancellation(&store, &scope, 0x5801, &cancelled).await;
    store
        .cancel_agent_run(&cancel.envelope, &cancel.input)
        .await
        .unwrap();
    if let Some(claim) = claim_duties(&admin, &scope, &cancelled).await {
        let duties = complete_agent_run(
            &store,
            &FakeDestination,
            &NoContractFaults,
            &RuntimeClock,
            &claim,
        );
        duties.await.unwrap();
    }
    let last = admitted(
        &store,
        &scope,
        &chapter_id,
        "582",
        "继续写。",
        existing(&conversation_id),
    )
    .await;
    complete(&store, &admin, &scope, &last).await;
    let capture = started(claim_due(&store, &admin, &scope, &conversation_id).await);
    let read = read_runs(&admin, &capture.run_id).await;
    run_capture(&store, &admin, &scope, &capture.run_id).await;

    assert_eq!(
        (
            read,
            settlement(&admin, &capture.run_id).await[2].clone(),
            due(&admin, &conversation_id).await
        ),
        (
            json!([
                "exact_required",
                [
                    ["author", first, "Help with this passage."],
                    ["author", first, "Keep the voice."],
                    ["author", last, "继续写。"]
                ],
                first,
                last
            ]),
            json!("no_observation"),
            "empty".to_owned()
        )
    );
}

/// Above the Context item limit, the input has the oldest Runs that fit, and the capture row
/// records the last Run that it read. A `no_observation` settlement with a later completed Run
/// makes the row due at once, and the next capture reads that Run.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_bounded_input_reads_the_oldest_runs_that_fit_and_its_settlement_makes_the_row_due() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "583").await;
    let (conversation_id, first) = source_runs(&source);
    let mut later = Vec::new();
    for (prefix, character) in [("584", "林"), ("585", "舟")] {
        let message = character.repeat(/*n*/ 20_000);
        let run_id = admitted(
            &store,
            &scope,
            &chapter_id,
            prefix,
            &message,
            existing(&conversation_id),
        )
        .await;
        complete(&store, &admin, &scope, &run_id).await;
        later.push(run_id);
    }
    let bounded = started(claim_due(&store, &admin, &scope, &conversation_id).await);
    let read = read_runs(&admin, &bounded.run_id).await;
    run_capture(&store, &admin, &scope, &bounded.run_id).await;
    let after = due(&admin, &conversation_id).await;
    let next = started(claim_due(&store, &admin, &scope, &conversation_id).await);
    let read_next = read_runs(&admin, &next.run_id).await;
    run_capture(&store, &admin, &scope, &next.run_id).await;

    assert_eq!(
        (read, after, read_next, due(&admin, &conversation_id).await),
        (
            json!([
                "exact_required",
                [
                    ["author", first, "Help with this passage."],
                    ["author", later[0], 20_000]
                ],
                first,
                later[0]
            ]),
            "now".to_owned(),
            json!([
                "exact_required",
                [["author", later[1], 20_000]],
                later[1],
                later[1]
            ]),
            "empty".to_owned()
        )
    );
}

/// A new message of the author during a capture is admitted, and its completion sets the due
/// time again. An edit of a Chapter during the request does not change the capture: it completes
/// with the messages that it pinned. Its `observations_recorded` settlement with the later Run
/// makes the row due at once. The next capture records with no later Run, so the due time stays
/// empty.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_new_message_during_a_capture_does_not_wait_and_the_settlement_makes_the_row_due() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "586").await;
    let (conversation_id, first) = source_runs(&source);
    let observation = |title: &str| {
        script(&[record(
            json!([{"type": "character", "title": title, "text": "船夫。"}]),
        )])
    };
    let second = admitted(
        &store,
        &scope,
        &chapter_id,
        "587",
        &observation("林舟"),
        existing(&conversation_id),
    )
    .await;
    complete(&store, &admin, &scope, &second).await;
    let capture = started(claim_due(&store, &admin, &scope, &conversation_id).await);
    let third = admitted(
        &store,
        &scope,
        &chapter_id,
        "588",
        &observation("渡口"),
        existing(&conversation_id),
    )
    .await;
    complete(&store, &admin, &scope, &third).await;
    let during = due(&admin, &conversation_id).await;
    let edit = rename(&store, &scope, 0x5802, &chapter_id, "雨夜").await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::UpdateChapter(Box::new(edit)),
    );
    settle(&store, &admin, &scope, &capture.run_id, &author)
        .await
        .unwrap();
    run_capture(&store, &admin, &scope, &capture.run_id).await;
    let read = read_runs(&admin, &capture.run_id).await;
    let after = due(&admin, &conversation_id).await;
    let next = started(claim_due(&store, &admin, &scope, &conversation_id).await);
    run_capture(&store, &admin, &scope, &next.run_id).await;
    let outcome = |run_id: String| {
        let admin = &admin;
        async move { settlement(admin, &run_id).await[2].clone() }
    };

    assert_eq!(
        (
            during,
            [read[2].clone(), read[3].clone()],
            outcome(capture.run_id).await,
            after,
            read_runs(&admin, &next.run_id).await[3].clone(),
            outcome(next.run_id).await,
            due(&admin, &conversation_id).await
        ),
        (
            "quiet".to_owned(),
            [json!(first), json!(second)],
            json!("observations_recorded"),
            "now".to_owned(),
            json!(third),
            json!("observations_recorded"),
            "empty".to_owned()
        )
    );
}

/// The settlement of a `step_limit` or a `failed` capture does not set the due time. The next
/// completed Run makes the row due again, and the next capture reads also the messages that the
/// capture before it did not complete.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_failed_or_step_limit_capture_sets_no_due_time_and_the_next_capture_reads_its_runs() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "589").await;
    let (conversation_id, first) = source_runs(&source);
    let max = usize::try_from(storyos_core::MEMORY_CAPTURE_MAX_RUN_STEPS).unwrap();
    let edits = script(&vec![
        ("propose_edit", json!({"reason": "改写"}));
        max + 2
    ]);
    let second = admitted(
        &store,
        &scope,
        &chapter_id,
        "58a",
        &edits,
        existing(&conversation_id),
    )
    .await;
    complete(&store, &admin, &scope, &second).await;
    let limited = started(claim_due(&store, &admin, &scope, &conversation_id).await);
    let third = admitted(
        &store,
        &scope,
        &chapter_id,
        "58b",
        "继续写。",
        existing(&conversation_id),
    )
    .await;
    complete(&store, &admin, &scope, &third).await;
    run_capture(&store, &admin, &scope, &limited.run_id).await;
    let after_limit = due(&admin, &conversation_id).await;

    let failed = started(claim_due(&store, &admin, &scope, &conversation_id).await);
    let grant = |version: &'static str| {
        let admin = &admin;
        let project_id = scope.project_id.as_ref().to_owned();
        async move {
            admin
                .execute(
                    "UPDATE storyos.project_destination_grants SET grant_scope = $2
                      WHERE project_id = $1::text::uuid",
                    &[
                        &project_id,
                        &format!("project_writing_assistance_{version}"),
                    ],
                )
                .await
                .unwrap();
        }
    };
    grant("v4").await;
    run_capture(&store, &admin, &scope, &failed.run_id).await;
    grant("v5").await;
    let after_failed = due(&admin, &conversation_id).await;
    let fourth = admitted(
        &store,
        &scope,
        &chapter_id,
        "58c",
        "再写一章。",
        existing(&conversation_id),
    )
    .await;
    complete(&store, &admin, &scope, &fourth).await;
    let next = started(claim_due(&store, &admin, &scope, &conversation_id).await);
    run_capture(&store, &admin, &scope, &next.run_id).await;
    let ends = |run_id: String| {
        let admin = &admin;
        async move {
            let read = read_runs(admin, &run_id).await;
            let settled = settlement(admin, &run_id).await;
            json!([read[2], read[3], settled[2]])
        }
    };

    assert_eq!(
        (
            ends(limited.run_id).await,
            after_limit,
            ends(failed.run_id).await,
            after_failed,
            ends(next.run_id).await
        ),
        (
            json!([first, second, "step_limit"]),
            "quiet".to_owned(),
            json!([first, third, "failed"]),
            "empty".to_owned(),
            json!([first, fourth, "step_limit"])
        )
    );
}

/// A capture reads only the Conversation of its row in the Project Scope of the row.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_capture_reads_only_the_conversation_of_its_row_in_its_project_scope() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope_a, _, source_a) = project(&store, &admin, "58d").await;
    let (scope_b, _, source_b) = project(&store, &admin, "58e").await;
    let mut reads = Vec::new();
    for (scope, source) in [(&scope_a, &source_a), (&scope_b, &source_b)] {
        let (conversation_id, _) = source_runs(source);
        let capture = started(claim_due(&store, &admin, scope, &conversation_id).await);
        reads.push(read_runs(&admin, &capture.run_id).await[1].clone());
        run_capture(&store, &admin, scope, &capture.run_id).await;
    }
    let message = |source| json!([["author", source_runs(source).1, "Help with this passage."]]);

    assert_eq!(reads, [message(&source_a), message(&source_b)]);
}

/// A Worker that stops after the claim of a row and before the start of its capture leaves the
/// row due, because the claim and the start are one transaction. The next claim starts one
/// capture, and a second delivery starts no second capture for the same messages.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_stopped_worker_leaves_the_row_due_and_the_next_claim_starts_one_capture() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "58f").await;
    let (conversation_id, _) = source_runs(&source);
    make_due(&admin, &conversation_id).await;
    // The lock holds the start of the capture after the claim of the row. The end of the session
    // of the claim is the stop of the Worker.
    admin
        .batch_execute("BEGIN; LOCK TABLE storyos.memory_captures IN SHARE ROW EXCLUSIVE MODE")
        .await
        .unwrap();
    let stop = async {
        // A blocked session gives no signal, so this is the one wait with a sleep.
        for _ in 0..500 {
            let waiting = admin
                .query_opt(
                    "SELECT pg_terminate_backend(pid) FROM pg_stat_activity
                      WHERE pid <> pg_backend_pid() AND wait_event_type = 'Lock'
                        AND query LIKE '%INSERT INTO storyos.memory_captures%'",
                    &[],
                )
                .await
                .unwrap();
            if waiting.is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(/*millis*/ 10)).await;
        }
        admin.batch_execute("ROLLBACK").await.unwrap();
    };
    let (stopped, ()) = tokio::join!(store.claim_due_memory_work(), stop);
    let after_stop = due(&admin, &conversation_id).await;
    let capture = started(
        store
            .claim_due_memory_work()
            .await
            .unwrap()
            .unwrap()
            .outcome,
    );
    let second = claim_due(&store, &admin, &scope, &conversation_id).await;
    run_capture(&store, &admin, &scope, &capture.run_id).await;
    let captures: i64 = admin
        .query_one(
            "SELECT count(*) FROM storyos.memory_captures WHERE conversation_id = $1::text::uuid",
            &[&conversation_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);

    assert_eq!(
        (stopped.is_err(), after_stop, second, captures),
        (
            true,
            "now".to_owned(),
            MemoryWorkOutcome::Skipped(ConversationCaptureSkip::CaptureInProgress),
            1
        )
    );
}
