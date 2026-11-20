//! The shared test support from `chapter_capture_schedule_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_read_chapter::{project, write_blocks};
use crate::shared_tests::chapter_list_context::{head, select};
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::memory_capture::run_capture;
use crate::shared_tests::memory_capture_loop::{record, script};
use crate::shared_tests::project_research_binding::park_queued_runs;
use serde_json::{Value, json};
use storyos_application::{
    DiagnosticField, EditorSessionId, MemoryCaptureSource, MemoryWorkScheduleStore,
    MemoryWorkSource, ProjectScope, StartedMemoryCapture,
};
use tokio_postgres::Client;

/// Makes the row of the Chapter due `seconds` ago, and each other row not due, so that the
/// claim takes this row. A Chapter with no commit gets its row.
pub(crate) async fn make_chapter_due(
    admin: &Client,
    scope: &ProjectScope,
    chapter_id: &str,
    seconds: i32,
) {
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
pub(crate) async fn row(admin: &Client, chapter_id: &str) -> Value {
    json_of(
        admin,
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
}

/// Whether the row of the Chapter is due now, due later, or not due.
pub(crate) async fn due_state(admin: &Client, chapter_id: &str) -> Value {
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
pub(crate) struct Fixture {
    pub(crate) store: PostgresProjectReader,
    pub(crate) admin: Client,
    pub(crate) scope: ProjectScope,
    pub(crate) session: String,
}

impl Fixture {
    /// Claims the due row of the Chapter. Returns the label of the outcome and the row after it.
    pub(crate) async fn decide(&self, chapter_id: &str) -> (&'static str, Value) {
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
    pub(crate) async fn move_starts_back(&self) {
        let moved =
            "UPDATE storyos.memory_captures SET started_at = started_at - interval '3 minutes'
                      WHERE project_id = $1::text::uuid";
        let project_id = self.scope.project_id.as_ref();
        self.admin.execute(moved, &[&project_id]).await.unwrap();
    }

    /// One Author Edit with the local intent `sequence` and the challenge `suffix` that writes
    /// `sentences` at the start of the first Block of `chapter_id`, one Block each.
    pub(crate) async fn edit(
        &self,
        (sequence, suffix): (u64, u16),
        chapter_id: &str,
        sentences: &[&str],
    ) {
        let suffix = format!("{suffix:04x}");
        let edit = (self.session.as_str(), sequence, suffix.as_str());
        let (store, admin, scope) = (&self.store, &self.admin, &self.scope);
        let write = write_blocks(store, admin, scope, edit, chapter_id, sentences).await;
        store.apply_author_edit(&write).await.unwrap();
    }

    /// Selects `chapter_id` as the Current Chapter after `from`, so that the next edit can
    /// change it.
    pub(crate) async fn select(&self, suffix: u16, chapter_id: &str, from: &str) {
        let writer = EditorSessionId::new(self.session.as_str());
        let (store, admin, scope) = (&self.store, &self.admin, &self.scope);
        let call = select(store, admin, scope, (&writer, suffix), chapter_id, from).await;
        store
            .set_current_chapter(&call.envelope, &call.input)
            .await
            .unwrap();
    }

    /// Starts one capture of the Chapter at its head with no observation.
    pub(crate) async fn start_capture(&self, chapter_id: &str) -> StartedMemoryCapture {
        let source = MemoryCaptureSource::Chapter {
            chapter_id: chapter_id.to_owned(),
            revision_id: head(&self.admin, chapter_id).await.0,
        };
        let input = script(&[record(json!([]))]);
        crate::shared_tests::memory_capture::start(&self.store, &self.scope, &source, &input)
            .await
            .unwrap()
    }

    pub(crate) async fn run_capture(&self, started: &StartedMemoryCapture) {
        run_capture(&self.store, &self.admin, &self.scope, &started.run_id).await;
    }
}

/// A Project of `lead` whose Chapter A has the Blocks `sentences`, with a second empty Chapter
/// `Fog` and an open Editor Session. `suffix` to `suffix + 2` are its challenges.
pub(crate) async fn fixture(
    lead: &str,
    suffix: u16,
    sentences: &[&str],
) -> (Fixture, String, String) {
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

/// The context item of the first RunStep of the capture Run `run_id`: its projection mode and its
/// content. The capture row has the Chapter and the head Revision of the Chapter.
pub(crate) async fn input_of(admin: &Client, run_id: &str, chapter_id: &str) -> (String, String) {
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
