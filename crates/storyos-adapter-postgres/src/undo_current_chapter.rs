use storyos_application::{
    UndoLatestAuthorActionCommand, UndoLatestAuthorActionError, UndoLatestAuthorActionSettlement,
    UndoLatestAuthorActionSettlementEffect,
};

use super::undo_frontier::ObservedCurrentChapterFrontier;
use super::undo_latest_author_action::{
    UndoReceiptAuthority, insert_undo_receipt, settle_idempotency, undo_database_error,
    undo_from_session,
};

pub(super) async fn live_chapter_is_lawful_target(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    chapter_id: &str,
) -> Result<bool, UndoLatestAuthorActionError> {
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

pub(super) async fn persist_current_chapter_compensation(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    frontier: &ObservedCurrentChapterFrontier,
    source_sequence: u64,
) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
    let sequences = crate::structural_authority_settlement::allocate_current_chapter_sequences(
        client,
        &command.project_scope,
    )
    .await
    .map_err(UndoLatestAuthorActionError::Unavailable)?;
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
        return Err(UndoLatestAuthorActionError::Unavailable(Box::new(
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
    let receipt_created_at = insert_undo_receipt(
        client,
        command,
        "authoritative_applied",
        "{}",
        &command.expected_authoritative_revision_id,
        &command.expected_authoritative_revision_id,
        UndoReceiptAuthority::None,
    )
    .await?;
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
        return Err(UndoLatestAuthorActionError::BindingConflict);
    }
    settle_idempotency(client, command).await?;
    let author_undo_frontier_sequence =
        crate::editor_session::current_author_undo_frontier_sequence(
            client,
            command.project_scope.owner_user_id.as_ref(),
            command.project_scope.project_id.as_ref(),
        )
        .await
        .map_err(undo_from_session)?;
    Ok(UndoLatestAuthorActionSettlement {
        ids: command.ids.clone(),
        effect: UndoLatestAuthorActionSettlementEffect::CompensatedCurrentChapter {
            source_sequence,
            author_action_sequence: sequences.author_action_sequence,
            snapshot_id: sequences.snapshot_id,
            author_undo_frontier_sequence,
        },
        receipt_created_at,
        project_activity_position: sequences.project_activity_position,
    })
}
