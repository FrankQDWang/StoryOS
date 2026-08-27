use storyos_application::{
    UpdateVolumeCommand, UpdateVolumeError, UpdateVolumeSettlement, UpdateVolumeSettlementEffect,
};
use storyos_core::{
    ProjectLifecycle, ProjectPresence, UpdateVolume as CoreUpdateVolume, UpdateVolumeResult,
    VolumeJoin, update_volume as classify_update_volume,
};
use uuid::Uuid;

use super::{update_volume_database_error, update_volume_parse_error};

pub(super) async fn persist_update_volume(
    client: &tokio_postgres::Client,
    command: &UpdateVolumeCommand,
) -> Result<UpdateVolumeSettlement, UpdateVolumeError> {
    let row = client
        .query_opt(
            "SELECT lifecycle_state, tree_revision::text FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              FOR UPDATE",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(update_volume_database_error)?;
    let Some(row) = row else {
        return Err(UpdateVolumeError::MissingProject);
    };
    let current_lifecycle = match row.get::<_, String>(0).as_str() {
        "active" => ProjectLifecycle::Active,
        "archived" => ProjectLifecycle::Archived,
        other => {
            return Err(UpdateVolumeError::Unavailable(Box::new(
                std::io::Error::other(format!("unsupported Project lifecycle {other}")),
            )));
        }
    };
    let current_volume_revision = row
        .get::<_, String>(1)
        .parse::<u64>()
        .map_err(update_volume_parse_error)?;
    let volumes = client
        .query(
            "SELECT manuscript_object_id::text, title, tree_order::text
               FROM storyos.manuscript_objects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND object_kind = 'volume'
              ORDER BY tree_order
              FOR UPDATE",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(update_volume_database_error)?;
    let volume_count = volumes.len() as u64;
    let current = volumes
        .iter()
        .find(|volume| volume.get::<_, String>(0) == command.volume_id.as_ref());
    let (volume_join, current_title, current_order, ordered_ids) = match current {
        Some(volume) => (
            VolumeJoin::ExactScope,
            volume.get::<_, String>(1),
            volume
                .get::<_, String>(2)
                .parse::<u64>()
                .map_err(update_volume_parse_error)?,
            volumes
                .iter()
                .map(|volume| volume.get::<_, String>(0))
                .collect::<Vec<_>>(),
        ),
        None => (VolumeJoin::Invalid, String::new(), 1, Vec::new()),
    };
    let classified = classify_update_volume(&CoreUpdateVolume {
        presence: ProjectPresence::Present,
        volume_join,
        expected_volume_revision: command.expected_volume_revision,
        current_volume_revision,
        current_lifecycle,
        title: command.title.clone(),
        current_title,
        order: command.order,
        current_order,
        volume_count,
    });
    let effect = match classified {
        UpdateVolumeResult::Applied {
            title,
            order,
            tree_revision,
        } => UpdateVolumeSettlementEffect::Applied {
            title,
            order,
            tree_revision,
        },
        UpdateVolumeResult::NoEffect { reason } => {
            UpdateVolumeSettlementEffect::NoEffect { reason }
        }
        UpdateVolumeResult::Conflicted { reason } => {
            UpdateVolumeSettlementEffect::Conflicted { reason }
        }
        UpdateVolumeResult::Refused {
            reason: storyos_core::UpdateVolumeRefusal::MissingProject,
        } => return Err(UpdateVolumeError::MissingProject),
        UpdateVolumeResult::Refused { reason } => UpdateVolumeSettlementEffect::Refused { reason },
    };
    insert_update_volume_admission(client, command).await?;
    let (result_kind, result_payload) = match &effect {
        UpdateVolumeSettlementEffect::Applied { .. } => ("authoritative_applied", "{}".to_owned()),
        UpdateVolumeSettlementEffect::NoEffect { .. } => {
            ("no_effect", r#"{"reason":"unchanged"}"#.to_owned())
        }
        UpdateVolumeSettlementEffect::Conflicted { .. } => (
            "conflicted",
            r#"{"reason":"stale_volume_revision"}"#.to_owned(),
        ),
        UpdateVolumeSettlementEffect::Refused { reason } => {
            let refused = match reason {
                storyos_core::UpdateVolumeRefusal::ArchivedProject => "archived_project",
                storyos_core::UpdateVolumeRefusal::InvalidTitle => "invalid_title",
                storyos_core::UpdateVolumeRefusal::InvalidOrder => "invalid_order",
                storyos_core::UpdateVolumeRefusal::InvalidVolumeJoin => "invalid_volume_join",
                storyos_core::UpdateVolumeRefusal::MissingProject => {
                    return Err(UpdateVolumeError::MissingProject);
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
                     $5::text::uuid, 'updateVolume', $6, $7::text::uuid,
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
        .map_err(update_volume_database_error)?
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
        .map_err(update_volume_database_error)?;
    let mut project_activity_position = 0;
    let mut project_activity_event_id = String::new();
    if let UpdateVolumeSettlementEffect::Applied {
        title,
        order,
        tree_revision,
    } = &effect
    {
        apply_volume_tree(
            client,
            command,
            &ordered_ids,
            current_order,
            title,
            *order,
            *tree_revision,
        )
        .await?;
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
            .map_err(update_volume_database_error)?
            .get::<_, String>(0)
            .parse::<u64>()
            .map_err(update_volume_parse_error)?;
        project_activity_event_id = Uuid::now_v7().to_string();
        let payload = serde_json::json!({
            "kind": "volume_updated",
            "volume_id": command.volume_id.as_ref(),
            "title": title,
            "tree_revision": tree_revision.to_string(),
            "order": order.to_string(),
        })
        .to_string();
        client
            .execute(
                "INSERT INTO storyos.project_activity_event_payloads
                   (owner_user_id, project_id, project_activity_position, project_activity_event_id,
                    event_kind, receipt_id, receipt_result_kind, payload)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                         'volume_updated', $5::text::uuid, 'authoritative_applied',
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
            .map_err(update_volume_database_error)?;
    }
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'updateVolume' AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(update_volume_database_error)?;
    Ok(UpdateVolumeSettlement {
        ids: command.ids.clone(),
        effect,
        receipt_created_at,
        project_activity_position,
        project_activity_event_id,
    })
}

async fn apply_volume_tree(
    client: &tokio_postgres::Client,
    command: &UpdateVolumeCommand,
    ordered_ids: &[String],
    current_order: u64,
    title: &str,
    order: u64,
    tree_revision: u64,
) -> Result<(), UpdateVolumeError> {
    let updated = client
        .execute(
            "UPDATE storyos.manuscript_objects
                SET title = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND manuscript_object_id = $4::text::uuid AND object_kind = 'volume'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &title,
                &command.volume_id.as_ref(),
            ],
        )
        .await
        .map_err(update_volume_database_error)?;
    if updated != 1 {
        return Err(UpdateVolumeError::Unavailable(Box::new(
            std::io::Error::other("Volume row changed under FOR UPDATE"),
        )));
    }
    if current_order != order {
        let mut ids = ordered_ids.to_vec();
        let moved = ids.remove((current_order - 1) as usize);
        ids.insert((order - 1) as usize, moved);
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
            .map_err(update_volume_database_error)?;
        for (index, volume_id) in ids.iter().enumerate() {
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
                        volume_id,
                    ],
                )
                .await
                .map_err(update_volume_database_error)?;
        }
    }
    let bumped = client
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
        .map_err(update_volume_database_error)?;
    if bumped != 1 {
        return Err(UpdateVolumeError::Unavailable(Box::new(
            std::io::Error::other("tree revision changed under FOR UPDATE"),
        )));
    }
    Ok(())
}

async fn insert_update_volume_admission(
    client: &tokio_postgres::Client,
    command: &UpdateVolumeCommand,
) -> Result<(), UpdateVolumeError> {
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
                    'explicit_project_command', $9, $10, $11, 'updateVolume',
                    $12, $13::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $14::text::uuid, NULL, NULL, '{}'::uuid[], '{}'::text[], NULL, $7,
                    NULL, NULL, NULL, convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.project_command_challenges AS challenge
              WHERE challenge.owner_user_id = $1::text::uuid
                AND challenge.project_id = $2::text::uuid
                AND challenge.command_kind = 'updateVolume'
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
        .map_err(update_volume_database_error)?;
    if inserted != 1 {
        return Err(UpdateVolumeError::InvalidChallenge);
    }
    Ok(())
}
