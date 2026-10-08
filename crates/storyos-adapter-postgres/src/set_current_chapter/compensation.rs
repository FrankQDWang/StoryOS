//! The Current Chapter Compensation of Author Undo (ADR 0026, ADR 0044).

use storyos_application::{ProjectCommandError, ProjectScope, UndoRecords};
use storyos_core::AuthorUndoFrontierKind;
use tokio_postgres::Client;

use crate::command_replay::ReplayFault;
use crate::structural_authority_settlement::{
    CurrentChapterSequences, allocate_current_chapter_sequences,
};
use crate::undo_compensation::{CompensationAdapter, CompensationReplay, UndoRequest};
use crate::undo_latest_author_action::undo_database_error;

/// Returns the writer to the Chapter that was current before a Current Chapter change.
pub(crate) struct CurrentChapterCompensation;

pub(crate) struct ObservedCurrentChapterFrontier {
    pub sequence: u64,
    pub prior_chapter_id: String,
    pub resulting_chapter_id: String,
}

impl CompensationAdapter for CurrentChapterCompensation {
    type Forward = ();
    type Evidence = ObservedCurrentChapterFrontier;
    type Sequences = CurrentChapterSequences;

    /// A prior Chapter that is not a lawful target makes the change a Barrier.
    async fn load(
        client: &Client,
        command: &UndoRequest,
        _forward: (),
        sequence: u64,
    ) -> Result<Option<ObservedCurrentChapterFrontier>, ProjectCommandError> {
        let Some(row) = client
            .query_opt(
                "SELECT payload.payload->>'prior_chapter_id', payload.payload->>'current_chapter_id'
                   FROM storyos.author_action_entries AS action
                   JOIN storyos.project_activity_event_payloads AS payload
                     ON (payload.owner_user_id, payload.project_id, payload.receipt_id) =
                        (action.owner_user_id, action.project_id, action.receipt_id)
                  WHERE action.owner_user_id = $1::text::uuid
                    AND action.project_id = $2::text::uuid
                    AND action.author_action_sequence = $3::text::numeric",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &sequence.to_string(),
                ],
            )
            .await
            .map_err(undo_database_error)?
        else {
            return Ok(None);
        };
        let (Some(prior_chapter_id), Some(resulting_chapter_id)) = (
            row.get::<_, Option<String>>(/*idx*/ 0),
            row.get::<_, Option<String>>(/*idx*/ 1),
        ) else {
            return Ok(None);
        };
        if !live_chapter_is_lawful_target(client, command, &prior_chapter_id).await? {
            return Ok(None);
        }
        Ok(Some(ObservedCurrentChapterFrontier {
            sequence,
            prior_chapter_id,
            resulting_chapter_id,
        }))
    }

    fn frontier_kind(_evidence: &ObservedCurrentChapterFrontier) -> AuthorUndoFrontierKind {
        AuthorUndoFrontierKind::ReversibleStructureTransition
    }

    async fn allocate(
        client: &Client,
        scope: &ProjectScope,
    ) -> Result<CurrentChapterSequences, ProjectCommandError> {
        allocate_current_chapter_sequences(client, scope)
            .await
            .map_err(ProjectCommandError::Unavailable)
    }

    async fn compensate(
        client: &Client,
        command: &UndoRequest,
        evidence: &ObservedCurrentChapterFrontier,
        sequences: CurrentChapterSequences,
        source_sequence: u64,
    ) -> Result<UndoRecords, ProjectCommandError> {
        persist_current_chapter_compensation(client, command, evidence, sequences, source_sequence)
            .await
    }

    fn decode(replay: &CompensationReplay) -> Result<UndoRecords, ReplayFault> {
        let Some((snapshot_id, project_activity_position)) = replay.snapshot.clone() else {
            return Err(ReplayFault::Unavailable(
                "a Current Chapter Compensation has no Snapshot".into(),
            ));
        };
        Ok(UndoRecords::CurrentChapter {
            snapshot_id,
            project_activity_position,
        })
    }
}

async fn live_chapter_is_lawful_target(
    client: &tokio_postgres::Client,
    command: &UndoRequest,
    chapter_id: &str,
) -> Result<bool, ProjectCommandError> {
    let row = client
        .query_opt(
            "SELECT chapter.manuscript_object_id
               FROM storyos.manuscript_objects AS chapter
              WHERE chapter.owner_user_id = $1::text::uuid
                AND chapter.project_id = $2::text::uuid
                AND chapter.object_kind = 'chapter'
                AND chapter.manuscript_object_id = $3::text::uuid
                AND NOT EXISTS (
                  SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                   WHERE removal.owner_user_id = chapter.owner_user_id
                     AND removal.project_id = chapter.project_id
                     AND removal.chapter_id = chapter.manuscript_object_id
                )",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &chapter_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    Ok(row.is_some())
}

async fn persist_current_chapter_compensation(
    client: &tokio_postgres::Client,
    command: &UndoRequest,
    frontier: &ObservedCurrentChapterFrontier,
    sequences: CurrentChapterSequences,
    source_sequence: u64,
) -> Result<UndoRecords, ProjectCommandError> {
    let updated = client
        .execute(
            "UPDATE storyos.projects
                SET current_chapter_id = $3::text::uuid
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND lifecycle_state = 'active'
                AND current_chapter_id = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &frontier.prior_chapter_id,
                &frontier.resulting_chapter_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    if updated != 1 {
        return Err(ProjectCommandError::Unavailable(Box::new(
            std::io::Error::other("current Chapter changed under FOR UPDATE"),
        )));
    }
    let prior_head = client
        .query_one(
            "SELECT head.current_revision_id::text
               FROM storyos.authoritative_heads AS head
              WHERE head.owner_user_id = $1::text::uuid
                AND head.project_id = $2::text::uuid
                AND head.manuscript_object_id = $3::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &frontier.prior_chapter_id,
            ],
        )
        .await
        .map_err(undo_database_error)?
        .get::<_, String>(0);
    crate::structural_authority_settlement::persist_current_chapter_compensation_author_action(
        client,
        &command.project_scope,
        sequences.author_action_sequence,
        &command.ids.receipt_id,
        source_sequence,
    )
    .await
    .map_err(undo_database_error)?;
    crate::snapshot::persist_canonical_snapshot(
        client,
        &command.project_scope,
        &sequences.snapshot_id,
        sequences.project_activity_position,
    )
    .await
    .map_err(undo_database_error)?;
    let activity_position_text = sequences.project_activity_position.to_string();
    let base_updates = client
        .execute(
            "UPDATE storyos.editor_session_base_snapshots AS snapshot
                SET snapshot_id = $4::text::uuid,
                    chapter_object_id = $5::text::uuid,
                    authoritative_revision_id = $6::text::uuid,
                    project_activity_position = $7::text::numeric,
                    created_at = clock_timestamp()
               FROM storyos.project_writer_generations AS writer
              WHERE snapshot.owner_user_id = $1::text::uuid
                AND snapshot.project_id = $2::text::uuid
                AND snapshot.editor_session_id = $3::text::uuid
                AND (writer.owner_user_id, writer.project_id,
                     writer.current_editor_session_id) =
                    (snapshot.owner_user_id, snapshot.project_id,
                     snapshot.editor_session_id)
                AND writer.writer_generation = (
                  SELECT max(current_writer.writer_generation)
                    FROM storyos.project_writer_generations AS current_writer
                   WHERE current_writer.owner_user_id = snapshot.owner_user_id
                     AND current_writer.project_id = snapshot.project_id
                )",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.editor_session_id.as_ref(),
                &sequences.snapshot_id,
                &frontier.prior_chapter_id,
                &prior_head,
                &activity_position_text,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    if base_updates != 1 {
        return Err(ProjectCommandError::BindingConflict);
    }
    Ok(UndoRecords::CurrentChapter {
        snapshot_id: sequences.snapshot_id,
        project_activity_position: sequences.project_activity_position,
    })
}
