use storyos_application::{
    UndoLatestAuthorActionCommand, UndoLatestAuthorActionError, UndoLatestAuthorActionSettlement,
    UndoLatestAuthorActionSettlementEffect,
};
use uuid::Uuid;

use super::structural_authority_settlement::{
    StructureAffectedIdentity, StructureCommitBinding, persist_compensation_author_action,
    persist_structure_commit,
};
use super::undo_latest_author_action::{
    ObservedStructureFrontier, ObservedStructureIdentity, UndoReceiptAuthority,
    insert_undo_receipt, settle_idempotency, undo_database_error, undo_from_session,
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
    restore_prior_tree(client, command, frontier).await?;
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
    persist_structure_removal(client, command, frontier).await?;
    persist_structure_commit(
        client,
        &command.project_scope,
        &sequences,
        &command.ids.author_command_admission_id,
        &command.ids.receipt_id,
        compensation_commit_binding(frontier),
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

async fn restore_prior_tree(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    frontier: &ObservedStructureFrontier,
) -> Result<(), UndoLatestAuthorActionError> {
    let updated = match &frontier.identity {
        ObservedStructureIdentity::Volume { .. } => client
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
            .map_err(undo_database_error)?,
        ObservedStructureIdentity::Chapter { chapter_id } => client
            .execute(
                "UPDATE storyos.projects
                    SET tree_revision = $3::text::bigint,
                        current_chapter_id = CASE
                          WHEN current_chapter_id = $5::text::uuid THEN NULL
                          ELSE current_chapter_id
                        END
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND tree_revision = $4::text::bigint AND lifecycle_state = 'active'",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &frontier.prior_manuscript_tree_revision.to_string(),
                    &frontier.resulting_manuscript_tree_revision.to_string(),
                    &chapter_id,
                ],
            )
            .await
            .map_err(undo_database_error)?,
    };
    if updated != 1 {
        return Err(UndoLatestAuthorActionError::Unavailable(Box::new(
            std::io::Error::other("tree revision changed under FOR UPDATE"),
        )));
    }
    Ok(())
}

async fn persist_structure_removal(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    frontier: &ObservedStructureFrontier,
) -> Result<(), UndoLatestAuthorActionError> {
    let decision_id = Uuid::now_v7().to_string();
    match &frontier.identity {
        ObservedStructureIdentity::Volume { volume_id } => {
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
                        &volume_id,
                        &frontier.prior_manuscript_tree_revision.to_string(),
                    ],
                )
                .await
                .map_err(undo_database_error)?;
        }
        ObservedStructureIdentity::Chapter { chapter_id } => {
            let volume_id = client
                .query_one(
                    "SELECT parent_volume_id::text
                       FROM storyos.manuscript_objects
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND manuscript_object_id = $3::text::uuid AND object_kind = 'chapter'",
                    &[
                        &command.project_scope.owner_user_id.as_ref(),
                        &command.project_scope.project_id.as_ref(),
                        &chapter_id,
                    ],
                )
                .await
                .map_err(undo_database_error)?
                .get::<_, String>(0);
            client
                .execute(
                    "INSERT INTO storyos.chapter_removal_decisions
                       (owner_user_id, project_id, chapter_removal_decision_id, receipt_id,
                        chapter_id, volume_id, tree_revision)
                     VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                             $5::text::uuid, $6::text::uuid, $7::text::bigint)",
                    &[
                        &command.project_scope.owner_user_id.as_ref(),
                        &command.project_scope.project_id.as_ref(),
                        &decision_id,
                        &command.ids.receipt_id,
                        &chapter_id,
                        &volume_id,
                        &frontier.prior_manuscript_tree_revision.to_string(),
                    ],
                )
                .await
                .map_err(undo_database_error)?;
        }
    }
    Ok(())
}

fn compensation_commit_binding(frontier: &ObservedStructureFrontier) -> StructureCommitBinding<'_> {
    StructureCommitBinding {
        prior_manuscript_tree_revision: frontier.resulting_manuscript_tree_revision,
        resulting_manuscript_tree_revision: frontier.prior_manuscript_tree_revision,
        identity: match &frontier.identity {
            ObservedStructureIdentity::Volume { volume_id } => {
                StructureAffectedIdentity::Volume { volume_id }
            }
            ObservedStructureIdentity::Chapter { chapter_id } => {
                StructureAffectedIdentity::Chapter { chapter_id }
            }
        },
    }
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
