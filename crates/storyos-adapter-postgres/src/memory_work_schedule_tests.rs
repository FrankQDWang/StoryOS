//! The schedule of a Project Conversation in the store (ADR 0094 section 5, writer 2): the due
//! write at the settlement of a Run of the author, the Worker claim with its row lock, and one
//! test for each row of the decision table. The tests move the stored due time with the test
//! administrator connection, and each test completes or parks each Run that it queued.

use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    ClaimedMemoryWork, ConversationSelection, MemoryCaptureSource, MemoryWorkOutcome,
    MemoryWorkScheduleStore, NoContractFaults, ProjectScope, complete_agent_run,
};
use storyos_core::ConversationCaptureSkip;
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{RuntimeClock, claim_run, stores};
use crate::agent_run_finish_rule::tests::complete;
use crate::agent_run_retrieval::tests::{adapter, destination, real_run};
use crate::agent_run_steering::tests::{cancellation, claim_duties, queued};
use crate::create_agent_run::context::chapter_list_tests::admitted;
use crate::memory_capture::tests::{project, run_capture};

/// The conversation of a source of a capture.
fn conversation(source: &MemoryCaptureSource) -> &str {
    match source {
        MemoryCaptureSource::Conversation {
            conversation_id, ..
        } => conversation_id,
        MemoryCaptureSource::Chapter { .. } => panic!("the source is a Conversation"),
    }
}

/// Whether the row of the Conversation is due in the quiet time from now: `None` when the
/// Conversation has no row, `Some(None)` when the due time is empty.
async fn due(admin: &Client, conversation_id: &str) -> Option<Option<bool>> {
    admin
        .query_opt(
            "SELECT due_at > clock_timestamp() + interval '29 minutes'
                AND due_at <= clock_timestamp() + interval '30 minutes'
               FROM storyos.memory_work_schedules WHERE conversation_id = $1::text::uuid",
            &[&conversation_id],
        )
        .await
        .unwrap()
        .map(|row| row.get(/*idx*/ 0))
}

/// Makes the row of the Conversation due now, and each other row not due, so that the claim
/// takes this row.
pub(crate) async fn make_due(admin: &Client, conversation_id: &str) {
    admin
        .execute(
            "UPDATE storyos.memory_work_schedules
                SET due_at = CASE WHEN conversation_id = $1::text::uuid
                                  THEN clock_timestamp() - interval '1 second' END
              WHERE conversation_id = $1::text::uuid OR due_at IS NOT NULL",
            &[&conversation_id],
        )
        .await
        .unwrap();
}

/// Claims the due row of the Conversation, and returns its outcome. The due time is then empty.
pub(crate) async fn claim_due(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    conversation_id: &str,
) -> MemoryWorkOutcome {
    make_due(admin, conversation_id).await;
    let claimed = store.claim_due_memory_work().await.unwrap().unwrap();
    assert_eq!(
        (&claimed.project_scope, claimed.conversation_id.as_str()),
        (scope, conversation_id)
    );
    assert_eq!(due(admin, conversation_id).await, Some(None));
    claimed.outcome
}

/// A Run of the author that completes sets the due time of its Conversation to the settlement
/// time and 30 minutes. A Run that is refused or cancelled does not change the row.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn only_the_completion_of_a_run_of_the_author_sets_the_due_time() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
/// claim after the decision finds no due row, so two Workers do not take the same row.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_due_row_starts_one_capture_and_a_locked_row_is_not_claimed() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
    let locked = store.claim_due_memory_work().await.unwrap();
    admin.batch_execute("ROLLBACK").await.unwrap();
    let first = store.claim_due_memory_work().await.unwrap();
    let second = store.claim_due_memory_work().await.unwrap();
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
            locked,
            serde_json::from_str::<serde_json::Value>(&capture).unwrap(),
            second
        ),
        (
            None,
            serde_json::json!([
                "conversation",
                conversation_id,
                first_run_id,
                last_run_id,
                true
            ]),
            None
        )
    );
}

/// Table rows 1 and 3: a Run of the author in the Conversation is queued, and then no completed
/// Run is after the last Run that a capture read.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_queued_run_of_the_author_and_read_messages_start_no_capture() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
