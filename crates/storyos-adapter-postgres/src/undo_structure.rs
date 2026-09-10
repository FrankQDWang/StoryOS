use storyos_application::{
    UndoLatestAuthorActionCommand, UndoLatestAuthorActionError, UndoLatestAuthorActionSettlement,
    UndoLatestAuthorActionSettlementEffect,
};
use uuid::Uuid;

use super::structural_authority_settlement::{
    StructureCommitBinding, persist_compensation_author_action, persist_structure_commit,
};
use super::undo_latest_author_action::{
    ObservedStructureFrontier, UndoReceiptAuthority, insert_undo_receipt, settle_idempotency,
    undo_database_error, undo_from_session,
};

pub(super) async fn persist_structure_compensation(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    frontier: &ObservedStructureFrontier,
    source_sequence: u64,
) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
    let sequences =
        crate::structural_authority_settlement::allocate_structure_transition_sequences(
            client,
            &command.project_scope,
        )
        .await
        .map_err(UndoLatestAuthorActionError::Unavailable)?;
    let updated = client
        .execute(
            "UPDATE storyos.projects
                SET tree_revision = $3::text::bigint
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND tree_revision = $4::text::bigint AND lifecycle_state = 'active'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &frontier.prior_manuscript_tree_revision.to_string(),
                &frontier.resulting_manuscript_tree_revision.to_string(),
            ],
        )
        .await
        .map_err(undo_database_error)?;
    if updated != 1 {
        return Err(UndoLatestAuthorActionError::Unavailable(Box::new(
            std::io::Error::other("tree revision changed under FOR UPDATE"),
        )));
    }
    let receipt_created_at = insert_undo_receipt(
        client,
        command,
        "authoritative_applied",
        "{}",
        &command.expected_authoritative_revision_id,
        &command.expected_authoritative_revision_id,
        UndoReceiptAuthority::Structure {
            commit_id: sequences.authoritative_commit_id.clone(),
        },
    )
    .await?;
    let decision_id = Uuid::now_v7().to_string();
    client
        .execute(
            "INSERT INTO storyos.volume_removal_decisions
               (owner_user_id, project_id, volume_removal_decision_id, receipt_id,
                volume_id, tree_revision)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::bigint)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &decision_id,
                &command.ids.receipt_id,
                &frontier.affected_volume_id,
                &frontier.prior_manuscript_tree_revision.to_string(),
            ],
        )
        .await
        .map_err(undo_database_error)?;
    persist_structure_commit(
        client,
        &command.project_scope,
        &sequences,
        &command.ids.author_command_admission_id,
        &command.ids.receipt_id,
        StructureCommitBinding {
            prior_manuscript_tree_revision: frontier.resulting_manuscript_tree_revision,
            resulting_manuscript_tree_revision: frontier.prior_manuscript_tree_revision,
            affected_volume_id: &frontier.affected_volume_id,
        },
    )
    .await
    .map_err(undo_database_error)?;
    persist_compensation_author_action(
        client,
        &command.project_scope,
        &sequences,
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
        effect: UndoLatestAuthorActionSettlementEffect::CompensatedStructure {
            source_sequence,
            author_action_sequence: sequences.author_action_sequence,
            authoritative_commit_id: sequences.authoritative_commit_id,
            snapshot_id: sequences.snapshot_id,
            author_undo_frontier_sequence,
        },
        receipt_created_at,
        project_activity_position: sequences.project_activity_position,
    })
}

pub(super) async fn editor_session_chapter(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
) -> Result<Option<String>, UndoLatestAuthorActionError> {
    let row = client
        .query_opt(
            "SELECT snapshot.chapter_object_id::text
               FROM storyos.editor_session_base_snapshots AS snapshot
              WHERE snapshot.owner_user_id = $1::text::uuid
                AND snapshot.project_id = $2::text::uuid
                AND snapshot.editor_session_id = $3::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.editor_session_id.as_ref(),
            ],
        )
        .await
        .map_err(undo_database_error)?;
    Ok(row.map(|row| row.get(0)))
}
