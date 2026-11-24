//! The input and the outcomes of a Conversation capture in the store (ADR 0094 section 5, writer
//! 2): the Runs that the input reads, the Context item limit, the due time at the settlement of a
//! capture, the Project Scope, the actions of the author during a capture, and a stopped Worker.
//! The tests move the stored due time with the test administrator connection, and each test
//! completes or parks each Run that it queued.

use crate::statement_cache::Client;
use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    ConversationSelection, MemoryCaptureSource, MemoryWorkOutcome, MemoryWorkScheduleStore,
    NoContractFaults, ProjectScope, StartedMemoryCapture, complete_agent_run,
};

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_steering::{
    AuthorAction, AuthorAt, Moment, cancellation, claim_duties, queued, settle, steering,
};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::chapter_list_context::{admitted, rename};
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::memory_capture::{project, run_capture};
use crate::shared_tests::memory_capture_loop::{record, script, settlement};
use crate::shared_tests::memory_work_outcome::stopped_at_the_start;
use crate::shared_tests::memory_work_schedule::{claim_due, make_due};
use crate::shared_tests::model_gateway::RuntimeClock;

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
            let block = match index {
                0 => block.to_owned(),
                _ => format!("{{\"role\"{block}"),
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

/// The Conversation and the first Run of a source of a capture.
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

/// The Run status, the capture outcome, and the first and the last Run of the capture Run.
async fn ends(admin: &Client, run_id: &str) -> Value {
    let read = read_runs(admin, run_id).await;
    let settled = settlement(admin, run_id).await;
    json!([settled[0], settled[2], read[2], read[3]])
}

/// A Worker that stops after the claim of the row of the Conversation and before the start of its
/// capture leaves the row due, because the claim and the start are one transaction. The next
/// claim starts one capture, and a second delivery starts no second capture for the same
/// messages. Returns whether the stopped claim failed, the due state after it, the skip of the
/// second delivery, and the count of the captures of the Conversation, and the started capture.
async fn stopped_claim(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    conversation_id: &str,
) -> (Value, StartedMemoryCapture) {
    make_due(admin, conversation_id).await;
    let stopped = stopped_at_the_start(admin, store.claim_due_memory_work()).await;
    let after_stop = due(admin, conversation_id).await;
    let capture = store.claim_due_memory_work().await.unwrap().unwrap();
    let capture = started(capture.outcome);
    let MemoryWorkOutcome::Skipped(second) = claim_due(store, admin, scope, conversation_id).await
    else {
        panic!("a second delivery must start no capture");
    };
    let captures: i64 = admin
        .query_one(
            "SELECT count(*) FROM storyos.memory_captures WHERE conversation_id = $1::text::uuid",
            &[&conversation_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let facts = json!([stopped.is_err(), after_stop, second.label(), captures]);
    (facts, capture)
}

/// The input has the messages of each completed Run of its Conversation after the last Run that
/// a capture read, also a steering input that a RunStep read. A cancelled Run gives no message. A
/// capture reads only the Conversation of its row in its Project Scope. A capture with no later
/// Run settles with `no_observation`, and the due time stays empty. The row of the second Project
/// goes through a stopped Worker (`stopped_claim`).
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_capture_reads_the_completed_runs_of_its_conversation_and_no_cancelled_run() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (other_scope, _, other) = project(&store, &admin, "583").await;
    let (scope, chapter_id, first) = queued(&store, "580").await;
    let input = steering(&store, &scope, 0x58fa, &first, "Keep the voice.", None).await;
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
    let conversation = existing(&conversation_id);
    let cancelled = admitted(&store, &scope, &chapter_id, "581", "取消。", conversation).await;
    let cancel = cancellation(&store, &scope, 0x58fb, &cancelled).await;
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
    let conversation = existing(&conversation_id);
    let last = admitted(&store, &scope, &chapter_id, "582", "继续写。", conversation).await;
    complete(&store, &admin, &scope, &last).await;
    let capture = started(claim_due(&store, &admin, &scope, &conversation_id).await);
    run_capture(&store, &admin, &scope, &capture.run_id).await;
    let mut reads = vec![
        read_runs(&admin, &capture.run_id).await,
        settlement(&admin, &capture.run_id).await[2].clone(),
    ];
    let (other_conversation, other_first) = source_runs(&other);
    let (stop, capture) = stopped_claim(&store, &admin, &other_scope, &other_conversation).await;
    run_capture(&store, &admin, &other_scope, &capture.run_id).await;
    reads.push(read_runs(&admin, &capture.run_id).await);
    reads.push(settlement(&admin, &capture.run_id).await[2].clone());

    assert_eq!(
        (reads, due(&admin, &conversation_id).await, stop),
        (
            vec![
                json!([
                    "exact_required",
                    [
                        ["author", first, "Help with this passage."],
                        ["author", first, "Keep the voice."],
                        ["assistant", first, 76],
                        ["author", last, "继续写。"],
                        ["assistant", last, 76]
                    ],
                    first,
                    last
                ]),
                json!("no_observation"),
                json!([
                    "exact_required",
                    [
                        ["author", other_first, "Help with this passage."],
                        ["assistant", other_first, 76]
                    ],
                    other_first,
                    other_first
                ]),
                json!("no_observation"),
            ],
            "empty".to_owned(),
            json!([true, "now", "capture_in_progress", 1])
        )
    );
}

/// Above the Context item limit, the input has the oldest Runs that fit, and the capture row
/// records the last Run that it read. A `no_observation` settlement with a later completed Run
/// makes the row due at once, and the next capture reads that Run.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_bounded_input_reads_the_oldest_runs_that_fit_and_its_settlement_makes_the_row_due() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "584").await;
    let (conversation_id, first) = source_runs(&source);
    // An author message has at most 8,000 characters, so the fourth long Run is above the limit.
    let mut later = Vec::new();
    for (prefix, character) in [("585", "林"), ("586", "舟"), ("587", "渡"), ("588", "口")] {
        let message = character.repeat(/*n*/ 8_000);
        let conversation = existing(&conversation_id);
        let run_id = admitted(&store, &scope, &chapter_id, prefix, &message, conversation).await;
        complete(&store, &admin, &scope, &run_id).await;
        later.push(run_id);
    }
    let bounded = started(claim_due(&store, &admin, &scope, &conversation_id).await);
    run_capture(&store, &admin, &scope, &bounded.run_id).await;
    let after = due(&admin, &conversation_id).await;
    let next = started(claim_due(&store, &admin, &scope, &conversation_id).await);
    run_capture(&store, &admin, &scope, &next.run_id).await;
    let long = |run_id: &String| json!([["author", run_id, 8_000], ["assistant", run_id, 76]]);
    let mut bounded_messages = vec![
        json!(["author", first, "Help with this passage."]),
        json!(["assistant", first, 76]),
    ];
    for run_id in &later[..3] {
        bounded_messages.extend(long(run_id).as_array().unwrap().iter().cloned());
    }

    assert_eq!(
        (
            read_runs(&admin, &bounded.run_id).await,
            settlement(&admin, &bounded.run_id).await[2].clone(),
            after,
            read_runs(&admin, &next.run_id).await,
            due(&admin, &conversation_id).await
        ),
        (
            json!(["exact_required", bounded_messages, first, later[2]]),
            json!("no_observation"),
            "now".to_owned(),
            json!(["exact_required", long(&later[3]), later[3], later[3]]),
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
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "589").await;
    let (conversation_id, first) = source_runs(&source);
    let observation = |title: &str| {
        script(&[record(
            json!([{"type": "character", "title": title, "text": "船夫。"}]),
        )])
    };
    let conversation = existing(&conversation_id);
    let message = observation("林舟");
    let second = admitted(&store, &scope, &chapter_id, "58a", &message, conversation).await;
    complete(&store, &admin, &scope, &second).await;
    let capture = started(claim_due(&store, &admin, &scope, &conversation_id).await);
    let conversation = existing(&conversation_id);
    let message = observation("渡口");
    let third = admitted(&store, &scope, &chapter_id, "58b", &message, conversation).await;
    complete(&store, &admin, &scope, &third).await;
    let during = due(&admin, &conversation_id).await;
    let edit = rename(&store, &scope, 0x58fc, &chapter_id, "雨夜").await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::UpdateChapter(Box::new(edit)),
    );
    settle(&store, &admin, &scope, &capture.run_id, &author)
        .await
        .unwrap();
    let title: String = admin
        .query_one(
            "SELECT title FROM storyos.manuscript_objects
              WHERE manuscript_object_id = $1::text::uuid",
            &[&chapter_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let after = due(&admin, &conversation_id).await;
    let next = started(claim_due(&store, &admin, &scope, &conversation_id).await);
    run_capture(&store, &admin, &scope, &next.run_id).await;

    assert_eq!(
        (
            during,
            title,
            ends(&admin, &capture.run_id).await,
            after,
            ends(&admin, &next.run_id).await,
            due(&admin, &conversation_id).await
        ),
        (
            "quiet".to_owned(),
            "雨夜".to_owned(),
            json!(["completed", "observations_recorded", first, second]),
            "now".to_owned(),
            json!(["completed", "observations_recorded", third, third]),
            "empty".to_owned()
        )
    );
}

/// The settlement of a `step_limit` or a `failed` capture does not set the due time, so the Host
/// starts no capture for the same messages by itself: a Run that completes during the capture
/// leaves the row due at the end of the quiet time, not at once. The next capture reads also the
/// messages that the capture before it did not complete.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_failed_or_step_limit_capture_sets_no_due_time_and_the_next_capture_reads_its_runs() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "58c").await;
    let (conversation_id, first) = source_runs(&source);
    let max = usize::try_from(storyos_core::MEMORY_CAPTURE_MAX_RUN_STEPS).unwrap();
    let edits = script(&vec![
        ("propose_edit", json!({"reason": "改写"}));
        max + 2
    ]);
    let conversation = existing(&conversation_id);
    let second = admitted(&store, &scope, &chapter_id, "58d", &edits, conversation).await;
    complete(&store, &admin, &scope, &second).await;
    let during = |prefix: &'static str| {
        let (store, admin, scope) = (&store, &admin, &scope);
        let (chapter_id, conversation) = (chapter_id.clone(), existing(&conversation_id));
        async move {
            let run_id =
                admitted(store, scope, &chapter_id, prefix, "继续写。", conversation).await;
            complete(store, admin, scope, &run_id).await;
            run_id
        }
    };
    let limited = started(claim_due(&store, &admin, &scope, &conversation_id).await);
    let third = during("58e").await;
    run_capture(&store, &admin, &scope, &limited.run_id).await;
    let after_limit = due(&admin, &conversation_id).await;
    let failed = started(claim_due(&store, &admin, &scope, &conversation_id).await);
    let fourth = during("58f").await;
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
    let next = started(claim_due(&store, &admin, &scope, &conversation_id).await);
    run_capture(&store, &admin, &scope, &next.run_id).await;

    assert_eq!(
        (
            ends(&admin, &limited.run_id).await,
            after_limit,
            ends(&admin, &failed.run_id).await,
            after_failed,
            ends(&admin, &next.run_id).await
        ),
        (
            json!(["completed", "step_limit", first, second]),
            "quiet".to_owned(),
            json!(["refused", "failed", first, third]),
            "quiet".to_owned(),
            json!(["completed", "step_limit", first, fourth])
        )
    );
}
