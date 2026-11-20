//! The schedule of a Project Conversation in the store (ADR 0094 section 5, writer 2): the due
//! write at the settlement of a Run of the author, the Worker claim with its row lock, and one
//! test for each row of the decision table. The tests move the stored due time with the test
//! administrator connection, and each test completes or parks each Run that it queued.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    ClaimedMemoryWork, ConversationSelection, MemoryCaptureSource, MemoryWorkOutcome,
    MemoryWorkScheduleStore, NoContractFaults, complete_agent_run,
};
use storyos_core::ConversationCaptureSkip;
use tokio_postgres::Client;

use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_retrieval::{adapter, destination, real_run};
use crate::shared_tests::agent_run_steering::{cancellation, claim_duties, queued};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::chapter_list_context::admitted;
use crate::shared_tests::memory_capture::{project, run_capture};
use crate::shared_tests::memory_capture_loop::{other_records, record, script, settlement};
use crate::shared_tests::memory_work_schedule::{claim_due, due, make_due};
use crate::shared_tests::model_gateway::NoEmbeddings;
use crate::shared_tests::model_gateway::{RuntimeClock, claim_run, stores};

/// The conversation of a source of a capture.
fn conversation(source: &MemoryCaptureSource) -> &str {
    match source {
        MemoryCaptureSource::Conversation {
            conversation_id, ..
        } => conversation_id,
        MemoryCaptureSource::Chapter { .. } => panic!("the source is a Conversation"),
    }
}

/// A Run of the author that completes sets the due time of its Conversation to the settlement
/// time and 30 minutes. A Run that is refused or cancelled does not change the row.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn only_the_completion_of_a_run_of_the_author_sets_the_due_time() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (_, _, completed) = project(&store, &admin, "290").await;

    let body = r#"{"error":{"code":"InvalidParameter","message":"The parameter is invalid."}}"#;
    let reply = format!(
        "HTTP/1.1 400 Bad Request\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\
         connection: close\r\n\r\n{body}",
        body.len()
    );
    let (base, served) = destination(vec![reply]).await;
    let (scope, refused) = real_run(&store, &admin, "291", &base).await;
    let claim = claim_run(&admin, scope, refused.clone()).await;
    let model = adapter();
    let ran = complete_agent_run(&store, &model, &NoContractFaults, &RuntimeClock, &claim);
    ran.await.unwrap();
    served.await.unwrap();

    let (scope, _, cancelled) = queued(&store, "292").await;
    let cancel = cancellation(&store, &scope, 0x5600, &cancelled).await;
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
    let of_run = |run_id: String| {
        let admin = &admin;
        async move {
            let row = admin
                .query_one(
                    "SELECT conversation_id::text, status FROM storyos.agent_runs
                      WHERE run_id = $1::text::uuid",
                    &[&run_id],
                )
                .await
                .unwrap();
            let conversation_id: String = row.get(/*idx*/ 0);
            (
                row.get::<_, String>(/*idx*/ 1),
                due(admin, &conversation_id).await,
            )
        }
    };

    assert_eq!(
        [
            (
                "completed".to_owned(),
                due(&admin, conversation(&completed)).await
            ),
            of_run(refused).await,
            of_run(cancelled).await,
        ],
        [
            ("completed".to_owned(), Some(Some(true))),
            ("refused".to_owned(), None),
            ("cancelled".to_owned(), None),
        ]
    );
}

/// A due row starts one capture. A claim skips a row that another claim locked, and the next
/// claim after the decision finds no due row, so two Workers do not take the same row. The due
/// check of the Worker sees the row before the claim and no row after it.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_due_row_starts_one_capture_and_a_locked_row_is_not_claimed() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "293").await;
    let conversation_id = conversation(&source);
    make_due(&admin, conversation_id).await;
    admin.batch_execute("BEGIN").await.unwrap();
    admin
        .execute(
            "SELECT 1 FROM storyos.memory_work_schedules
              WHERE conversation_id = $1::text::uuid FOR UPDATE",
            &[&conversation_id],
        )
        .await
        .unwrap();
    let due = store.memory_work_due().await.unwrap();
    let locked = store.claim_due_memory_work().await.unwrap();
    admin.batch_execute("ROLLBACK").await.unwrap();
    let first = store.claim_due_memory_work().await.unwrap();
    let second = store.claim_due_memory_work().await.unwrap();
    let due_after = store.memory_work_due().await.unwrap();
    let Some(ClaimedMemoryWork {
        outcome: MemoryWorkOutcome::Started(started),
        ..
    }) = &first
    else {
        panic!("the claim must start a capture: {first:?}");
    };
    let capture = admin
        .query_one(
            "SELECT jsonb_build_array(capture.source_kind, capture.conversation_id::text,
                      capture.first_run_id::text, capture.last_run_id::text,
                      capture.memory_settings_revision = run.memory_settings_revision)::text
               FROM storyos.memory_captures AS capture
               JOIN storyos.agent_runs AS run
                 ON (run.owner_user_id, run.project_id, run.run_id) =
                    (capture.owner_user_id, capture.project_id, capture.last_run_id)
              WHERE capture.capture_id = $1::text::uuid",
            &[&started.capture_id],
        )
        .await
        .unwrap()
        .get::<_, String>(/*idx*/ 0);
    run_capture(&store, &admin, &scope, &started.run_id).await;
    let MemoryCaptureSource::Conversation {
        first_run_id,
        last_run_id,
        ..
    } = &source
    else {
        unreachable!()
    };

    assert_eq!(
        (
            (due, locked),
            serde_json::from_str::<serde_json::Value>(&capture).unwrap(),
            (second, due_after)
        ),
        (
            (true, None),
            serde_json::json!([
                "conversation",
                conversation_id,
                first_run_id,
                last_run_id,
                true
            ]),
            (None, false)
        )
    );
}

/// Table rows 1 and 3: a Run of the author in the Conversation is queued, and then no completed
/// Run is after the last Run that a capture read.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_queued_run_of_the_author_and_read_messages_start_no_capture() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "294").await;
    let conversation_id = conversation(&source).to_owned();
    let MemoryWorkOutcome::Started(started) =
        claim_due(&store, &admin, &scope, &conversation_id).await
    else {
        panic!("the first claim must start a capture");
    };
    run_capture(&store, &admin, &scope, &started.run_id).await;
    let read = claim_due(&store, &admin, &scope, &conversation_id).await;
    let existing = ConversationSelection::Existing {
        conversation_id: conversation_id.clone(),
    };
    let next = admitted(&store, &scope, &chapter_id, "295", "继续写。", existing).await;
    let unsettled = claim_due(&store, &admin, &scope, &conversation_id).await;
    complete(&store, &admin, &scope, &next).await;

    assert_eq!(
        (read, unsettled, due(&admin, &conversation_id).await),
        (
            MemoryWorkOutcome::Skipped(ConversationCaptureSkip::NothingUnread),
            MemoryWorkOutcome::Skipped(ConversationCaptureSkip::AuthorRunUnsettled),
            Some(Some(true))
        )
    );
}

/// Table row 4: a capture of the Conversation is in progress.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_capture_in_progress_starts_no_second_capture() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "296").await;
    let conversation_id = conversation(&source);
    let MemoryWorkOutcome::Started(started) =
        claim_due(&store, &admin, &scope, conversation_id).await
    else {
        panic!("the first claim must start a capture");
    };
    let in_progress = claim_due(&store, &admin, &scope, conversation_id).await;
    run_capture(&store, &admin, &scope, &started.run_id).await;

    assert_eq!(
        in_progress,
        MemoryWorkOutcome::Skipped(ConversationCaptureSkip::CaptureInProgress)
    );
}

/// Table rows 2 and 5: the current Conversation Memory Settings do not permit contribution, and
/// the start refuses. No control for the settings exists, so SQL writes the new current
/// settings revision (default 15).
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_settings_and_a_refused_start_start_no_capture() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "297").await;
    let conversation_id = conversation(&source);
    let settings = |enabled: bool, revision: &'static str| {
        let admin = &admin;
        async move {
            admin
                .execute(
                    "UPDATE storyos.conversation_memory_settings SET is_current = false
                      WHERE conversation_id = $1::text::uuid AND COALESCE(is_current, true)",
                    &[&conversation_id],
                )
                .await
                .unwrap();
            admin
                .execute(
                    "INSERT INTO storyos.conversation_memory_settings
                       (owner_user_id, project_id, conversation_id, memory_settings_revision,
                        use_enabled, contribution_enabled, receipt_id)
                     SELECT owner_user_id, project_id, conversation_id, $2::text::uuid, true, $3,
                            receipt_id
                       FROM storyos.conversation_memory_settings
                      WHERE conversation_id = $1::text::uuid
                      LIMIT 1",
                    &[&conversation_id, &revision, &enabled],
                )
                .await
                .unwrap();
        }
    };
    settings(false, "018f0000-0000-7001-8000-000000560001").await;
    let not_permitted = claim_due(&store, &admin, &scope, conversation_id).await;
    settings(true, "018f0000-0000-7001-8000-000000560002").await;
    admin
        .execute(
            "UPDATE storyos.project_destination_grants
                SET grant_scope = 'project_writing_assistance_v4' WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let refused = claim_due(&store, &admin, &scope, conversation_id).await;
    let captures: i64 = admin
        .query_one(
            "SELECT count(*) FROM storyos.memory_captures WHERE conversation_id = $1::text::uuid",
            &[&conversation_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);

    assert_eq!(
        (not_permitted, refused, captures),
        (
            MemoryWorkOutcome::Skipped(ConversationCaptureSkip::ContributionNotPermitted),
            MemoryWorkOutcome::Skipped(ConversationCaptureSkip::StartRefused),
            0
        )
    );
}

/// The capture Run of the Conversation, and its capture row as its source, its first and its
/// last Run, and whether it records the settings revision of its last Run.
async fn capture_of(admin: &Client, conversation_id: &str) -> Option<(String, String)> {
    let row = admin
        .query_opt(
            "SELECT capture.run_id::text, jsonb_build_array(capture.source_kind,
                      capture.conversation_id::text, capture.first_run_id::text,
                      capture.last_run_id::text,
                      capture.memory_settings_revision = run.memory_settings_revision)::text
               FROM storyos.memory_captures AS capture
               JOIN storyos.agent_runs AS run
                 ON (run.owner_user_id, run.project_id, run.run_id) =
                    (capture.owner_user_id, capture.project_id, capture.last_run_id)
              WHERE capture.conversation_id = $1::text::uuid",
            &[&conversation_id],
        )
        .await
        .unwrap();
    row.map(|row| (row.get(/*idx*/ 0), row.get(/*idx*/ 1)))
}

/// Two completed Runs of a Conversation that permits contribution, on the fake destination, get
/// one capture through `run_once` after their due time. The observations of the fake script are
/// stored with the Conversation as their source, and no Memory Entry changes.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn run_once_captures_two_completed_runs_into_stored_observations() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "298").await;
    let MemoryCaptureSource::Conversation {
        conversation_id,
        first_run_id,
        ..
    } = &source
    else {
        unreachable!()
    };
    let observation = json!({"type": "character", "title": "林舟", "text": "沉默的船夫。"});
    let message = format!("记住林舟。\n{}", script(&[record(json!([observation]))]));
    let existing = ConversationSelection::Existing {
        conversation_id: conversation_id.clone(),
    };
    let second = admitted(&store, &scope, &chapter_id, "299", &message, existing).await;
    complete(&store, &admin, &scope, &second).await;
    let before = other_records(&admin, &scope).await;
    make_due(&admin, conversation_id).await;
    let destination = storyos_worker::ModelDestination {
        adapter: FakeDestination,
        observer: NoContractFaults,
        tools: storyos_application::NoToolDestination,
        embeddings: NoEmbeddings,
    };
    // One step claims the row and starts the capture. The next steps claim its Run, also after
    // work that other tests left.
    let mut captured: Option<(String, String)> = None;
    for _ in 0..16 {
        storyos_worker::run_once(&store, &destination)
            .await
            .unwrap();
        captured = capture_of(&admin, conversation_id).await;
        if let Some((run_id, _)) = &captured
            && settlement(&admin, run_id).await[0] == "completed"
        {
            break;
        }
    }
    let (run_id, row) = captured.expect("run_once must start one capture");

    assert_eq!(
        (
            serde_json::from_str::<Value>(&row).unwrap(),
            settlement(&admin, &run_id).await,
            other_records(&admin, &scope).await
        ),
        (
            json!(["conversation", conversation_id, first_run_id, second, true]),
            json!([
                "completed",
                "settled",
                "observations_recorded",
                null,
                [[1, "character", "林舟", "沉默的船夫。", "waiting"]]
            ]),
            before
        )
    );
}
