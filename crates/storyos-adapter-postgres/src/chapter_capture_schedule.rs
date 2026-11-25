//! The schedule of a Chapter capture in the store (ADR 0094 section 5, writer 3): the due write in
//! the transaction of each Authoritative Commit that changes the text of a Chapter, the due write
//! at the settlement of a Chapter capture, and the decision for a claimed row of a Chapter in the
//! transaction of the claim. No record of this work has manuscript text outside the input of the
//! capture (ADR 0047).

use std::time::SystemTime;

use crate::statement_cache::Client;
use storyos_application::{
    ChapterId, ClaimedAgentRun, CompleteAgentRunError, MemoryCaptureSource, MemoryWorkOutcome,
    ProjectReadError, ProjectScope,
};
use storyos_core::{ChapterCaptureFacts, ChapterCaptureSkip, ChapterRowChange};

use crate::agent_run_work::complete_unavailable;
use crate::manuscript_search::{LiveChapterReadExtent, read_live_chapter_blocks};
use crate::read_error;

/// The kind of the schedule row of a Chapter.
const CHAPTER_CAPTURE: &str = "chapter_capture";

/// Sets the due time of the row of the Chapter `chapter_id` by the rule of Core, in the
/// transaction of the Authoritative Commit that changes its text. A later commit moves the due
/// time. A `held` row stays `held`.
pub(crate) async fn mark_chapter_due(
    client: &Client,
    scope: &ProjectScope,
    chapter_id: &str,
) -> Result<(), tokio_postgres::Error> {
    let keys = [scope.owner_user_id.as_ref(), scope.project_id.as_ref()];
    let row = client
        .query_one(
            "SELECT clock_timestamp(),
                    (SELECT max(started_at) FROM storyos.memory_captures
                      WHERE (owner_user_id, project_id, chapter_id) =
                            ($1::text::uuid, $2::text::uuid, $3::text::uuid))",
            &[&keys[0], &keys[1], &chapter_id],
        )
        .await?;
    let due_at = storyos_core::chapter_capture_due(row.get(/*idx*/ 0), row.get(/*idx*/ 1));
    write_due(client, scope, chapter_id, due_at).await
}

/// Sets the due time of the row of the Chapter of the capture `capture_id`, in the transaction of
/// its settlement, when the Authoritative Revision of the Chapter is not the Revision that the
/// capture read. A Conversation capture has no Chapter row.
pub(crate) async fn mark_due_after_capture(
    client: &Client,
    claim: &ClaimedAgentRun,
    capture_id: &str,
) -> Result<(), CompleteAgentRunError> {
    let scope = &claim.project_scope;
    let Some(row) = client
        .query_opt(
            "SELECT capture.chapter_id::text,
                    (SELECT max(commit.committed_at) FROM storyos.authoritative_commits AS commit
                      WHERE (commit.owner_user_id, commit.project_id,
                             commit.manuscript_object_id) =
                            (capture.owner_user_id, capture.project_id, capture.chapter_id)),
                    capture.started_at
               FROM storyos.memory_captures AS capture
               JOIN storyos.authoritative_heads AS head
                 ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                    (capture.owner_user_id, capture.project_id, capture.chapter_id)
              WHERE (capture.owner_user_id, capture.project_id, capture.capture_id) =
                    ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                AND capture.source_kind = 'chapter'
                AND head.current_revision_id <> capture.chapter_revision_id",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &capture_id,
            ],
        )
        .await
        .map_err(complete_unavailable)?
    else {
        return Ok(());
    };
    let chapter_id: String = row.get(/*idx*/ 0);
    let Some(committed_at) = row.get::<_, Option<SystemTime>>(/*idx*/ 1) else {
        return Ok(());
    };
    let due_at = storyos_core::chapter_capture_due(committed_at, row.get(/*idx*/ 2));
    write_due(client, scope, &chapter_id, due_at)
        .await
        .map_err(complete_unavailable)
}

/// Makes each `held` row of a Chapter of the Project due by the rule of Core, in the transaction of
/// the settlement of the completed Run of the author `claim`. A completed Run shows that the
/// Project has a binding, a grant, and an available assistance. A background Run releases no row.
pub(crate) async fn release_held(
    client: &Client,
    claim: &ClaimedAgentRun,
) -> Result<(), CompleteAgentRunError> {
    let scope = &claim.project_scope;
    let held = client
        .query(
            "SELECT work.source_id::text,
                    (SELECT max(commit.committed_at) FROM storyos.authoritative_commits AS commit
                      WHERE (commit.owner_user_id, commit.project_id,
                             commit.manuscript_object_id) =
                            (work.owner_user_id, work.project_id, work.source_id)),
                    (SELECT max(capture.started_at) FROM storyos.memory_captures AS capture
                      WHERE (capture.owner_user_id, capture.project_id, capture.chapter_id) =
                            (work.owner_user_id, work.project_id, work.source_id))
               FROM storyos.memory_work_schedules AS work
               JOIN storyos.agent_runs AS run
                 ON (run.owner_user_id, run.project_id) = (work.owner_user_id, work.project_id)
              WHERE (run.owner_user_id, run.project_id, run.run_id) =
                    ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                AND run.run_kind = 'author' AND work.work_kind = $4 AND work.held
                FOR UPDATE OF work",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &claim.run_id,
                &CHAPTER_CAPTURE,
            ],
        )
        .await
        .map_err(complete_unavailable)?;
    for row in held {
        let chapter_id: String = row.get(/*idx*/ 0);
        // A row has a commit of its Chapter, because a commit writes the row.
        let Some(committed_at) = row.get::<_, Option<SystemTime>>(/*idx*/ 1) else {
            continue;
        };
        let due_at = storyos_core::chapter_capture_due(committed_at, row.get(/*idx*/ 2));
        set_row(client, scope, &chapter_id, (false, Some(due_at)))
            .await
            .map_err(complete_unavailable)?;
    }
    Ok(())
}

/// Inserts the row of the Chapter with `due_at`, or moves the due time of a row that is not
/// `held`.
async fn write_due(
    client: &Client,
    scope: &ProjectScope,
    chapter_id: &str,
    due_at: SystemTime,
) -> Result<(), tokio_postgres::Error> {
    let keys = [scope.owner_user_id.as_ref(), scope.project_id.as_ref()];
    client
        .execute(
            "INSERT INTO storyos.memory_work_schedules AS work
               (owner_user_id, project_id, work_kind, source_id, chapter_id, due_at)
             VALUES ($1::text::uuid, $2::text::uuid, $4, $3::text::uuid, $3::text::uuid, $5)
             ON CONFLICT (owner_user_id, project_id, work_kind, source_id)
             DO UPDATE SET due_at = EXCLUDED.due_at WHERE NOT work.held",
            &[&keys[0], &keys[1], &chapter_id, &CHAPTER_CAPTURE, &due_at],
        )
        .await
        .map(|_| ())
}

/// Decides for the claimed row of the Chapter `claimed`, whose due time was `claimed_due` before
/// the claim. When another due row of the Project has a Chapter that is first in Chapter order,
/// the decision is for that Chapter, and the claimed row stays due. Returns the Chapter of the
/// decision and the outcome. The decision writes its change of the row.
pub(crate) async fn decide_and_start(
    client: &Client,
    scope: &ProjectScope,
    claimed: &str,
    claimed_due: SystemTime,
) -> Result<(String, MemoryWorkOutcome), ProjectReadError> {
    let keys = [scope.owner_user_id.as_ref(), scope.project_id.as_ref()];
    let live = live_chapter_positions(client, scope).await?;
    let position = |chapter_id: &str| live.iter().position(|live| live == chapter_id);
    if position(claimed).is_none() {
        let not_live = ChapterCaptureFacts {
            live: false,
            ..ChapterCaptureFacts::default()
        };
        let skipped = skip(client, scope, claimed, &not_live).await?;
        let outcome = skipped.ok_or_else(|| missing("the claimed Chapter is not live"))?;
        return Ok((claimed.to_owned(), outcome));
    }
    let row = client
        .query_one(
            "SELECT clock_timestamp(),
                    (SELECT max(started_at) FROM storyos.memory_captures
                      WHERE (owner_user_id, project_id) = ($1::text::uuid, $2::text::uuid)
                        AND source_kind = 'chapter'),
                    EXISTS (SELECT 1 FROM storyos.agent_runs
                             WHERE (owner_user_id, project_id) = ($1::text::uuid, $2::text::uuid)
                               AND run_kind = 'author' AND status = 'completed')",
            &[&keys[0], &keys[1]],
        )
        .await
        .map_err(read_error)?;
    let now: SystemTime = row.get(/*idx*/ 0);
    // The read takes each due row, also a row that a concurrent claim locked. The claim is
    // Serializable, so of two concurrent claims of one Project, one fails with a serialization
    // failure, and an earlier Chapter in order stays a barrier for a later Chapter.
    let mut due: Vec<String> = vec![claimed.to_owned()];
    due.extend(
        client
            .query(
                "SELECT source_id::text FROM storyos.memory_work_schedules
                  WHERE (owner_user_id, project_id, work_kind) =
                        ($1::text::uuid, $2::text::uuid, $3)
                    AND due_at <= $4
                  ORDER BY source_id",
                &[&keys[0], &keys[1], &CHAPTER_CAPTURE, &now],
            )
            .await
            .map_err(read_error)?
            .iter()
            .map(|row| row.get::<_, String>(/*idx*/ 0))
            .filter(|chapter_id| position(chapter_id).is_some()),
    );
    let positions: Vec<usize> = due.iter().filter_map(|id| position(id)).collect();
    let turn = storyos_core::chapter_capture_turn(&positions, row.get(/*idx*/ 1), now)
        .ok_or_else(|| missing("the claimed Chapter row is missing"))?;
    let chapter_id = due[turn.first].clone();
    if chapter_id != claimed {
        let claimed_row = set_row(client, scope, claimed, (false, Some(claimed_due))).await;
        claimed_row.map_err(read_error)?;
        let chosen_row = set_row(client, scope, &chapter_id, (false, None)).await;
        chosen_row.map_err(read_error)?;
    }
    let read_id = ChapterId::new(chapter_id.clone());
    let extent = LiveChapterReadExtent::CurrentChapter {
        chapter_id: &read_id,
    };
    let chapter = read_live_chapter_blocks(client, scope, extent)
        .await?
        .pop()
        .ok_or_else(|| missing("the live Chapter is missing"))?;
    let captures = client
        .query_one(
            "SELECT COALESCE((SELECT chapter_revision_id::text = $4
                                FROM storyos.memory_captures
                               WHERE (owner_user_id, project_id, chapter_id) =
                                     ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                                 AND outcome IN ('observations_recorded', 'no_observation')
                               ORDER BY capture_id DESC
                               LIMIT 1), false),
                    EXISTS (SELECT 1 FROM storyos.memory_captures
                             WHERE (owner_user_id, project_id, chapter_id) =
                                   ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                               AND state = 'running')",
            &[&keys[0], &keys[1], &chapter_id, &chapter.head_revision_id],
        )
        .await
        .map_err(read_error)?;
    let text_empty = chapter
        .blocks
        .iter()
        .all(|block| block.text.trim().is_empty());
    let facts = ChapterCaptureFacts {
        live: true,
        text_empty,
        revision_read: captures.get(/*idx*/ 0),
        capture_in_progress: captures.get(/*idx*/ 1),
        rate_permits_at: turn.rate_permits_at,
        author_run_completed: row.get(/*idx*/ 2),
    };
    if let Some(outcome) = skip(client, scope, &chapter_id, &facts).await? {
        return Ok((chapter_id, outcome));
    }
    let blocks: Vec<String> = chapter.blocks.into_iter().map(|block| block.text).collect();
    let number = position(&chapter_id).map_or(0, |index| index + 1);
    let limit = storyos_core::CONTEXT_ITEM_TOKEN_LIMIT;
    let input = storyos_core::chapter_capture_input(&chapter.title, number, &blocks, limit);
    let source = MemoryCaptureSource::Chapter {
        chapter_id: chapter_id.clone(),
        revision_id: chapter.head_revision_id,
    };
    // A refused start writes no row of the capture, and the row becomes `held`.
    client
        .batch_execute("SAVEPOINT chapter_capture_start")
        .await
        .map_err(read_error)?;
    let started =
        crate::memory_capture::start(client, scope, &source, &input.text, input.projection_mode)
            .await?;
    match started {
        Ok(started) => Ok((chapter_id, MemoryWorkOutcome::Started(started))),
        Err(_) => {
            client
                .batch_execute("ROLLBACK TO SAVEPOINT chapter_capture_start")
                .await
                .map_err(read_error)?;
            let skip = ChapterCaptureSkip::StartRefused;
            apply(client, scope, &chapter_id, skip).await?;
            Ok((chapter_id, MemoryWorkOutcome::ChapterSkipped(skip)))
        }
    }
}

/// The live Chapters of the Project in the order of the Chapter list (S7-B1a).
async fn live_chapter_positions(
    client: &Client,
    scope: &ProjectScope,
) -> Result<Vec<String>, ProjectReadError> {
    Ok(client
        .query(
            "SELECT chapter.manuscript_object_id::text
               FROM storyos.manuscript_objects AS chapter
               JOIN storyos.manuscript_objects AS volume
                 ON (volume.owner_user_id, volume.project_id, volume.manuscript_object_id) =
                    (chapter.owner_user_id, chapter.project_id, chapter.parent_volume_id)
              WHERE (chapter.owner_user_id, chapter.project_id) =
                    ($1::text::uuid, $2::text::uuid)
                AND chapter.object_kind = 'chapter' AND volume.object_kind = 'volume'
                AND NOT EXISTS (SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                                 WHERE (removal.owner_user_id, removal.project_id,
                                        removal.chapter_id) =
                                       (chapter.owner_user_id, chapter.project_id,
                                        chapter.manuscript_object_id))
                AND NOT EXISTS (SELECT 1 FROM storyos.volume_removal_decisions AS removal
                                 WHERE (removal.owner_user_id, removal.project_id,
                                        removal.volume_id) =
                                       (volume.owner_user_id, volume.project_id,
                                        volume.manuscript_object_id))
              ORDER BY volume.tree_order, chapter.tree_order",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(read_error)?
        .iter()
        .map(|row| row.get(/*idx*/ 0))
        .collect())
}

/// Decides with Core for the row of `chapter_id`. A skip writes its change of the row and gives
/// its outcome. `None`: one capture starts.
async fn skip(
    client: &Client,
    scope: &ProjectScope,
    chapter_id: &str,
    facts: &ChapterCaptureFacts,
) -> Result<Option<MemoryWorkOutcome>, ProjectReadError> {
    let Err(skip) = storyos_core::decide_chapter_capture(facts) else {
        return Ok(None);
    };
    apply(client, scope, chapter_id, skip).await?;
    Ok(Some(MemoryWorkOutcome::ChapterSkipped(skip)))
}

/// Writes the change of the row of `chapter_id` that `skip` gives.
async fn apply(
    client: &Client,
    scope: &ProjectScope,
    chapter_id: &str,
    skip: ChapterCaptureSkip,
) -> Result<(), ProjectReadError> {
    let changed = match skip.row_change() {
        ChapterRowChange::DueEmpty => Ok(()),
        ChapterRowChange::DueAt(permitted) => {
            set_row(client, scope, chapter_id, (false, Some(permitted))).await
        }
        ChapterRowChange::Held => set_row(client, scope, chapter_id, (true, None)).await,
    };
    changed.map_err(read_error)
}

fn missing(message: &'static str) -> ProjectReadError {
    ProjectReadError::unavailable(std::io::Error::other(message))
}

/// Sets `held` and the due time of the row of `chapter_id`.
async fn set_row(
    client: &Client,
    scope: &ProjectScope,
    chapter_id: &str,
    (held, due_at): (bool, Option<SystemTime>),
) -> Result<(), tokio_postgres::Error> {
    let keys = [scope.owner_user_id.as_ref(), scope.project_id.as_ref()];
    client
        .execute(
            "UPDATE storyos.memory_work_schedules SET held = $5, due_at = $6
              WHERE (owner_user_id, project_id, work_kind, source_id) =
                    ($1::text::uuid, $2::text::uuid, $3, $4::text::uuid)",
            &[
                &keys[0],
                &keys[1],
                &CHAPTER_CAPTURE,
                &chapter_id,
                &held,
                &due_at,
            ],
        )
        .await
        .map(|_| ())
}

#[cfg(test)]
#[path = "chapter_capture_schedule_tests.rs"]
pub(crate) mod tests;
