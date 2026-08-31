use storyos_application::{
    DeleteVolumeCommand, DeleteVolumeError, DeleteVolumeSettlement, DeleteVolumeSettlementEffect,
};
use storyos_core::{
    DeleteVolume as CoreDeleteVolume, DeleteVolumeResult, ProjectLifecycle, ProjectPresence,
    VolumeChildPolicy, VolumeJoin, VolumeRemovalLifecycle, delete_volume as classify_delete_volume,
};
use uuid::Uuid;

use super::{delete_volume_database_error, delete_volume_parse_error};

pub(super) async fn persist_delete_volume(
    client: &tokio_postgres::Client,
    command: &DeleteVolumeCommand,
) -> Result<DeleteVolumeSettlement, DeleteVolumeError> {
    let row = client
        .query_opt(
            "SELECT lifecycle_state, tree_revision::text, current_chapter_id::text
               FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              FOR UPDATE",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(delete_volume_database_error)?;
    let Some(row) = row else {
        return Err(DeleteVolumeError::MissingProject);
    };
    let current_lifecycle = match row.get::<_, String>(0).as_str() {
        "active" => ProjectLifecycle::Active,
        "archived" => ProjectLifecycle::Archived,
        other => {
            return Err(DeleteVolumeError::Unavailable(Box::new(
                std::io::Error::other(format!("unsupported Project lifecycle {other}")),
            )));
        }
    };
    let current_volume_revision = row
        .get::<_, String>(1)
        .parse::<u64>()
        .map_err(delete_volume_parse_error)?;
    let current_chapter_id = row.get::<_, Option<String>>(2);
    let target = client
        .query_opt(
            "SELECT removal.volume_id IS NOT NULL
               FROM storyos.manuscript_objects AS volume
               LEFT JOIN storyos.volume_removal_decisions AS removal
                 ON (removal.owner_user_id, removal.project_id, removal.volume_id) =
                    (volume.owner_user_id, volume.project_id, volume.manuscript_object_id)
              WHERE volume.owner_user_id = $1::text::uuid AND volume.project_id = $2::text::uuid
                AND volume.manuscript_object_id = $3::text::uuid
                AND volume.object_kind = 'volume'
              FOR UPDATE OF volume",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.volume_id.as_ref(),
            ],
        )
        .await
        .map_err(delete_volume_database_error)?;
    let (volume_join, volume_lifecycle) = match target {
        Some(target) => (
            VolumeJoin::ExactScope,
            if target.get::<_, bool>(0) {
                VolumeRemovalLifecycle::Removed
            } else {
                VolumeRemovalLifecycle::Active
            },
        ),
        None => (VolumeJoin::Invalid, VolumeRemovalLifecycle::Active),
    };
    let child_chapters = if volume_join == VolumeJoin::ExactScope {
        let active_chapters = client
            .query_one(
                "SELECT count(*)::text
                   FROM storyos.manuscript_objects AS chapter
                  WHERE chapter.owner_user_id = $1::text::uuid
                    AND chapter.project_id = $2::text::uuid
                    AND chapter.parent_volume_id = $3::text::uuid
                    AND chapter.object_kind = 'chapter'
                    AND NOT EXISTS (
                      SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                       WHERE removal.owner_user_id = chapter.owner_user_id
                         AND removal.project_id = chapter.project_id
                         AND removal.chapter_id = chapter.manuscript_object_id
                    )",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &command.volume_id.as_ref(),
                ],
            )
            .await
            .map_err(delete_volume_database_error)?
            .get::<_, String>(0)
            .parse::<u64>()
            .map_err(delete_volume_parse_error)?;
        if active_chapters == 0 {
            VolumeChildPolicy::Empty
        } else {
            VolumeChildPolicy::Nonempty
        }
    } else {
        VolumeChildPolicy::Empty
    };
    let classified = classify_delete_volume(&CoreDeleteVolume {
        presence: ProjectPresence::Present,
        volume_join,
        volume_lifecycle,
        child_chapters,
        expected_volume_revision: command.expected_volume_revision,
        current_volume_revision,
        current_lifecycle,
    });
    let effect = match classified {
        DeleteVolumeResult::Applied { tree_revision } => DeleteVolumeSettlementEffect::Applied {
            tree_revision,
            volume_id: command.volume_id.as_ref().to_owned(),
        },
        DeleteVolumeResult::NoEffect { reason } => {
            DeleteVolumeSettlementEffect::NoEffect { reason }
        }
        DeleteVolumeResult::Conflicted { reason } => {
            DeleteVolumeSettlementEffect::Conflicted { reason }
        }
        DeleteVolumeResult::Refused {
            reason: storyos_core::DeleteVolumeRefusal::MissingProject,
        } => return Err(DeleteVolumeError::MissingProject),
        DeleteVolumeResult::Refused { reason } => DeleteVolumeSettlementEffect::Refused { reason },
    };
    insert_delete_volume_admission(client, command).await?;
    let (result_kind, result_payload) = match &effect {
        DeleteVolumeSettlementEffect::Applied { .. } => ("authoritative_applied", "{}".to_owned()),
        DeleteVolumeSettlementEffect::NoEffect { .. } => {
            ("no_effect", r#"{"reason":"already_removed"}"#.to_owned())
        }
        DeleteVolumeSettlementEffect::Conflicted { .. } => (
            "conflicted",
            r#"{"reason":"stale_volume_revision"}"#.to_owned(),
        ),
        DeleteVolumeSettlementEffect::Refused { reason } => {
            let refused = match reason {
                storyos_core::DeleteVolumeRefusal::ArchivedProject => "archived_project",
                storyos_core::DeleteVolumeRefusal::InvalidVolumeJoin => "invalid_volume_join",
                storyos_core::DeleteVolumeRefusal::NonemptyVolume => "nonempty_volume",
                storyos_core::DeleteVolumeRefusal::MissingProject => {
                    return Err(DeleteVolumeError::MissingProject);
                }
            };
            ("refused", format!(r#"{{"reason":"{refused}"}}"#))
        }
    };
    let receipt_created_at = client
        .query_one(
            "INSERT INTO storyos.domain_receipts
               (owner_user_id, project_id, receipt_id, author_command_admission_id,
                command_id, command_kind, command_digest, idempotency_key, producer_cause,
                expected_heads, prior_heads, resulting_heads, authoritative_revision_ids,
                proposal_revision_ids, authoritative_commit_ids, draft_artifact_refs,
                artifact_lifecycle_event_refs, condition_refs, result_kind, result_payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'deleteVolume', $6, $7::text::uuid,
                     'author_command_admission', '{}'::uuid[], '{}'::uuid[], '{}'::uuid[],
                     '{}'::uuid[], '{}'::uuid[], '{}'::uuid[], '{}'::text[], '{}'::text[],
                     '{}'::text[], $8, $9::text::jsonb)
          RETURNING to_char(created_at AT TIME ZONE 'UTC',
                            'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.ids.author_command_admission_id,
                &command.ids.command_id,
                &command.challenge_binding.canonical_command_digest,
                &command.challenge_binding.idempotency_key,
                &result_kind,
                &result_payload,
            ],
        )
        .await
        .map_err(delete_volume_database_error)?
        .get::<_, String>(0);
    client
        .execute(
            "INSERT INTO storyos.author_command_admission_settlements
               (owner_user_id, project_id, author_command_admission_id, settlement_kind, receipt_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid,
                     'receipt_settled', $4::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.author_command_admission_id,
                &command.ids.receipt_id,
            ],
        )
        .await
        .map_err(delete_volume_database_error)?;
    let mut project_activity_position = 0;
    let mut project_activity_event_id = String::new();
    if let DeleteVolumeSettlementEffect::Applied {
        tree_revision,
        volume_id,
    } = &effect
    {
        persist_removed_volume(client, command, tree_revision, volume_id).await?;
        project_activity_position = client
            .query_one(
                "INSERT INTO storyos.scope_counters AS counters
                   (owner_user_id, project_id, project_activity_position)
                 VALUES ($1::text::uuid, $2::text::uuid, 1)
                 ON CONFLICT (owner_user_id, project_id)
                 DO UPDATE SET
                   project_activity_position = counters.project_activity_position + 1
                 RETURNING counters.project_activity_position::text",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                ],
            )
            .await
            .map_err(delete_volume_database_error)?
            .get::<_, String>(0)
            .parse::<u64>()
            .map_err(delete_volume_parse_error)?;
        project_activity_event_id = Uuid::now_v7().to_string();
        let payload = serde_json::json!({
            "kind": "volume_deleted",
            "volume_id": volume_id,
            "tree_revision": tree_revision.to_string(),
        })
        .to_string();
        client
            .execute(
                "INSERT INTO storyos.project_activity_event_payloads
                   (owner_user_id, project_id, project_activity_position, project_activity_event_id,
                    event_kind, receipt_id, receipt_result_kind, payload)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                         'volume_deleted', $5::text::uuid, 'authoritative_applied',
                         $6::text::jsonb)",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &project_activity_position.to_string(),
                    &project_activity_event_id,
                    &command.ids.receipt_id,
                    &payload,
                ],
            )
            .await
            .map_err(delete_volume_database_error)?;
        if let Some(chapter_id) = current_chapter_id.as_deref() {
            persist_writer_base_snapshot(client, command, chapter_id, project_activity_position)
                .await?;
        } else {
            let snapshot_id = Uuid::now_v7().to_string();
            crate::snapshot::persist_canonical_snapshot(
                client,
                &command.project_scope,
                &snapshot_id,
                project_activity_position,
            )
            .await
            .map_err(delete_volume_database_error)?;
        }
    }
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'deleteVolume' AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(delete_volume_database_error)?;
    Ok(DeleteVolumeSettlement {
        ids: command.ids.clone(),
        effect,
        receipt_created_at,
        project_activity_position,
        project_activity_event_id,
    })
}

async fn persist_removed_volume(
    client: &tokio_postgres::Client,
    command: &DeleteVolumeCommand,
    tree_revision: &u64,
    volume_id: &str,
) -> Result<(), DeleteVolumeError> {
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
                &volume_id,
                &tree_revision.to_string(),
            ],
        )
        .await
        .map_err(delete_volume_database_error)?;
    let updated = client
        .execute(
            "UPDATE storyos.projects
                SET tree_revision = $3::text::bigint
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND tree_revision = $4::text::bigint AND lifecycle_state = 'active'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &tree_revision.to_string(),
                &command.expected_volume_revision.to_string(),
            ],
        )
        .await
        .map_err(delete_volume_database_error)?;
    if updated != 1 {
        return Err(DeleteVolumeError::Unavailable(Box::new(
            std::io::Error::other("tree revision changed under FOR UPDATE"),
        )));
    }
    Ok(())
}

async fn persist_writer_base_snapshot(
    client: &tokio_postgres::Client,
    command: &DeleteVolumeCommand,
    chapter_id: &str,
    project_activity_position: u64,
) -> Result<(), DeleteVolumeError> {
    let Some(authoritative_revision_id) = client
        .query_opt(
            "SELECT head.current_revision_id::text
               FROM storyos.authoritative_heads AS head
              WHERE head.owner_user_id = $1::text::uuid AND head.project_id = $2::text::uuid
                AND head.manuscript_object_id = $3::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &chapter_id,
            ],
        )
        .await
        .map_err(delete_volume_database_error)?
        .map(|row| row.get::<_, String>(0))
    else {
        return Err(DeleteVolumeError::Unavailable(Box::new(
            std::io::Error::other("current Chapter has no authoritative head"),
        )));
    };
    let snapshot_id = Uuid::now_v7().to_string();
    crate::snapshot::persist_canonical_snapshot(
        client,
        &command.project_scope,
        &snapshot_id,
        project_activity_position,
    )
    .await
    .map_err(delete_volume_database_error)?;
    let activity_position_text = project_activity_position.to_string();
    client
        .execute(
            "UPDATE storyos.editor_session_base_snapshots AS snapshot
                SET snapshot_id = $3::text::uuid,
                    chapter_object_id = $4::text::uuid,
                    authoritative_revision_id = $5::text::uuid,
                    project_activity_position = $6::text::numeric,
                    created_at = clock_timestamp()
               FROM storyos.project_writer_generations AS writer
              WHERE snapshot.owner_user_id = $1::text::uuid
                AND snapshot.project_id = $2::text::uuid
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
                &snapshot_id,
                &chapter_id,
                &authoritative_revision_id,
                &activity_position_text,
            ],
        )
        .await
        .map_err(delete_volume_database_error)?;
    Ok(())
}

async fn insert_delete_volume_admission(
    client: &tokio_postgres::Client,
    command: &DeleteVolumeCommand,
) -> Result<(), DeleteVolumeError> {
    let client_session_generation = command.client_binding.session_generation.to_string();
    let inserted = client
        .execute(
            "INSERT INTO storyos.author_command_admissions
               (owner_user_id, project_id, author_command_admission_id, command_id,
                editor_session_id, writer_generation, client_session_binding_ref,
                client_session_generation, client_contract_revision, security_policy_revision,
                action_class, method, route_template, command_schema, command_kind,
                canonical_command_digest, idempotency_key, challenge_consumed_at,
                challenge_expires_at, correlation_id, chapter_object_id,
                expected_authoritative_revision_id, expected_proposal_head_revision_ids,
                target_refs, observed_ownership_partition, editor_contract_revision,
                undo_group_id, completed_intent_record_id, local_intent_sequence, command_payload)
             SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                    NULL, NULL, $5, $6::text::numeric, $7, $8,
                    'explicit_project_command', $9, $10, $11, 'deleteVolume',
                    $12, $13::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $14::text::uuid, NULL, NULL, '{}'::uuid[], '{}'::text[], NULL, $7,
                    NULL, NULL, NULL, convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.project_command_challenges AS challenge
              WHERE challenge.owner_user_id = $1::text::uuid
                AND challenge.project_id = $2::text::uuid
                AND challenge.command_kind = 'deleteVolume'
                AND challenge.idempotency_key = $13::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.author_command_admission_id,
                &command.ids.command_id,
                &command.client_binding.binding_ref,
                &client_session_generation,
                &command.client_binding.client_contract_revision,
                &command.client_binding.security_policy_revision,
                &command.challenge_binding.method,
                &command.challenge_binding.route_template,
                &command.challenge_binding.command_schema,
                &command.challenge_binding.canonical_command_digest,
                &command.challenge_binding.idempotency_key,
                &command.correlation_id,
                &command.canonical_command_bytes.as_slice(),
            ],
        )
        .await
        .map_err(delete_volume_database_error)?;
    if inserted != 1 {
        return Err(DeleteVolumeError::InvalidChallenge);
    }
    Ok(())
}
