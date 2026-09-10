use storyos_application::{
    UndoLatestAuthorActionCommand, UndoLatestAuthorActionError, UndoLatestAuthorActionSettlement,
    UndoLatestAuthorActionSettlementEffect,
};
use uuid::Uuid;

use super::structural_authority_settlement::{
    StructureAffectedIdentity, StructureCommitBinding, persist_compensation_author_action,
    persist_structure_commit,
};
use super::undo_frontier::{ObservedStructureFrontier, ObservedStructureIdentity};
use super::undo_latest_author_action::{
    UndoReceiptAuthority, insert_undo_receipt, settle_idempotency, undo_database_error,
    undo_from_session,
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
    restore_volume_update_sibling_order(client, command, frontier).await?;
    restore_chapter_update_sibling_order(client, command, frontier).await?;
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
        ObservedStructureIdentity::Volume { .. }
        | ObservedStructureIdentity::VolumeUpdate { .. }
        | ObservedStructureIdentity::ChapterUpdate { .. } => client
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
        // Update Volume or Update Chapter Compensation restores title and Canonical Sibling Order.
        // It must not write a removal decision.
        ObservedStructureIdentity::VolumeUpdate { .. }
        | ObservedStructureIdentity::ChapterUpdate { .. } => {}
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

async fn restore_volume_update_sibling_order(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    frontier: &ObservedStructureFrontier,
) -> Result<(), UndoLatestAuthorActionError> {
    let ObservedStructureIdentity::VolumeUpdate {
        volume_id,
        prior_title,
        prior_order,
    } = &frontier.identity
    else {
        return Ok(());
    };
    let volumes = client
        .query(
            "SELECT manuscript_object_id::text
               FROM storyos.manuscript_objects AS volume
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND object_kind = 'volume'
                AND NOT EXISTS (
                  SELECT 1 FROM storyos.volume_removal_decisions AS removal
                   WHERE removal.owner_user_id = volume.owner_user_id
                     AND removal.project_id = volume.project_id
                     AND removal.volume_id = volume.manuscript_object_id
                )
              ORDER BY tree_order
              FOR UPDATE",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(undo_database_error)?;
    let ordered_ids = volumes
        .iter()
        .map(|volume| volume.get::<_, String>(0))
        .collect::<Vec<_>>();
    let Some(current_index) = ordered_ids.iter().position(|id| id == volume_id) else {
        return Err(UndoLatestAuthorActionError::Unavailable(Box::new(
            std::io::Error::other("updated Volume missing under FOR UPDATE"),
        )));
    };
    let current_order = current_index as u64 + 1;
    let updated = client
        .execute(
            "UPDATE storyos.manuscript_objects
                SET title = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND manuscript_object_id = $4::text::uuid AND object_kind = 'volume'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                prior_title,
                volume_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    if updated != 1 {
        return Err(UndoLatestAuthorActionError::Unavailable(Box::new(
            std::io::Error::other("Volume row changed under FOR UPDATE"),
        )));
    }
    if current_order == *prior_order {
        return Ok(());
    }
    if *prior_order < 1 || *prior_order as usize > ordered_ids.len() {
        return Err(UndoLatestAuthorActionError::Unavailable(Box::new(
            std::io::Error::other("prior Canonical Sibling Order is outside the live Volume set"),
        )));
    }
    let mut ids = ordered_ids;
    let moved = ids.remove(current_index);
    ids.insert((*prior_order - 1) as usize, moved);
    client
        .execute(
            "UPDATE storyos.manuscript_objects
                SET tree_order = tree_order + 1000000
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND object_kind = 'volume'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(undo_database_error)?;
    for (index, live_volume_id) in ids.iter().enumerate() {
        let tree_order = (index + 1).to_string();
        client
            .execute(
                "UPDATE storyos.manuscript_objects
                    SET tree_order = $3::text::bigint
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND manuscript_object_id = $4::text::uuid AND object_kind = 'volume'",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &tree_order,
                    live_volume_id,
                ],
            )
            .await
            .map_err(undo_database_error)?;
    }
    Ok(())
}

async fn restore_chapter_update_sibling_order(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    frontier: &ObservedStructureFrontier,
) -> Result<(), UndoLatestAuthorActionError> {
    let ObservedStructureIdentity::ChapterUpdate {
        chapter_id,
        prior_title,
        prior_order,
    } = &frontier.identity
    else {
        return Ok(());
    };
    let parent_volume_id = client
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
    let chapters = client
        .query(
            "SELECT manuscript_object_id::text
               FROM storyos.manuscript_objects AS chapter
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND object_kind = 'chapter'
                AND parent_volume_id = $3::text::uuid
                AND NOT EXISTS (
                  SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                   WHERE removal.owner_user_id = chapter.owner_user_id
                     AND removal.project_id = chapter.project_id
                     AND removal.chapter_id = chapter.manuscript_object_id
                )
              ORDER BY tree_order
              FOR UPDATE",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &parent_volume_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    let ordered_ids = chapters
        .iter()
        .map(|chapter| chapter.get::<_, String>(0))
        .collect::<Vec<_>>();
    let Some(current_index) = ordered_ids.iter().position(|id| id == chapter_id) else {
        return Err(UndoLatestAuthorActionError::Unavailable(Box::new(
            std::io::Error::other("updated Chapter missing under FOR UPDATE"),
        )));
    };
    let current_order = current_index as u64 + 1;
    let updated = client
        .execute(
            "UPDATE storyos.manuscript_objects
                SET title = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND manuscript_object_id = $4::text::uuid AND object_kind = 'chapter'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                prior_title,
                chapter_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    if updated != 1 {
        return Err(UndoLatestAuthorActionError::Unavailable(Box::new(
            std::io::Error::other("Chapter row changed under FOR UPDATE"),
        )));
    }
    if current_order == *prior_order {
        return Ok(());
    }
    if *prior_order < 1 || *prior_order as usize > ordered_ids.len() {
        return Err(UndoLatestAuthorActionError::Unavailable(Box::new(
            std::io::Error::other("prior Canonical Sibling Order is outside the live Chapter set"),
        )));
    }
    let mut ids = ordered_ids;
    let moved = ids.remove(current_index);
    ids.insert((*prior_order - 1) as usize, moved);
    client
        .execute(
            "UPDATE storyos.manuscript_objects
                SET tree_order = tree_order + 1000000
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND object_kind = 'chapter' AND parent_volume_id = $3::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &parent_volume_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    for (index, live_chapter_id) in ids.iter().enumerate() {
        let tree_order = (index + 1).to_string();
        client
            .execute(
                "UPDATE storyos.manuscript_objects
                    SET tree_order = $3::text::bigint
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND manuscript_object_id = $4::text::uuid AND object_kind = 'chapter'",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &tree_order,
                    live_chapter_id,
                ],
            )
            .await
            .map_err(undo_database_error)?;
    }
    Ok(())
}

fn compensation_commit_binding(frontier: &ObservedStructureFrontier) -> StructureCommitBinding<'_> {
    StructureCommitBinding {
        prior_manuscript_tree_revision: frontier.resulting_manuscript_tree_revision,
        resulting_manuscript_tree_revision: frontier.prior_manuscript_tree_revision,
        identity: match &frontier.identity {
            ObservedStructureIdentity::Volume { volume_id }
            | ObservedStructureIdentity::VolumeUpdate { volume_id, .. } => {
                StructureAffectedIdentity::Volume { volume_id }
            }
            ObservedStructureIdentity::Chapter { chapter_id }
            | ObservedStructureIdentity::ChapterUpdate { chapter_id, .. } => {
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
