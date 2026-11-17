//! The schedule of a Chapter in the store (ADR 0094 section 5, writer 3): one test for the rows
//! of the decision table, the order and the rate of three due Chapters, the order under a
//! concurrent lock, and the input and its excerpt. The tests move the stored due times and capture starts with the test administrator
//! connection, and each test completes or parks each Run that it queued.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    ChapterId, ClaimedMemoryWork, CreateChapterInput, DeleteChapterInput, DiagnosticField,
    EditorSessionId, EditorSessionStore, MemoryCaptureSource, MemoryCaptureStore,
    MemoryWorkOutcome, MemoryWorkScheduleStore, MemoryWorkSource, NoContractFaults, ProjectScope,
    StartedMemoryCapture, complete_agent_run,
};
use storyos_core::{CreateChapterPlacement, TransitionOutcome};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::stores;
use crate::agent_run_read_chapter::tests::{project, write_blocks};
use crate::agent_run_steering::tests::{cancellation, claim_duties, queued};
use crate::command_sequence::tests::{
    CREATE_CHAPTER, DELETE_CHAPTER, create_chapter, delete_chapter, issued,
};
use crate::create_agent_run::context::chapter_list_tests::{head, select};
use crate::memory_capture::loop_tests::{record, script};
use crate::memory_capture::tests::run_capture;
use crate::project_research_binding::tests::park_queued_runs;
use crate::set_current_chapter_authority_tests::open_session_request;

/// Makes the row of the Chapter due `seconds` ago, and each other row not due, so that the
/// claim takes this row. A Chapter with no commit gets its row.
async fn make_chapter_due(admin: &Client, scope: &ProjectScope, chapter_id: &str, seconds: i32) {
    let keys = [scope.owner_user_id.as_ref(), scope.project_id.as_ref()];
    let others = "UPDATE storyos.memory_work_schedules SET due_at = NULL
                   WHERE due_at IS NOT NULL AND project_id <> $1::text::uuid";
    admin.execute(others, &[&keys[1]]).await.unwrap();
    admin
        .execute(
            "INSERT INTO storyos.memory_work_schedules
               (owner_user_id, project_id, work_kind, source_id, chapter_id, due_at)
             VALUES ($1::text::uuid, $2::text::uuid, 'chapter_capture', $3::text::uuid,
                     $3::text::uuid, clock_timestamp() - make_interval(secs => $4))
             ON CONFLICT (owner_user_id, project_id, work_kind, source_id)
             DO UPDATE SET due_at = EXCLUDED.due_at, held = false",
            &[&keys[0], &keys[1], &chapter_id, &f64::from(seconds)],
        )
        .await
        .unwrap();
}

/// The row of the Chapter: `held`, and its due time in seconds after the start of the last Chapter
/// capture of the Project, or `null`.
async fn row(admin: &Client, chapter_id: &str) -> Value {
    admin
        .query_one(
            "SELECT jsonb_build_array(work.held,
                      extract(epoch FROM work.due_at - (
                        SELECT max(capture.started_at) FROM storyos.memory_captures AS capture
                         WHERE capture.project_id = work.project_id
                           AND capture.source_kind = 'chapter'))::bigint)::text
               FROM storyos.memory_work_schedules AS work
              WHERE work.work_kind = 'chapter_capture' AND work.source_id = $1::text::uuid",
            &[&chapter_id],
        )
        .await
        .map(|row| serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
        .unwrap()
}

/// Whether the row of the Chapter is due now, due later, or not due.
async fn due_state(admin: &Client, chapter_id: &str) -> Value {
    admin
        .query_one(
            "SELECT CASE WHEN due_at <= clock_timestamp() THEN 'due'
                         WHEN due_at IS NOT NULL THEN 'later' END
               FROM storyos.memory_work_schedules
              WHERE work_kind = 'chapter_capture' AND source_id = $1::text::uuid",
            &[&chapter_id],
        )
        .await
        .map(|row| json!(row.get::<_, Option<String>>(/*idx*/ 0)))
        .unwrap()
}

/// One test Project: its stores, its Scope, and its writer Editor Session.
struct Fixture {
    store: PostgresProjectReader,
    admin: Client,
    scope: ProjectScope,
    session: String,
}

impl Fixture {
    /// Claims the due row of the Chapter. Returns the label of the outcome and the row after it.
    async fn decide(&self, chapter_id: &str) -> (&'static str, Value) {
        make_chapter_due(&self.admin, &self.scope, chapter_id, /*seconds*/ 1).await;
        let claimed = self.store.claim_due_memory_work().await.unwrap().unwrap();
        let source = MemoryWorkSource::Chapter(chapter_id.to_owned());
        assert_eq!(
            (&claimed.project_scope, &claimed.source),
            (&self.scope, &source)
        );
        park_queued_runs(&self.admin, &self.scope).await;
        (
            claimed.outcome.diagnostic(),
            row(&self.admin, chapter_id).await,
        )
    }

    /// Moves each capture start of the Project 3 minutes back, so that the rate permits a start.
    async fn move_starts_back(&self) {
        let moved =
            "UPDATE storyos.memory_captures SET started_at = started_at - interval '3 minutes'
                      WHERE project_id = $1::text::uuid";
        let project_id = self.scope.project_id.as_ref();
        self.admin.execute(moved, &[&project_id]).await.unwrap();
    }

    /// One Author Edit with the local intent `sequence` and the challenge `suffix` that writes
    /// `sentences` at the start of the first Block of `chapter_id`, one Block each.
    async fn edit(&self, (sequence, suffix): (u64, u16), chapter_id: &str, sentences: &[&str]) {
        let suffix = format!("{suffix:04x}");
        let edit = (self.session.as_str(), sequence, suffix.as_str());
        let (store, admin, scope) = (&self.store, &self.admin, &self.scope);
        let write = write_blocks(store, admin, scope, edit, chapter_id, sentences).await;
        store.apply_author_edit(&write).await.unwrap();
    }

    /// Selects `chapter_id` as the Current Chapter after `from`, so that the next edit can
    /// change it.
    async fn select(&self, suffix: u16, chapter_id: &str, from: &str) {
        let writer = EditorSessionId::new(self.session.as_str());
        let (store, admin, scope) = (&self.store, &self.admin, &self.scope);
        let call = select(store, admin, scope, (&writer, suffix), chapter_id, from).await;
        store
            .set_current_chapter(&call.envelope, &call.input)
            .await
            .unwrap();
    }

    /// Starts one capture of the Chapter at its head with no observation.
    async fn start_capture(&self, chapter_id: &str) -> StartedMemoryCapture {
        let source = MemoryCaptureSource::Chapter {
            chapter_id: chapter_id.to_owned(),
            revision_id: head(&self.admin, chapter_id).await.0,
        };
        let input = script(&[record(json!([]))]);
        let started = self
            .store
            .start_memory_capture(&self.scope, &source, &input);
        started.await.unwrap().unwrap()
    }

    async fn run_capture(&self, started: &StartedMemoryCapture) {
        run_capture(&self.store, &self.admin, &self.scope, &started.run_id).await;
    }
}

/// A Project of `lead` whose Chapter A has the Blocks `sentences`, with a second empty Chapter
/// `Fog` and an open Editor Session. `suffix` to `suffix + 2` are its challenges.
async fn fixture(lead: &str, suffix: u16, sentences: &[&str]) -> (Fixture, String, String) {
    let (store, admin) = stores().await;
    let (scope, chapter_a, fog, session) = project(&store, &admin, lead, suffix, sentences).await;
    let fixture = Fixture {
        store,
        admin,
        scope,
        session,
    };
    (fixture, chapter_a, fog)
}

/// Each row of the decision table of S7-D5 for one claimed row, in the order of the table. The
/// settlement of a completed or a failed capture that read an earlier Revision sets the due time
/// again, and a `held` row stays `held` after a commit. The test moves the capture starts back before the rows that
/// come after the rate.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_claimed_chapter_row_follows_the_rows_of_the_decision_table() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (f, chapter_a, fog) = fixture("832", 0x8390, &["雨夜。"]).await;
    let mut decisions = vec![f.decide(&fog).await];
    let project_id = f.scope.project_id.as_ref();
    // A grant that does not name the Purpose fails the next request of a capture, and refuses a
    // start.
    let grant = async |version: &str| {
        let grant = "UPDATE storyos.project_destination_grants
                        SET grant_scope = 'project_writing_assistance_' || $2
                      WHERE project_id = $1::text::uuid";
        f.admin
            .execute(grant, &[&project_id, &version])
            .await
            .unwrap();
    };

    // A completed capture and a failed capture.
    let mut settled = Vec::new();
    for (version, sequence, suffix) in [("v5", 2, 0x8393), ("v4", 3, 0x8397)] {
        let capture = f.start_capture(&chapter_a).await;
        f.edit((sequence, suffix), &chapter_a, &["风。"]).await;
        decisions.push(f.decide(&chapter_a).await);
        grant(version).await;
        f.run_capture(&capture).await;
        grant("v5").await;
        settled.push(row(&f.admin, &chapter_a).await);
    }

    let second = f.start_capture(&chapter_a).await;
    f.run_capture(&second).await;
    decisions.push(f.decide(&chapter_a).await);
    f.edit((4, 0x8394), &chapter_a, &["霜。"]).await;
    decisions.push(f.decide(&chapter_a).await);

    f.move_starts_back().await;
    grant("v4").await;
    decisions.push(f.decide(&chapter_a).await);
    f.edit((5, 0x8395), &chapter_a, &["雪。"]).await;
    let held_after_commit = row(&f.admin, &chapter_a).await;

    let input = DeleteChapterInput {
        chapter_id: ChapterId::new(fog.as_str()),
        expected_tree_revision: 4,
    };
    let call = issued(&f.store, &f.scope, 0x8396, &DELETE_CHAPTER, input).await;
    delete_chapter(&f.store, &call).await.unwrap();
    decisions.push(f.decide(&fog).await);
    let count = "SELECT count(*) FROM storyos.memory_captures WHERE project_id = $1::text::uuid";
    let captures: i64 = f
        .admin
        .query_one(count, &[&project_id])
        .await
        .unwrap()
        .get(/*idx*/ 0);

    assert_eq!(
        (decisions, Value::from(settled), held_after_commit, captures),
        (
            vec![
                ("text_empty", json!([false, null])),
                ("capture_in_progress", json!([false, null])),
                ("capture_in_progress", json!([false, null])),
                ("revision_read", json!([false, null])),
                ("rate_wait", json!([false, 120])),
                ("start_refused", json!([true, null])),
                ("chapter_not_live", json!([false, null])),
            ],
            json!([[false, 21_600], [false, 21_600]]),
            json!([true, null]),
            3
        )
    );
}

/// A Project with no completed Run of the author gets no background work: a Project whose one Run
/// is queued and a Project whose one Run was cancelled hold the row of a changed Chapter. Neither
/// gets a capture, sends a request, or writes a grant.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_project_with_no_completed_run_holds_the_row() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let mut projects = Vec::new();
    for (lead, suffix, cancel) in [("836", 0x83d0, false), ("89a", 0x89e0, true)] {
        let (store, admin) = stores().await;
        let (scope, chapter_id, run_id) = queued(&store, lead).await;
        if cancel {
            let cancel = cancellation(&store, &scope, suffix + 2, &run_id).await;
            let cancelled = store.cancel_agent_run(&cancel.envelope, &cancel.input);
            cancelled.await.unwrap();
            if let Some(claim) = claim_duties(&admin, &scope, &run_id).await {
                let clock = &crate::agent_run_dispatch::tests::RuntimeClock;
                let destination = (&FakeDestination, &NoContractFaults);
                let duties =
                    complete_agent_run(&store, destination.0, destination.1, clock, &claim);
                duties.await.unwrap();
            }
        }
        park_queued_runs(&admin, &scope).await;
        let request = open_session_request(&store, &scope, &format!("{suffix:04x}")).await;
        store.create_editor_session(&request).await.unwrap();
        let session = request.editor_session_id.as_ref().to_owned();
        let f = Fixture {
            store,
            admin,
            scope,
            session,
        };
        let counts = async || {
            let counts = "SELECT jsonb_build_array(
                                (SELECT count(*) FROM storyos.memory_captures WHERE project_id = $1::text::uuid),
                                (SELECT count(*) FROM storyos.model_attempts WHERE project_id = $1::text::uuid),
                                (SELECT count(*) FROM storyos.project_destination_grants
                                  WHERE project_id = $1::text::uuid))::text";
            let project_id = f.scope.project_id.as_ref();
            let row = f.admin.query_one(counts, &[&project_id]).await.unwrap();
            serde_json::from_str::<Value>(row.get(/*idx*/ 0)).unwrap()
        };
        let before = counts().await;
        f.edit((1, suffix + 1), &chapter_id, &["雨。"]).await;
        let decision = f.decide(&chapter_id).await;
        let after = counts().await;
        projects.push((
            decision,
            after[0].clone(),
            after[1].clone(),
            before[2] == after[2],
        ));
    }

    let held = (
        ("no_completed_run", json!([true, null])),
        json!(0),
        json!(0),
        true,
    );
    assert_eq!(projects, [held.clone(), held]);
}

/// Three due Chapters start one capture each, the first in Chapter order first, and not in the
/// order of their due times. A claim within 2 minutes of the last start moves the claimed row to
/// the time that the rate permits, and the other row stays due. The test parks each capture Run
/// and moves the starts back after each start.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn three_due_chapters_start_in_chapter_order_at_the_rate_of_the_project() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (f, chapter_a, fog) = fixture("833", 0x83a0, &["雨。"]).await;
    let (store, admin, scope) = (&f.store, &f.admin, &f.scope);
    f.select(0x83a6, &fog, &chapter_a).await;
    f.edit((2, 0x83a3), &fog, &["雾。"]).await;
    let parent = "SELECT parent_volume_id::text FROM storyos.manuscript_objects
                   WHERE manuscript_object_id = $1::text::uuid";
    let input = CreateChapterInput {
        volume_id: admin
            .query_one(parent, &[&chapter_a])
            .await
            .unwrap()
            .get(/*idx*/ 0),
        title: "Ice".to_owned(),
        placement: CreateChapterPlacement::Append,
        expected_tree_revision: 4,
    };
    let call = issued(store, scope, 0x83a4, &CREATE_CHAPTER, input).await;
    let TransitionOutcome::Applied(created) = create_chapter(store, &call).await.unwrap().outcome
    else {
        panic!("Ice must be created");
    };
    let ice = created.effect.chapter_id;
    f.select(0x83a7, &ice, &fog).await;
    f.edit((3, 0x83a5), &ice, &["冰。"]).await;
    // The due times are in the reverse of the Chapter order.
    for (seconds, chapter_id) in [(1, &chapter_a), (2, &fog), (3, &ice)] {
        make_chapter_due(admin, scope, chapter_id, seconds).await;
    }
    let state = async || {
        let titles = "SELECT chapter.title FROM storyos.memory_captures AS capture
                        JOIN storyos.manuscript_objects AS chapter
                          ON chapter.manuscript_object_id = capture.chapter_id
                       WHERE capture.project_id = $1::text::uuid ORDER BY capture.capture_id";
        let project_id = scope.project_id.as_ref();
        let titles = admin.query(titles, &[&project_id]).await.unwrap();
        let titles: Vec<String> = titles.iter().map(|row| row.get(/*idx*/ 0)).collect();
        let mut rows = Vec::new();
        for chapter_id in [&chapter_a, &fog, &ice] {
            rows.push(due_state(admin, chapter_id).await);
        }
        json!([titles, rows])
    };
    let destination = storyos_worker::ModelDestination {
        adapter: FakeDestination,
        observer: NoContractFaults,
        tools: storyos_application::NoToolDestination,
        embeddings: storyos_application::NoEmbeddingDestination,
    };
    // A step can claim work that other tests left. The test records the state of its Project
    // only when a capture starts.
    let (mut states, mut rate) = (Vec::new(), None);
    for _ in 0..24 {
        storyos_worker::run_once(store, &destination).await.unwrap();
        let now = state().await;
        if states.last() == Some(&now) || now[0] == json!([]) {
            continue;
        }
        park_queued_runs(admin, scope).await;
        states.push(now);
        if rate.is_none() {
            let claimed = store.claim_due_memory_work().await.unwrap();
            let outcome = claimed.map(|claimed| claimed.outcome.diagnostic());
            rate = Some((outcome, state().await));
            make_chapter_due(admin, scope, &fog, /*seconds*/ 2).await;
            make_chapter_due(admin, scope, &ice, /*seconds*/ 3).await;
        }
        f.move_starts_back().await;
        if states.len() == 3 {
            break;
        }
    }

    assert_eq!(
        (Value::from(states), rate),
        (
            json!([
                [["Chapter A"], [null, "due", "due"]],
                [["Chapter A", "Fog"], [null, null, "due"]],
                [["Chapter A", "Fog", "Ice"], [null, null, null]]
            ]),
            Some((
                Some("rate_wait"),
                json!([["Chapter A"], [null, "later", "due"]])
            ))
        )
    );
}

/// A row of an earlier Chapter that a concurrent transaction locked stays first in Chapter order:
/// the claim takes the row of the later Chapter, waits for the lock, and starts the capture of
/// the earlier Chapter. The row of the later Chapter stays due.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_locked_row_of_an_earlier_chapter_stays_first_in_order() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (mut f, chapter_a, fog) = fixture("835", 0x83c0, &["雨。"]).await;
    f.select(0x83c4, &fog, &chapter_a).await;
    f.edit((2, 0x83c3), &fog, &["雾。"]).await;
    make_chapter_due(&f.admin, &f.scope, &chapter_a, /*seconds*/ 2).await;
    make_chapter_due(&f.admin, &f.scope, &fog, /*seconds*/ 1).await;
    let lock = f.admin.transaction().await.unwrap();
    let locked = "SELECT 1 FROM storyos.memory_work_schedules
                   WHERE source_id = $1::text::uuid FOR UPDATE";
    lock.execute(locked, &[&chapter_a]).await.unwrap();
    let release = async move {
        let waiting = "SELECT EXISTS (SELECT 1 FROM pg_locks AS waiting JOIN pg_locks AS held
                                        ON held.transactionid = waiting.transactionid
                                     WHERE NOT waiting.granted AND held.pid = pg_backend_pid())";
        let wait = async {
            while !lock
                .query_one(waiting, &[])
                .await
                .unwrap()
                .get::<_, bool>(/*idx*/ 0)
            {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        };
        let timeout = std::time::Duration::from_secs(10);
        tokio::time::timeout(timeout, wait)
            .await
            .expect("the claim must wait");
        lock.rollback().await.unwrap();
    };
    let (claimed, ()) = tokio::join!(f.store.claim_due_memory_work(), release);
    let claimed = claimed.unwrap().unwrap();
    park_queued_runs(&f.admin, &f.scope).await;
    let fog_state = due_state(&f.admin, &fog).await;
    let cleared = "UPDATE storyos.memory_work_schedules SET due_at = NULL
                    WHERE source_id = $1::text::uuid";
    f.admin.execute(cleared, &[&fog]).await.unwrap();

    assert_eq!(
        (claimed.source, claimed.outcome.diagnostic(), fog_state),
        (
            MemoryWorkSource::Chapter(chapter_a),
            "capture_started",
            json!("due")
        )
    );
}

#[path = "chapter_capture_release_tests.rs"]
mod release_tests;

/// The context item of the first RunStep of the capture Run `run_id`: its projection mode and its
/// content. The capture row has the Chapter and the head Revision of the Chapter.
async fn input_of(admin: &Client, run_id: &str, chapter_id: &str) -> (String, String) {
    let row = admin
        .query_one(
            "SELECT manifest.payload #>> '{selected,0,projection_mode}',
                    manifest.payload #>> '{selected,0,content}',
                    capture.chapter_revision_id = head.current_revision_id
               FROM storyos.context_assembly_manifests AS manifest
               JOIN storyos.memory_captures AS capture USING (owner_user_id, project_id, run_id)
               JOIN storyos.authoritative_heads AS head
                 ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                    (capture.owner_user_id, capture.project_id, capture.chapter_id)
              WHERE manifest.run_id = $1::text::uuid AND manifest.manifest_role = 'decision'
                AND manifest.decision_position = 0 AND capture.chapter_id = $2::text::uuid",
            &[&run_id, &chapter_id],
        )
        .await
        .unwrap();
    assert!(row.get::<_, bool>(/*idx*/ 2));
    (row.get(/*idx*/ 0), row.get(/*idx*/ 1))
}

/// The input of a Chapter capture is the title, the number in the Chapter list, and the text at
/// the pinned Revision. A text above the Context item limit gives a Deterministic Excerpt.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_input_is_the_title_the_number_and_the_text_with_its_excerpt() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let sentences = ["雨夜。", "林舟撑船。"];
    let (f, chapter_a, fog) = fixture("834", 0x83b0, &sentences).await;
    let half = usize::try_from(storyos_core::CONTEXT_ITEM_TOKEN_LIMIT).unwrap() / 2 + 1_000;
    let long = ["雾".repeat(half), "霜".repeat(half)];
    let long = [long[0].as_str(), long[1].as_str()];
    f.select(0x83b4, &fog, &chapter_a).await;
    f.edit((2, 0x83b3), &fog, &long).await;
    let mut inputs = Vec::new();
    for chapter_id in [&chapter_a, &fog] {
        make_chapter_due(&f.admin, &f.scope, chapter_id, /*seconds*/ 1).await;
        let claimed = f.store.claim_due_memory_work().await.unwrap();
        let Some(ClaimedMemoryWork {
            outcome: MemoryWorkOutcome::Started(started),
            ..
        }) = claimed
        else {
            panic!("the claim must start a capture: {claimed:?}");
        };
        park_queued_runs(&f.admin, &f.scope).await;
        f.move_starts_back().await;
        inputs.push(input_of(&f.admin, &started.run_id, chapter_id).await);
    }
    let expected = |mode: &str, title: &str, number: usize, blocks: &[&str]| {
        let blocks: Vec<String> = blocks.iter().map(|block| (*block).to_owned()).collect();
        let limit = storyos_core::CONTEXT_ITEM_TOKEN_LIMIT;
        let text = storyos_core::chapter_capture_input(title, number, &blocks, limit).text;
        (mode.to_owned(), text)
    };

    assert_eq!(
        inputs,
        [
            expected("exact_required", "Chapter A", 1, &sentences),
            expected("deterministic_excerpt", "Fog", 2, &long)
        ]
    );
}
