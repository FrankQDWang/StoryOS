//! The due write of the schedule row of a Chapter at each Authoritative Commit that changes its
//! text (ADR 0094 section 5, writer 3), over the command kinds of the command sequence. The
//! command sequence has no Redo and no promoted Draft. A created Chapter has an empty text.

use std::time::SystemTime;

use storyos_application::{EditorSessionId, ProjectScope, UndoLatestAuthorActionInput};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::create_volume_authority_tests::{NamedEdit, apply_named_edit};

use super::acceptance::{ACCEPT_PROPOSAL, accept_proposal, acceptable_proposal};
use super::structure::{UPDATE_CHAPTER, update_chapter};
use super::support::{issued, stores, two_chapter_writer};
use super::undo::{issued_undo, latest_forward, session_chapter_head, undo};

/// The row of each Chapter of the Project: the Chapter, its due time, and whether the due time is
/// the time of the last commit of the Chapter and the quiet time of 1 hour. The write precedes
/// the commit row in the transaction, so the due time is at most that time.
async fn chapter_rows(admin: &Client, scope: &ProjectScope) -> Vec<(String, SystemTime, bool)> {
    admin
        .query(
            "SELECT work.source_id::text, work.due_at,
                    work.due_at > max(commit.committed_at) + interval '59 minutes'
                AND work.due_at <= max(commit.committed_at) + interval '1 hour'
               FROM storyos.memory_work_schedules AS work
               JOIN storyos.authoritative_commits AS commit
                 ON (commit.owner_user_id, commit.project_id, commit.manuscript_object_id) =
                    (work.owner_user_id, work.project_id, work.source_id)
              WHERE work.project_id = $1::text::uuid AND work.work_kind = 'chapter_capture'
              GROUP BY work.source_id, work.due_at
              ORDER BY work.source_id",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(/*idx*/ 0), row.get(/*idx*/ 1), row.get(/*idx*/ 2)))
        .collect()
}

/// The Chapters whose row a step wrote or moved, named by `label`, and whether each new due time
/// is the time of the commit and 1 hour.
fn changed(
    before: &[(String, SystemTime, bool)],
    after: &[(String, SystemTime, bool)],
    label: impl Fn(&str) -> &'static str,
) -> Vec<(&'static str, bool)> {
    after
        .iter()
        .filter(|row| !before.iter().any(|earlier| earlier.0 == row.0 && earlier.1 == row.1))
        .map(|row| (label(&row.0), row.2))
        .collect()
}

/// One Author Edit that inserts `text` at the start of the Chapter of the Editor Session.
async fn edit(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    session: &str,
    (suffix, text): (u16, &str),
) {
    let chapter_id: String = admin
        .query_one(
            "SELECT chapter_object_id::text FROM storyos.editor_session_base_snapshots
              WHERE editor_session_id = $1::text::uuid",
            &[&session],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let head = session_chapter_head(admin, session).await;
    apply_named_edit(
        store,
        scope,
        NamedEdit {
            editor_session_id: session,
            chapter_id: &chapter_id,
            expected_revision_id: &head,
            suffix: &format!("{suffix:04x}"),
            local_intent_sequence: u64::from(suffix),
            text,
            proposal_target: None,
        },
    )
    .await;
}

/// Each command kind that changes the text of a Chapter writes the due time of the row of that
/// Chapter in its transaction: an accepted Proposal, an Author Edit, and the Author Undo of that
/// edit. A created Chapter and a new title write no row.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_commit_that_changes_the_text_of_a_chapter_writes_the_due_time_of_its_row() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, input, chapter_a_head) = acceptable_proposal(&store, &admin, 0x8300).await;
    let chapter_of = async |revision_id: &str| -> String {
        admin
            .query_one(
                "SELECT manuscript_object_id::text FROM storyos.authoritative_revisions
                  WHERE revision_id = $1::text::uuid",
                &[&revision_id],
            )
            .await
            .unwrap()
            .get(/*idx*/ 0)
    };
    let chapter_a = chapter_of(&chapter_a_head).await;
    let chapter_b = chapter_of(&input.expected_authoritative_revision_id).await;
    let label = |chapter_id: &str| match chapter_id {
        id if id == chapter_a => "A",
        id if id == chapter_b => "B",
        _ => "other",
    };
    let session = input.editor_session_id.as_ref().to_owned();
    let mut rows = chapter_rows(&admin, &scope).await;
    let mut steps = vec![("createChapter", rows.iter().map(|row| (label(&row.0), row.2)).collect())];
    let mut step = async |kind: &'static str, rows: &mut Vec<(String, SystemTime, bool)>| {
        let after = chapter_rows(&admin, &scope).await;
        steps.push((kind, changed(rows, &after, label)));
        *rows = after;
    };

    let call = issued(&store, &scope, 0x8370, &ACCEPT_PROPOSAL, input).await;
    accept_proposal(&store, &call).await.unwrap();
    step("acceptProposal", &mut rows).await;
    let rename = storyos_application::UpdateChapterInput {
        chapter_id: storyos_application::ChapterId::new(chapter_b.as_str()),
        title: "Renamed".to_owned(),
        order: 2,
        expected_tree_revision: 3,
    };
    let call = issued(&store, &scope, 0x8371, &UPDATE_CHAPTER, rename).await;
    update_chapter(&store, &call).await.unwrap();
    step("updateChapter", &mut rows).await;
    edit(&store, &admin, &scope, &session, (0x8372, "Rain. ")).await;
    step("applyAuthorEdit", &mut rows).await;
    let input = UndoLatestAuthorActionInput {
        editor_session_id: EditorSessionId::new(session.as_str()),
        expected_author_undo_frontier_sequence: latest_forward(&admin, &scope).await,
        expected_authoritative_revision_id: session_chapter_head(&admin, &session).await,
    };
    let call = issued_undo(&store, &scope, 0x8373, input).await;
    undo(&store, &call).await.unwrap();
    step("undoLatestAuthorAction", &mut rows).await;

    assert_eq!(
        steps,
        [
            ("createChapter", vec![]),
            ("acceptProposal", vec![("B", true)]),
            ("updateChapter", vec![]),
            ("applyAuthorEdit", vec![("A", true)]),
            ("undoLatestAuthorAction", vec![("A", true)]),
        ]
    );
}

/// A later commit of the same Chapter before its due time moves the due time, so a Chapter that
/// the author edits again and again starts no capture.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_later_commit_of_the_chapter_moves_its_due_time() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, _, _, session) = two_chapter_writer(&store, 0x8310).await;
    edit(&store, &admin, &scope, &session, (0x8380, "Rain. ")).await;
    let first = chapter_rows(&admin, &scope).await;
    edit(&store, &admin, &scope, &session, (0x8381, "Wind. ")).await;
    let second = chapter_rows(&admin, &scope).await;

    assert_eq!(
        (
            first.iter().map(|row| row.2).collect::<Vec<_>>(),
            second.iter().map(|row| row.2).collect::<Vec<_>>(),
            first[0].0 == second[0].0,
            second[0].1 > first[0].1,
        ),
        (vec![true], vec![true], true, true)
    );
}
