//! The schedule of a Chapter in the store (ADR 0094 section 5, writer 3): one test for the rows
//! of the decision table, the order and the rate of three due Chapters, the order under a
//! concurrent lock, and the input and its excerpt. The tests move the stored due times and capture starts with the test administrator
//! connection, and each test completes or parks each Run that it queued.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    ChapterId, ClaimedMemoryWork, CreateChapterInput, DeleteChapterInput, DiagnosticField,
    EditorSessionStore, MemoryWorkOutcome, MemoryWorkScheduleStore, MemoryWorkSource,
    NoContractFaults, StartedMemoryCapture, complete_agent_run,
};
use storyos_core::{CreateChapterPlacement, TransitionOutcome};

use crate::shared_tests::agent_run_steering::{cancellation, claim_duties, queued};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::chapter_capture_schedule::{
    Fixture, due_state, fixture, input_of, make_chapter_due, row,
};
use crate::shared_tests::chapter_list_context::head;
use crate::shared_tests::command_sequence_structure::{
    CREATE_CHAPTER, DELETE_CHAPTER, create_chapter, delete_chapter,
};
use crate::shared_tests::command_sequence_support::issued;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::memory_capture_loop::{record, script};
use crate::shared_tests::model_gateway::NoEmbeddings;
use crate::shared_tests::model_gateway::assisted_chapter;
use crate::shared_tests::project_research_binding::park_queued_runs;
use crate::shared_tests::set_current_chapter_authority::open_session_request;

/// Each row of the decision table of S7-D5 for one claimed row, in the order of the table. The
/// settlement of a completed or a failed capture that read an earlier Revision sets the due time
/// again, and a `held` row stays `held` after a commit. The test moves the capture starts back before the rows that
/// come after the rate.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_claimed_chapter_row_follows_the_rows_of_the_decision_table() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
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

/// A Project with no completed Run of the author gets no background work: a Project with no Run, a
/// Project whose one Run is queued, and a Project whose one Run was cancelled hold the row of a
/// changed Chapter. None gets a capture, sends a request, or writes a grant. The request log of the
/// fake destination is the stored requests of the Project: the store writes each request to a
/// destination before it sends it.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_project_with_no_completed_run_holds_the_row() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let mut projects = Vec::new();
    let cases = [
        ("89f", 0x89e4, Runs::None),
        ("836", 0x83d0, Runs::Queued),
        ("89a", 0x89e0, Runs::Cancelled),
    ];
    for (lead, suffix, runs) in cases {
        let (store, admin) = stores().await;
        let (scope, chapter_id, run_id) = match runs {
            Runs::None => {
                let (scope, chapter_id) = assisted_chapter(&store, lead).await;
                (scope, chapter_id, String::new())
            }
            Runs::Queued | Runs::Cancelled => queued(&store, lead).await,
        };
        if runs == Runs::Cancelled {
            let cancel = cancellation(&store, &scope, suffix + 2, &run_id).await;
            let cancelled = store.cancel_agent_run(&cancel.envelope, &cancel.input);
            cancelled.await.unwrap();
            if let Some(claim) = claim_duties(&admin, &scope, &run_id).await {
                let clock = &crate::shared_tests::model_gateway::RuntimeClock;
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
                                (SELECT coalesce(jsonb_agg(payload #> '{wire,serialized_payload}'), '[]')
                                   FROM storyos.model_attempts WHERE project_id = $1::text::uuid)
                                || (SELECT coalesce(jsonb_agg(cause), '[]')
                                      FROM storyos.embedding_attempts WHERE project_id = $1::text::uuid),
                                (SELECT count(*) FROM storyos.project_destination_grants
                                  WHERE project_id = $1::text::uuid))::text";
            let project_id = f.scope.project_id.as_ref();
            json_of(&f.admin, counts, &[&project_id]).await
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
        json!([]),
        true,
    );
    assert_eq!(projects, [held.clone(), held.clone(), held]);
}

/// The Runs of a Project before its Chapter changes.
#[derive(Clone, Copy, PartialEq)]
enum Runs {
    None,
    Queued,
    Cancelled,
}

/// Three due Chapters start one capture each, the first in Chapter order first, and not in the
/// order of their due times. A claim within 2 minutes of the last start moves the claimed row to
/// the time that the rate permits, and the other row stays due. The test parks each capture Run
/// and moves the starts back after each start.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn three_due_chapters_start_in_chapter_order_at_the_rate_of_the_project() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
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
        embeddings: NoEmbeddings,
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
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
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

/// The input of a Chapter capture is the title, the number in the Chapter list, and the text at
/// the pinned Revision. A text above the Context item limit gives a Deterministic Excerpt.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_input_is_the_title_the_number_and_the_text_with_its_excerpt() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
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
