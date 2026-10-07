//! The structure Compensation of Author Undo (ADR 0029, ADR 0030, ADR 0044).

use storyos_application::{
    UndoLatestAuthorActionCommand, UndoLatestAuthorActionError, UndoLatestAuthorActionSettlement,
    UndoLatestAuthorActionSettlementEffect,
};
use storyos_core::AuthorUndoFrontierKind;
use tokio_postgres::Client;
use uuid::Uuid;

use super::{
    StructureAffectedIdentity, StructureCommitBinding, persist_compensation_author_action,
    persist_structure_commit,
};
use crate::undo_compensation::{CompensationAdapter, CompensationReplay, StructureCommand};
use crate::undo_latest_author_action::{
    UndoReceiptAuthority, insert_undo_receipt, settle_idempotency, undo_database_error,
    undo_from_session,
};

/// Restores the prior manuscript tree and identities of one Volume or Chapter change.
pub(crate) struct StructureCompensation;

pub(crate) struct ObservedStructureFrontier {
    pub sequence: u64,
    pub prior_manuscript_tree_revision: u64,
    pub resulting_manuscript_tree_revision: u64,
    pub identity: ObservedStructureIdentity,
}

pub(crate) enum ObservedStructureIdentity {
    Volume {
        volume_id: String,
    },
    VolumeDelete {
        volume_id: String,
    },
    VolumeUpdate {
        volume_id: String,
        prior_title: String,
        prior_order: u64,
    },
    Chapter {
        chapter_id: String,
    },
    ChapterDelete {
        chapter_id: String,
        prior_current_chapter_id: Option<String>,
    },
    ChapterUpdate {
        chapter_id: String,
        prior_title: String,
        prior_order: u64,
    },
}

impl CompensationAdapter for StructureCompensation {
    type Forward = StructureCommand;
    type Evidence = ObservedStructureFrontier;

    async fn load(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        forward: StructureCommand,
        sequence: u64,
    ) -> Result<Option<ObservedStructureFrontier>, UndoLatestAuthorActionError> {
        load_structure_frontier(client, command, forward, sequence).await
    }

    fn frontier_kind(_evidence: &ObservedStructureFrontier) -> AuthorUndoFrontierKind {
        AuthorUndoFrontierKind::ReversibleStructureTransition
    }

    async fn compensate(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        evidence: &ObservedStructureFrontier,
        source_sequence: u64,
    ) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
        persist_structure_compensation(client, command, evidence, source_sequence).await
    }

    async fn decode(
        _client: &Client,
        _command: &UndoLatestAuthorActionCommand,
        replay: &CompensationReplay,
    ) -> Result<UndoLatestAuthorActionSettlementEffect, UndoLatestAuthorActionError> {
        Ok(
            UndoLatestAuthorActionSettlementEffect::CompensatedStructure {
                source_sequence: replay.source_sequence,
                author_action_sequence: replay.author_action_sequence,
                authoritative_commit_id: replay
                    .authoritative_commit_id
                    .clone()
                    .ok_or(UndoLatestAuthorActionError::BindingConflict)?,
                snapshot_id: replay
                    .snapshot_id
                    .clone()
                    .ok_or(UndoLatestAuthorActionError::BindingConflict)?,
                author_undo_frontier_sequence: replay.author_undo_frontier_sequence,
            },
        )
    }
}

async fn load_structure_frontier(
    client: &Client,
    command: &UndoLatestAuthorActionCommand,
    forward: StructureCommand,
    sequence: u64,
) -> Result<Option<ObservedStructureFrontier>, UndoLatestAuthorActionError> {
    let Some(row) = client
        .query_opt(
            "SELECT commit.manuscript_object_id::text, commit.resulting_revision_id::text,
                    commit.prior_revision_id::text, head.current_revision_id::text,
                    commit.prior_manuscript_tree_revision::text,
                    commit.resulting_manuscript_tree_revision::text,
                    commit.affected_volume_id::text, commit.affected_chapter_id::text,
                    payload.payload->>'prior_title', payload.payload->>'prior_order',
                    payload.payload->>'prior_current_chapter_id'
               FROM storyos.author_action_entries AS action
               JOIN storyos.authoritative_commits AS commit
                 ON (commit.owner_user_id, commit.project_id, commit.receipt_id,
                     commit.receipt_result_kind, commit.authoritative_commit_id) =
                    (action.owner_user_id, action.project_id, action.receipt_id,
                     action.receipt_result_kind, action.authoritative_commit_id)
          LEFT JOIN storyos.authoritative_heads AS head
                 ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                    (commit.owner_user_id, commit.project_id, commit.manuscript_object_id)
          LEFT JOIN storyos.project_activity_event_payloads AS payload
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
    let (object_id, resulting_revision_id, prior_revision_id, head_revision_id) = (
        row.get::<_, Option<String>>(/*idx*/ 0),
        row.get::<_, Option<String>>(/*idx*/ 1),
        row.get::<_, Option<String>>(/*idx*/ 2),
        row.get::<_, Option<String>>(/*idx*/ 3),
    );
    let (Some(prior_tree), Some(resulting_tree)) = (
        row.get::<_, Option<String>>(/*idx*/ 4),
        row.get::<_, Option<String>>(/*idx*/ 5),
    ) else {
        return Ok(None);
    };
    let tree_only =
        object_id.is_none() && resulting_revision_id.is_none() && prior_revision_id.is_none();
    let prior_title_order = || -> Result<Option<(String, u64)>, UndoLatestAuthorActionError> {
        match (
            row.get::<_, Option<String>>(/*idx*/ 8),
            row.get::<_, Option<String>>(/*idx*/ 9),
        ) {
            (Some(title), Some(order)) => Ok(Some((title, order.parse().map_err(parse_error)?))),
            (None, _) | (_, None) => Ok(None),
        }
    };
    let identity = match (
        forward,
        row.get::<_, Option<String>>(/*idx*/ 6),
        row.get::<_, Option<String>>(/*idx*/ 7),
    ) {
        (StructureCommand::CreateVolume, Some(volume_id), None) if tree_only => {
            ObservedStructureIdentity::Volume { volume_id }
        }
        (StructureCommand::DeleteVolume, Some(volume_id), None) if tree_only => {
            ObservedStructureIdentity::VolumeDelete { volume_id }
        }
        (StructureCommand::UpdateVolume, Some(volume_id), None) if tree_only => {
            let Some((prior_title, prior_order)) = prior_title_order()? else {
                return Ok(None);
            };
            ObservedStructureIdentity::VolumeUpdate {
                volume_id,
                prior_title,
                prior_order,
            }
        }
        (StructureCommand::UpdateChapter, None, Some(chapter_id)) if tree_only => {
            let Some((prior_title, prior_order)) = prior_title_order()? else {
                return Ok(None);
            };
            ObservedStructureIdentity::ChapterUpdate {
                chapter_id,
                prior_title,
                prior_order,
            }
        }
        (StructureCommand::DeleteChapter, None, Some(chapter_id)) if tree_only => {
            ObservedStructureIdentity::ChapterDelete {
                chapter_id,
                prior_current_chapter_id: row.get(/*idx*/ 10),
            }
        }
        (StructureCommand::CreateChapter, None, Some(chapter_id))
            if object_id.as_deref() == Some(chapter_id.as_str())
                && resulting_revision_id.is_some()
                && prior_revision_id.is_none()
                && head_revision_id.is_some() =>
        {
            ObservedStructureIdentity::Chapter { chapter_id }
        }
        _ => return Ok(None),
    };
    Ok(Some(ObservedStructureFrontier {
        sequence,
        prior_manuscript_tree_revision: prior_tree.parse().map_err(parse_error)?,
        resulting_manuscript_tree_revision: resulting_tree.parse().map_err(parse_error)?,
        identity,
    }))
}

fn parse_error(error: std::num::ParseIntError) -> UndoLatestAuthorActionError {
    UndoLatestAuthorActionError::Unavailable(Box::new(error))
}

async fn persist_structure_compensation(
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
    let response_project = settle_idempotency(client, command).await?;
    let author_undo_frontier_sequence =
        crate::editor_session::current_author_undo_frontier_sequence(
            client,
            command.project_scope.owner_user_id.as_ref(),
            command.project_scope.project_id.as_ref(),
        )
        .await
        .map_err(undo_from_session)?;
    Ok(UndoLatestAuthorActionSettlement {
        source_reopen_event: None,
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
        response_project,
    })
}

async fn restore_prior_tree(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    frontier: &ObservedStructureFrontier,
) -> Result<(), UndoLatestAuthorActionError> {
    let updated = match &frontier.identity {
        ObservedStructureIdentity::Volume { .. }
        | ObservedStructureIdentity::VolumeDelete { .. }
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
        ObservedStructureIdentity::ChapterDelete {
            prior_current_chapter_id,
            ..
        } => client
            .execute(
                "UPDATE storyos.projects
                    SET tree_revision = $3::text::bigint,
                        current_chapter_id = $5::text::uuid
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND tree_revision = $4::text::bigint AND lifecycle_state = 'active'",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &frontier.prior_manuscript_tree_revision.to_string(),
                    &frontier.resulting_manuscript_tree_revision.to_string(),
                    &prior_current_chapter_id.as_deref(),
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
        ObservedStructureIdentity::ChapterDelete { chapter_id, .. } => {
            client
                .execute(
                    "DELETE FROM storyos.chapter_removal_decisions
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND chapter_id = $3::text::uuid",
                    &[
                        &command.project_scope.owner_user_id.as_ref(),
                        &command.project_scope.project_id.as_ref(),
                        &chapter_id,
                    ],
                )
                .await
                .map_err(undo_database_error)?;
        }
        ObservedStructureIdentity::VolumeDelete { volume_id } => {
            client
                .execute(
                    "DELETE FROM storyos.volume_removal_decisions
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND volume_id = $3::text::uuid",
                    &[
                        &command.project_scope.owner_user_id.as_ref(),
                        &command.project_scope.project_id.as_ref(),
                        &volume_id,
                    ],
                )
                .await
                .map_err(undo_database_error)?;
        }
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
    crate::volume_storage_order::persist_volume_storage_order(client, &command.project_scope, &ids)
        .await
        .map_err(UndoLatestAuthorActionError::Unavailable)?;
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
            | ObservedStructureIdentity::VolumeDelete { volume_id }
            | ObservedStructureIdentity::VolumeUpdate { volume_id, .. } => {
                StructureAffectedIdentity::Volume { volume_id }
            }
            ObservedStructureIdentity::Chapter { chapter_id }
            | ObservedStructureIdentity::ChapterDelete { chapter_id, .. }
            | ObservedStructureIdentity::ChapterUpdate { chapter_id, .. } => {
                StructureAffectedIdentity::Chapter { chapter_id }
            }
        },
    }
}
