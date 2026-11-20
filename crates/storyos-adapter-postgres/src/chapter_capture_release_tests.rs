//! The existing manuscript and the path of a Chapter capture in the store (ADR 0094 section 5,
//! writer 3, S7-D5b): the release of the `held` rows at the first completed Run of the author, the
//! path from a changed Chapter to an entry, what a capture reads, its failed outcomes, the Project
//! Scope, the actions of the author during a capture, and a stopped Worker.

use storyos_application::MemoryEntrySourceItem;
use storyos_core::MemoryEntryOrigin;

use super::*;
use crate::agent_run_dispatch::tests::NoEmbeddings;
use crate::agent_run_finish_rule::tests::complete;
use crate::memory_capture::loop_tests::settlement;
use crate::memory_consolidation::tests::{create, scripted};
use crate::memory_work_schedule::outcome_tests::stopped_at_the_start;

/// The fake destination with no Tool destination and no embedding destination.
fn destination() -> storyos_worker::ModelDestination<
    FakeDestination,
    NoContractFaults,
    storyos_application::NoToolDestination,
    NoEmbeddings,
> {
    storyos_worker::ModelDestination {
        adapter: FakeDestination,
        observer: NoContractFaults,
        tools: storyos_application::NoToolDestination,
        embeddings: NoEmbeddings,
    }
}

/// Runs `sql` with the Project of `scope` as `$1` on the test administrator connection.
async fn project_sql(f: &Fixture, sql: &str) -> Vec<tokio_postgres::Row> {
    let project_id = f.scope.project_id.as_ref();
    f.admin.query(sql, &[&project_id]).await.unwrap()
}

/// The started capture of a claim, or a panic.
fn started(claimed: Option<ClaimedMemoryWork>) -> StartedMemoryCapture {
    match claimed {
        Some(ClaimedMemoryWork {
            outcome: MemoryWorkOutcome::Started(started),
            ..
        }) => started,
        other => panic!("the claim must start a capture: {other:?}"),
    }
}

/// The Chapter text of a capture that records one observation, from which the pass of the fake
/// creates the entry `林舟`.
fn observation_text() -> [String; 2] {
    let observation = scripted("林舟", &[create("character", "林舟", json!([1]))]);
    let call = script(&[record(json!([observation]))]);
    let call = call.lines().last().unwrap().to_owned();
    ["林舟在雨夜撑船。".to_owned(), call]
}

/// A Project of 5 Chapters that the author wrote before the first Run, in the reverse of the
/// Chapter order. Each row becomes `held`. The settlement of the first completed Run of the author
/// makes each row due at its commit and 1 hour. Then `run_once` starts one capture for each
/// Chapter in Chapter order, a claim before 2 minutes waits for the rate, and a second pass over
/// the rows starts no capture. The test moves the due times and the capture starts back.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn five_chapters_written_before_the_first_run_get_one_capture_each_in_chapter_order() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_a, first) = queued(&store, "891").await;
    let request = open_session_request(&store, &scope, "89b0").await;
    store.create_editor_session(&request).await.unwrap();
    let session = request.editor_session_id.as_ref().to_owned();
    let f = Fixture {
        store,
        admin,
        scope,
        session,
    };
    let volume = "SELECT parent_volume_id::text FROM storyos.manuscript_objects
                   WHERE manuscript_object_id = $1::text::uuid";
    let volume_id: String = f
        .admin
        .query_one(volume, &[&chapter_a])
        .await
        .unwrap()
        .get(0);
    let mut chapters = vec![chapter_a];
    for (index, title) in ["Chapter B", "Chapter C", "Chapter D", "Chapter E"]
        .into_iter()
        .enumerate()
    {
        let input = CreateChapterInput {
            volume_id: volume_id.clone(),
            title: title.to_owned(),
            placement: CreateChapterPlacement::Append,
            expected_tree_revision: 3 + u64::try_from(index).unwrap(),
        };
        let suffix = 0x89b1 + u16::try_from(index).unwrap();
        let call = issued(&f.store, &f.scope, suffix, &CREATE_CHAPTER, input).await;
        let outcome = create_chapter(&f.store, &call).await.unwrap().outcome;
        let TransitionOutcome::Applied(created) = outcome else {
            panic!("{title} must be created");
        };
        chapters.push(created.effect.chapter_id);
    }
    // The setup issues more Shared Challenges than one window of the Project permits.
    let window = "UPDATE storyos.project_command_challenge_rate_windows SET issued_count = 0
                   WHERE project_id = $1::text::uuid";
    project_sql(&f, window).await;
    let no_observation = script(&[record(json!([]))]);
    let call = no_observation.lines().last().unwrap();
    let mut from = chapters[0].clone();
    for (sequence, chapter_id) in (1..).zip(chapters.iter().rev()) {
        if *chapter_id != from {
            f.select(0x89b4 + sequence, chapter_id, &from).await;
        }
        f.edit(
            (u64::from(sequence), 0x89b9 + sequence),
            chapter_id,
            &["雨。", call],
        )
        .await;
        from.clone_from(chapter_id);
    }
    let rows = async || {
        let rows = project_sql(
            &f,
            "SELECT jsonb_agg(jsonb_build_array(work.held, work.due_at = (
                      SELECT max(commit.committed_at) + interval '1 hour'
                        FROM storyos.authoritative_commits AS commit
                       WHERE commit.manuscript_object_id = work.source_id))
                      ORDER BY work.source_id)::text
               FROM storyos.memory_work_schedules AS work
              WHERE work.project_id = $1::text::uuid AND work.work_kind = 'chapter_capture'",
        );
        serde_json::from_str::<Value>(rows.await[0].get(0)).unwrap()
    };
    // Each row of the Project that has a due time is due now, and no row of another Project is due.
    let due_now = "UPDATE storyos.memory_work_schedules
                      SET due_at = CASE WHEN project_id = $1::text::uuid THEN clock_timestamp() END
                    WHERE due_at IS NOT NULL
                      AND (work_kind = 'chapter_capture' OR project_id <> $1::text::uuid)";
    project_sql(&f, due_now).await;
    let mut holds = Vec::new();
    for _ in 0..5 {
        let claimed = f.store.claim_due_memory_work().await.unwrap().unwrap();
        holds.push(claimed.outcome.diagnostic());
    }
    let held = rows().await;
    complete(&f.store, &f.admin, &f.scope, &first).await;
    let released = rows().await;

    let back = "UPDATE storyos.memory_work_schedules SET due_at = due_at - interval '1 hour'
                 WHERE project_id = $1::text::uuid AND work_kind = 'chapter_capture'";
    project_sql(&f, back).await;
    let titles = "SELECT chapter.title, capture.state FROM storyos.memory_captures AS capture
                    JOIN storyos.manuscript_objects AS chapter
                      ON chapter.manuscript_object_id = capture.chapter_id
                   WHERE capture.project_id = $1::text::uuid ORDER BY capture.capture_id";
    let (mut order, mut rate) = (Vec::new(), None);
    for _ in 0..40 {
        storyos_worker::run_once(&f.store, &destination())
            .await
            .unwrap();
        let captures = project_sql(&f, titles).await;
        if captures.len() > order.len() {
            order.push(captures.last().unwrap().get::<_, String>(0));
            if rate.is_none() {
                let claimed = f.store.claim_due_memory_work().await.unwrap();
                rate = claimed.map(|claimed| claimed.outcome.diagnostic());
                project_sql(&f, due_now).await;
            }
            f.move_starts_back().await;
        }
        let settled = captures
            .iter()
            .all(|row| row.get::<_, &str>(1) == "settled");
        if captures.len() == 5 && settled {
            break;
        }
    }
    let all_due = "UPDATE storyos.memory_work_schedules SET due_at = clock_timestamp()
                    WHERE project_id = $1::text::uuid AND work_kind = 'chapter_capture'";
    project_sql(&f, all_due).await;
    project_sql(&f, due_now).await;
    let mut second = Vec::new();
    for _ in 0..5 {
        let claimed = f.store.claim_due_memory_work().await.unwrap().unwrap();
        second.push(claimed.outcome.diagnostic());
    }

    assert_eq!(
        (holds, held, released, order.join(" "), rate, second),
        (
            vec!["no_completed_run"; 5],
            json!(vec![json!([true, null]); 5]),
            json!(vec![json!([false, true]); 5]),
            "Chapter A Chapter B Chapter C Chapter D Chapter E".to_owned(),
            Some("rate_wait"),
            vec!["revision_read"; 5]
        )
    );
}

/// What the author does while the capture of Chapter A runs.
enum During {
    Nothing,
    Edit,
    Delete,
}

/// Runs `run_once` from a due row of Chapter A until the pass of the Project applies, and does
/// `during` one time while the capture runs. Returns each changed state of the captures and the
/// passes, and the number, the origin, and the source of each entry.
async fn to_entry(f: &Fixture, chapter_a: &str, during: During) -> (Value, Vec<Entry>) {
    make_chapter_due(&f.admin, &f.scope, chapter_a, /*seconds*/ 1).await;
    let state = async || {
        let state = project_sql(
            f,
            "SELECT jsonb_build_array(
                      (SELECT jsonb_agg(COALESCE(outcome, state)) FROM storyos.memory_captures
                        WHERE project_id = $1::text::uuid),
                      (SELECT jsonb_agg(COALESCE(outcome, 'running'))
                         FROM storyos.memory_consolidation_passes
                        WHERE project_id = $1::text::uuid))::text",
        );
        serde_json::from_str::<Value>(state.await[0].get(0)).unwrap()
    };
    // A step can claim work that other tests left. The test records the state of its Project
    // only when it changes.
    let mut states = vec![state().await];
    for _ in 0..24 {
        storyos_worker::run_once(&f.store, &destination())
            .await
            .unwrap();
        let now = state().await;
        if now == json!([["running"], null]) && states.last() != Some(&now) {
            match during {
                During::Nothing => {}
                During::Edit => f.edit((2, 0x89c6), chapter_a, &["风。"]).await,
                During::Delete => {
                    let input = DeleteChapterInput {
                        chapter_id: ChapterId::new(chapter_a),
                        expected_tree_revision: 4,
                    };
                    let call = issued(&f.store, &f.scope, 0x89ca, &DELETE_CHAPTER, input).await;
                    delete_chapter(&f.store, &call).await.unwrap();
                }
            }
        }
        if states.last() != Some(&now) {
            states.push(now);
        }
        if states.last().unwrap()[1] == json!(["applied"]) {
            break;
        }
    }
    let entries = storyos_application::list_memory_entries(&f.store, &f.scope).await;
    let entries = entries.unwrap().unwrap().into_iter();
    let entries = entries.map(|entry| (entry.number, entry.origin, entry.source));
    (Value::from(states), entries.collect())
}

/// The states of `to_entry` for one capture that records one observation and one pass.
fn capture_and_pass() -> Value {
    json!([
        [null, null],
        [["running"], null],
        [["observations_recorded"], null],
        [["observations_recorded"], ["running"]],
        [["observations_recorded"], ["applied"]]
    ])
}

/// The number, the origin, and the source of an entry.
type Entry = (u64, MemoryEntryOrigin, Option<MemoryEntrySourceItem>);

/// The one entry of the Chapter, with its source.
fn chapter_entry(chapter_id: &str, title: Option<&str>, available: bool) -> Vec<Entry> {
    let source = MemoryEntrySourceItem::Chapter {
        chapter_id: chapter_id.to_owned(),
        title: title.map(str::to_owned),
        available,
    };
    vec![(1, MemoryEntryOrigin::Chapter, Some(source))]
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
    let text = observation_text();
    let (f, chapter_a, _) = fixture("892", 0x89c0, &[&text[0], &text[1]]).await;

    assert_eq!(
        to_entry(&f, &chapter_a, During::Nothing).await,
        (
            capture_and_pass(),
            chapter_entry(&chapter_a, Some("Chapter A"), true)
        )
    );
}

/// An edit of the Chapter during its capture is admitted and does not wait. The capture completes
/// with the Revision that it pinned, its settlement sets the due time again, and a later capture
/// reads the new text.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_edit_during_a_capture_is_admitted_and_a_later_capture_reads_the_new_text() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let text = observation_text();
    let (f, chapter_a, _) = fixture("893", 0x89c3, &[&text[0], &text[1]]).await;
    let pinned = head(&f.admin, &chapter_a).await.0;
    let path = to_entry(&f, &chapter_a, During::Edit).await;
    let read = "SELECT chapter_revision_id::text FROM storyos.memory_captures
                 WHERE project_id = $1::text::uuid";
    let first: String = project_sql(&f, read).await[0].get(0);
    let after_settlement = row(&f.admin, &chapter_a).await;
    f.move_starts_back().await;
    make_chapter_due(&f.admin, &f.scope, &chapter_a, /*seconds*/ 1).await;
    let later = started(f.store.claim_due_memory_work().await.unwrap());
    park_queued_runs(&f.admin, &f.scope).await;
    let (_, input) = input_of(&f.admin, &later.run_id, &chapter_a).await;

    assert_eq!(
        (
            path,
            first == pinned,
            after_settlement,
            input.contains("风。")
        ),
        (
            (
                capture_and_pass(),
                chapter_entry(&chapter_a, Some("Chapter A"), true)
            ),
            true,
            json!([false, 21_600]),
            true
        )
    );
}

/// A delete of the Chapter during its capture is admitted. The capture completes, and the entry
/// of its observation keeps a source that is not available. A due row of the deleted Chapter starts
/// no capture.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_delete_during_a_capture_keeps_an_unavailable_source_and_starts_no_capture() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let text = observation_text();
    let (f, chapter_a, _) = fixture("894", 0x89c7, &[&text[0], &text[1]]).await;
    let path = to_entry(&f, &chapter_a, During::Delete).await;

    assert_eq!(
        (path, f.decide(&chapter_a).await),
        (
            (capture_and_pass(), chapter_entry(&chapter_a, None, false)),
            ("chapter_not_live", json!([false, null]))
        )
    );
}

/// A capture reads the authoritative text of its Chapter. A Proposal for the Chapter that waits
/// with another text does not reach the input. StoryOS has no stored Draft (ADR 0079), and the text
/// of the editor that is not committed is not stored.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_capture_reads_the_authoritative_text_and_not_a_waiting_proposal() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let sentences = ["雨夜。", "林舟撑船。"];
    let (f, chapter_a, _) = fixture("895", 0x89cb, &sentences).await;
    let revision = head(&f.admin, &chapter_a).await.0;
    let block = "SELECT manuscript_block_id::text FROM storyos.manuscript_revision_members
                  WHERE revision_id = $1::text::uuid ORDER BY block_order LIMIT 1";
    let block: String = f.admin.query_one(block, &[&revision]).await.unwrap().get(0);
    let ids: Vec<String> = (0..3).map(|_| uuid::Uuid::now_v7().to_string()).collect();
    f.admin
        .batch_execute(&format!(
            "BEGIN;
             SET LOCAL session_replication_role = replica;
             INSERT INTO storyos.proposals
               (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                source_run_id, source_decision_id)
             VALUES ('{owner}', '{project}', '{proposal}', 'block_edit', '{chapter_a}',
                     '{block}', '{proposal}', '{proposal}');
             INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{proposal_revision}', 'ready',
                     'valid', 'open', '提议的文字。', '{revision}');
             INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{proposal_revision}');
             INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             VALUES ('{owner}', '{project}', '{proposal}', '{operation}', '{block}',
                     'pending', 'unresolved', '提议的文字。');
             COMMIT;",
            owner = f.scope.owner_user_id.as_ref(),
            project = f.scope.project_id.as_ref(),
            proposal = ids[0],
            proposal_revision = ids[1],
            operation = ids[2],
        ))
        .await
        .unwrap();
    make_chapter_due(&f.admin, &f.scope, &chapter_a, /*seconds*/ 1).await;
    let capture = started(f.store.claim_due_memory_work().await.unwrap());
    park_queued_runs(&f.admin, &f.scope).await;
    let blocks = sentences.map(str::to_owned);
    let limit = storyos_core::CONTEXT_ITEM_TOKEN_LIMIT;
    let expected = storyos_core::chapter_capture_input("Chapter A", 1, &blocks, limit).text;

    assert_eq!(
        input_of(&f.admin, &capture.run_id, &chapter_a).await,
        ("exact_required".to_owned(), expected)
    );
}

/// After a capture with the outcome `step_limit` or `failed`, the Host starts no capture of the
/// same Revision by itself: the due time stays empty. The next commit of the Chapter sets the due
/// time again.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn after_a_failed_capture_only_the_next_commit_sets_the_due_time_again() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let max = usize::try_from(storyos_core::MEMORY_CAPTURE_MAX_RUN_STEPS).unwrap();
    let edits = script(&vec![
        ("propose_edit", json!({"reason": "改写"}));
        max + 2
    ]);
    let lines: Vec<&str> = edits.lines().collect();
    let (f, chapter_a, _) = fixture("896", 0x89d0, &lines).await;
    let grant = async |version: &str| {
        let grant = "UPDATE storyos.project_destination_grants
                        SET grant_scope = 'project_writing_assistance_' || $2
                      WHERE project_id = $1::text::uuid";
        let project_id = f.scope.project_id.as_ref();
        f.admin
            .execute(grant, &[&project_id, &version])
            .await
            .unwrap();
    };
    let mut ends = Vec::new();
    for (version, sequence, suffix) in [("v5", 2, 0x89d3), ("v4", 3, 0x89d4)] {
        f.move_starts_back().await;
        make_chapter_due(&f.admin, &f.scope, &chapter_a, /*seconds*/ 1).await;
        let capture = started(f.store.claim_due_memory_work().await.unwrap());
        grant(version).await;
        f.run_capture(&capture).await;
        grant("v5").await;
        let outcome = settlement(&f.admin, &capture.run_id).await[2].clone();
        let settled = row(&f.admin, &chapter_a).await;
        f.edit((sequence, suffix), &chapter_a, &["风。"]).await;
        ends.push(json!([outcome, settled, row(&f.admin, &chapter_a).await]));
    }

    assert_eq!(
        ends,
        [
            json!(["step_limit", [false, null], [false, 21_600]]),
            json!(["failed", [false, null], [false, 21_600]])
        ]
    );
}

/// A Chapter capture reads only the Chapter of its row in its Project Scope: the rows of two
/// Projects start two captures, each with the text of its own Chapter.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_capture_reads_only_the_chapter_of_its_row_in_its_project() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let mut inputs = Vec::new();
    for (lead, suffix, text) in [("897", 0x89d5, "甲。"), ("898", 0x89d8, "乙。")] {
        let (f, chapter_a, _) = fixture(lead, suffix, &[text]).await;
        make_chapter_due(&f.admin, &f.scope, &chapter_a, /*seconds*/ 1).await;
        let capture = started(f.store.claim_due_memory_work().await.unwrap());
        park_queued_runs(&f.admin, &f.scope).await;
        inputs.push(input_of(&f.admin, &capture.run_id, &chapter_a).await.1);
    }
    let limit = storyos_core::CONTEXT_ITEM_TOKEN_LIMIT;
    let input = |text: &str| {
        let blocks = [text.to_owned()];
        storyos_core::chapter_capture_input("Chapter A", 1, &blocks, limit).text
    };

    assert_eq!(inputs, [input("甲。"), input("乙。")]);
}

/// A Worker that stops after the claim of the row of a Chapter and before the start of its
/// capture leaves the row due, because the claim and the start are one transaction. The next
/// claim starts one capture.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_stopped_worker_leaves_the_chapter_row_due_and_the_next_claim_starts_one_capture() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (f, chapter_a, _) = fixture("899", 0x89db, &["雨。"]).await;
    make_chapter_due(&f.admin, &f.scope, &chapter_a, /*seconds*/ 1).await;
    let stopped = stopped_at_the_start(&f.admin, f.store.claim_due_memory_work()).await;
    let after_stop = due_state(&f.admin, &chapter_a).await;
    let next = f.store.claim_due_memory_work().await.unwrap();
    park_queued_runs(&f.admin, &f.scope).await;
    let count = "SELECT count(*) FROM storyos.memory_captures WHERE project_id = $1::text::uuid";
    let captures: i64 = project_sql(&f, count).await[0].get(0);

    assert_eq!(
        (
            stopped.is_err(),
            after_stop,
            next.map(|next| next.source),
            captures
        ),
        (
            true,
            json!("due"),
            Some(MemoryWorkSource::Chapter(chapter_a)),
            1
        )
    );
}
