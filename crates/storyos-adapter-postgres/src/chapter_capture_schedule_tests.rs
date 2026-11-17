//! The schedule of a Chapter in the store (ADR 0094 section 5, writer 3): one test for the rows
//! of the decision table, the order and the rate of three due Chapters, the input and its excerpt,
//! and the path from a changed Chapter to an entry through `run_once`. The tests move the stored
//! due times and capture starts with the test administrator connection, and each test completes
//! or parks each Run that it queued.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    ChapterId, ClaimedMemoryWork, CreateChapterInput, DeleteChapterInput, DiagnosticField,
    EditorSessionId, EditorSessionStore, MemoryCaptureSource, MemoryCaptureStore, MemoryEntrySourceItem,
    MemoryWorkOutcome, MemoryWorkScheduleStore, MemoryWorkSource, NoContractFaults, ProjectScope,
    StartedMemoryCapture,
};
use storyos_core::{CreateChapterPlacement, MemoryEntryOrigin, TransitionOutcome};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::stores;
use crate::agent_run_read_chapter::tests::{project, write_blocks};
use crate::agent_run_steering::tests::queued;
use crate::command_sequence::tests::{
    CREATE_CHAPTER, DELETE_CHAPTER, create_chapter, delete_chapter, issued,
};
use crate::create_agent_run::context::chapter_list_tests::{head, select};
use crate::memory_capture::loop_tests::{record, script};
use crate::memory_capture::tests::run_capture;
use crate::memory_consolidation::tests::{create, scripted};
use crate::project_research_binding::tests::park_queued_runs;
use crate::set_current_chapter_authority_tests::open_session_request;

/// Makes the row of the Chapter due `seconds` ago, and each other row not due, so that the
/// claim takes this row. A Chapter with no commit gets its row.
async fn make_chapter_due(admin: &Client, scope: &ProjectScope, chapter_id: &str, seconds: i32) {
    admin
        .execute(
            "UPDATE storyos.memory_work_schedules SET due_at = NULL
              WHERE due_at IS NOT NULL AND project_id <> $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    admin
        .execute(
            "INSERT INTO storyos.memory_work_schedules
               (owner_user_id, project_id, work_kind, source_id, chapter_id, due_at)
             VALUES ($1::text::uuid, $2::text::uuid, 'chapter_capture', $3::text::uuid,
                     $3::text::uuid, clock_timestamp() - make_interval(secs => $4))
             ON CONFLICT (owner_user_id, project_id, work_kind, source_id)
             DO UPDATE SET due_at = EXCLUDED.due_at, held = false",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &chapter_id,
                &f64::from(seconds),
            ],
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

/// Claims the due row of the Chapter. Returns the label of the outcome and the row after it.
async fn decide(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    chapter_id: &str,
) -> (&'static str, Value) {
    make_chapter_due(admin, scope, chapter_id, /*seconds*/ 1).await;
    let claimed = store.claim_due_memory_work().await.unwrap().unwrap();
    assert_eq!(
        (&claimed.project_scope, &claimed.source),
        (scope, &MemoryWorkSource::Chapter(chapter_id.to_owned()))
    );
    park_queued_runs(admin, scope).await;
    (claimed.outcome.diagnostic(), row(admin, chapter_id).await)
}

/// Moves each capture start of the Project 3 minutes back, so that the rate permits a start.
async fn move_starts_back(admin: &Client, scope: &ProjectScope) {
    admin
        .execute(
            "UPDATE storyos.memory_captures SET started_at = started_at - interval '3 minutes'
              WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
}

/// One Author Edit of the Editor Session `session` that writes `sentences` at the start of the
/// first Block of `chapter_id`, one Block each.
async fn edit(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    (session, sequence, suffix): (&str, u64, u16),
    chapter_id: &str,
    sentences: &[&str],
) {
    let suffix = format!("{suffix:04x}");
    let edit = (session, sequence, suffix.as_str());
    let write = write_blocks(store, admin, scope, edit, chapter_id, sentences).await;
    store.apply_author_edit(&write).await.unwrap();
}

/// The writer Editor Session `session` selects `chapter_id` as the Current Chapter after `from`,
/// so that the next edit can change it.
async fn select_chapter(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    (session, suffix): (&str, u16),
    chapter_id: &str,
    from: &str,
) {
    let writer = EditorSessionId::new(session);
    let call = select(store, admin, scope, (&writer, suffix), chapter_id, from).await;
    store
        .set_current_chapter(&call.envelope, &call.input)
        .await
        .unwrap();
}

/// Starts one capture of the Chapter at its head with no observation.
async fn start_capture(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    chapter_id: &str,
) -> StartedMemoryCapture {
    let source = MemoryCaptureSource::Chapter {
        chapter_id: chapter_id.to_owned(),
        revision_id: head(admin, chapter_id).await.0,
    };
    let input = script(&[record(json!([]))]);
    store
        .start_memory_capture(scope, &source, &input)
        .await
        .unwrap()
        .unwrap()
}

/// Each row of the decision table of S7-D5 for one claimed row, in the order of the table. The
/// settlement of a capture that read an earlier Revision sets the due time again, and a `held`
/// row stays `held` after a commit. The test moves the capture starts back before the rows that
/// come after the rate.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_claimed_chapter_row_follows_the_rows_of_the_decision_table() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_a, fog, session) = project(&store, &admin, "832", 0x8390, &["雨夜。"]).await;
    let mut decisions = vec![decide(&store, &admin, &scope, &fog).await];

    let first = start_capture(&store, &admin, &scope, &chapter_a).await;
    edit(&store, &admin, &scope, (&session, 2, 0x8393), &chapter_a, &["风。"]).await;
    decisions.push(decide(&store, &admin, &scope, &chapter_a).await);
    run_capture(&store, &admin, &scope, &first.run_id).await;
    let after_settlement = row(&admin, &chapter_a).await;

    let second = start_capture(&store, &admin, &scope, &chapter_a).await;
    run_capture(&store, &admin, &scope, &second.run_id).await;
    decisions.push(decide(&store, &admin, &scope, &chapter_a).await);
    edit(&store, &admin, &scope, (&session, 3, 0x8394), &chapter_a, &["霜。"]).await;
    decisions.push(decide(&store, &admin, &scope, &chapter_a).await);

    move_starts_back(&admin, &scope).await;
    let grant = "UPDATE storyos.project_destination_grants
                    SET grant_scope = 'project_writing_assistance_v4' WHERE project_id = $1::text::uuid";
    admin
        .execute(grant, &[&scope.project_id.as_ref()])
        .await
        .unwrap();
    decisions.push(decide(&store, &admin, &scope, &chapter_a).await);
    edit(&store, &admin, &scope, (&session, 4, 0x8395), &chapter_a, &["雪。"]).await;
    let held_after_commit = row(&admin, &chapter_a).await;

    let input = DeleteChapterInput {
        chapter_id: ChapterId::new(fog.as_str()),
        expected_tree_revision: 4,
    };
    let call = issued(&store, &scope, 0x8396, &DELETE_CHAPTER, input).await;
    delete_chapter(&store, &call).await.unwrap();
    decisions.push(decide(&store, &admin, &scope, &fog).await);
    let captures: i64 = admin
        .query_one(
            "SELECT count(*) FROM storyos.memory_captures WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);

    assert_eq!(
        (decisions, after_settlement, held_after_commit, captures),
        (
            vec![
                ("text_empty", json!([false, null])),
                ("capture_in_progress", json!([false, null])),
                ("revision_read", json!([false, null])),
                ("rate_wait", json!([false, 120])),
                ("start_refused", json!([true, null])),
                ("chapter_not_live", json!([false, null])),
            ],
            json!([false, 21_600]),
            json!([true, null]),
            2
        )
    );
}

/// A Project whose Run of the author did not complete holds the row of a changed Chapter.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_project_with_no_completed_run_holds_the_row() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, _) = queued(&store, "836").await;
    park_queued_runs(&admin, &scope).await;
    let request = open_session_request(&store, &scope, "83d0").await;
    store.create_editor_session(&request).await.unwrap();
    let session = request.editor_session_id.as_ref();
    edit(&store, &admin, &scope, (session, 1, 0x83d1), &chapter_id, &["雨。"]).await;

    assert_eq!(
        decide(&store, &admin, &scope, &chapter_id).await,
        ("no_completed_run", json!([true, null]))
    );
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
    let (store, admin) = stores().await;
    let (scope, chapter_a, fog, session) = project(&store, &admin, "833", 0x83a0, &["雨。"]).await;
    select_chapter(&store, &admin, &scope, (&session, 0x83a6), &fog, &chapter_a).await;
    edit(&store, &admin, &scope, (&session, 2, 0x83a3), &fog, &["雾。"]).await;
    let volume_id: String = admin
        .query_one(
            "SELECT parent_volume_id::text FROM storyos.manuscript_objects
              WHERE manuscript_object_id = $1::text::uuid",
            &[&chapter_a],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let input = CreateChapterInput {
        volume_id,
        title: "Ice".to_owned(),
        placement: CreateChapterPlacement::Append,
        expected_tree_revision: 4,
    };
    let call = issued(&store, &scope, 0x83a4, &CREATE_CHAPTER, input).await;
    let ice = match create_chapter(&store, &call).await.unwrap().outcome {
        TransitionOutcome::Applied(applied) => applied.effect.chapter_id,
        other => panic!("{other:?}"),
    };
    select_chapter(&store, &admin, &scope, (&session, 0x83a7), &ice, &fog).await;
    edit(&store, &admin, &scope, (&session, 3, 0x83a5), &ice, &["冰。"]).await;
    // The due times are in the reverse of the Chapter order.
    make_chapter_due(&admin, &scope, &chapter_a, /*seconds*/ 1).await;
    make_chapter_due(&admin, &scope, &fog, /*seconds*/ 2).await;
    make_chapter_due(&admin, &scope, &ice, /*seconds*/ 3).await;
    let chapters = [&chapter_a, &fog, &ice];
    let state = async || {
        let titles: Vec<String> = admin
            .query(
                "SELECT chapter.title FROM storyos.memory_captures AS capture
                   JOIN storyos.manuscript_objects AS chapter
                     ON chapter.manuscript_object_id = capture.chapter_id
                  WHERE capture.project_id = $1::text::uuid ORDER BY capture.capture_id",
                &[&scope.project_id.as_ref()],
            )
            .await
            .unwrap()
            .iter()
            .map(|row| row.get(/*idx*/ 0))
            .collect();
        let mut rows = Vec::new();
        for chapter_id in chapters {
            rows.push(due_state(&admin, chapter_id).await);
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
    let mut states = Vec::new();
    let mut rate = None;
    for _ in 0..24 {
        storyos_worker::run_once(&store, &destination)
            .await
            .unwrap();
        let now = state().await;
        if states.last() == Some(&now) || now[0] == json!([]) {
            continue;
        }
        park_queued_runs(&admin, &scope).await;
        states.push(now);
        if rate.is_none() {
            let claimed = store.claim_due_memory_work().await.unwrap();
            rate = Some((claimed.map(|claimed| claimed.outcome.diagnostic()), state().await));
            make_chapter_due(&admin, &scope, &fog, /*seconds*/ 2).await;
            make_chapter_due(&admin, &scope, &ice, /*seconds*/ 3).await;
        }
        move_starts_back(&admin, &scope).await;
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
    let (store, admin) = stores().await;
    let sentences = ["雨夜。", "林舟撑船。"];
    let (scope, chapter_a, fog, session) = project(&store, &admin, "834", 0x83b0, &sentences).await;
    let half = usize::try_from(storyos_core::CONTEXT_ITEM_TOKEN_LIMIT).unwrap() / 2 + 1_000;
    let long = ["雾".repeat(half), "霜".repeat(half)];
    let long_refs = [long[0].as_str(), long[1].as_str()];
    select_chapter(&store, &admin, &scope, (&session, 0x83b4), &fog, &chapter_a).await;
    edit(&store, &admin, &scope, (&session, 2, 0x83b3), &fog, &long_refs).await;
    let mut inputs = Vec::new();
    for chapter_id in [&chapter_a, &fog] {
        make_chapter_due(&admin, &scope, chapter_id, /*seconds*/ 1).await;
        let claimed = store.claim_due_memory_work().await.unwrap();
        let Some(ClaimedMemoryWork {
            outcome: MemoryWorkOutcome::Started(started),
            ..
        }) = claimed
        else {
            panic!("the claim must start a capture: {claimed:?}");
        };
        park_queued_runs(&admin, &scope).await;
        move_starts_back(&admin, &scope).await;
        inputs.push(input_of(&admin, &started.run_id, chapter_id).await);
    }
    let expected = |title: &str, number: usize, blocks: &[&str]| {
        let blocks: Vec<String> = blocks.iter().map(|block| (*block).to_owned()).collect();
        let limit = storyos_core::CONTEXT_ITEM_TOKEN_LIMIT;
        let input = storyos_core::chapter_capture_input(title, number, &blocks, limit);
        let mode = match input.projection_mode {
            storyos_core::ProjectionMode::ExactRequired => "exact_required",
            storyos_core::ProjectionMode::DeterministicExcerpt => "deterministic_excerpt",
            other => panic!("{other:?}"),
        };
        (mode.to_owned(), input.text)
    };

    assert_eq!(
        inputs,
        [
            expected("Chapter A", 1, &sentences),
            expected("Fog", 2, &long_refs)
        ]
    );
    assert_eq!(
        [inputs[0].0.as_str(), inputs[1].0.as_str()],
        ["exact_required", "deterministic_excerpt"]
    );
}

/// From a changed Chapter to an entry through `run_once` on the fake destination: the commit of
/// the Chapter, the capture, and the pass give one current entry with the origin `chapter` and
/// that Chapter as its source. The test moves the due time of the Chapter after the commit.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn run_once_takes_a_changed_chapter_through_a_capture_and_a_pass_to_an_entry() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let observation = scripted("林舟", &[create("character", "林舟", json!([1]))]);
    let call = script(&[record(json!([observation]))]);
    let call = call.lines().last().unwrap();
    let sentences = ["林舟在雨夜撑船。", call];
    let (scope, chapter_id, _, _) = project(&store, &admin, "835", 0x83c0, &sentences).await;
    make_chapter_due(&admin, &scope, &chapter_id, /*seconds*/ 1).await;
    let destination = storyos_worker::ModelDestination {
        adapter: FakeDestination,
        observer: NoContractFaults,
        tools: storyos_application::NoToolDestination,
        embeddings: storyos_application::NoEmbeddingDestination,
    };
    let state = async || {
        admin
            .query_one(
                "SELECT jsonb_build_array(
                          (SELECT jsonb_agg(COALESCE(outcome, state)) FROM storyos.memory_captures
                            WHERE project_id = $1::text::uuid),
                          (SELECT jsonb_agg(COALESCE(outcome, 'running'))
                             FROM storyos.memory_consolidation_passes
                            WHERE project_id = $1::text::uuid))::text",
                &[&scope.project_id.as_ref()],
            )
            .await
            .map(|row| serde_json::from_str::<Value>(row.get(/*idx*/ 0)).unwrap())
            .unwrap()
    };
    // A step can claim work that other tests left. The test records the state of its Project
    // only when it changes.
    let mut states = vec![state().await];
    for _ in 0..24 {
        storyos_worker::run_once(&store, &destination)
            .await
            .unwrap();
        let now = state().await;
        if states.last() != Some(&now) {
            states.push(now);
        }
        if states.last().unwrap()[1] == json!(["applied"]) {
            break;
        }
    }
    let entries: Vec<_> = storyos_application::list_memory_entries(&store, &scope)
        .await
        .unwrap()
        .unwrap()
        .into_iter()
        .map(|entry| (entry.number, entry.origin, entry.source))
        .collect();

    assert_eq!(
        (Value::from(states), entries),
        (
            json!([
                [null, null],
                [["running"], null],
                [["observations_recorded"], null],
                [["observations_recorded"], ["running"]],
                [["observations_recorded"], ["applied"]]
            ]),
            vec![(
                1,
                MemoryEntryOrigin::Chapter,
                Some(MemoryEntrySourceItem::Chapter {
                    chapter_id: chapter_id.clone(),
                    title: Some("Chapter A".to_owned()),
                    available: true,
                })
            )]
        )
    );
}
