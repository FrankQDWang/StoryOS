use storyos_application::{
    DeleteChapterCommand, DeleteChapterError, DeleteChapterSettlement,
    DeleteChapterSettlementEffect,
};
use storyos_core::{
    ChapterJoin, ChapterRemovalLifecycle, DeleteChapter as CoreDeleteChapter, DeleteChapterCurrent,
    DeleteChapterResult, ProjectLifecycle, ProjectPresence,
    delete_chapter as classify_delete_chapter,
};
use uuid::Uuid;

use super::{delete_chapter_database_error, delete_chapter_parse_error};

const ACTIVE_CHAPTER_PREDICATE: &str = "NOT EXISTS (
                 SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                  WHERE removal.owner_user_id = chapter.owner_user_id
                    AND removal.project_id = chapter.project_id
                    AND removal.chapter_id = chapter.manuscript_object_id
               )";

pub(super) async fn persist_delete_chapter(
    client: &tokio_postgres::Client,
    command: &DeleteChapterCommand,
) -> Result<DeleteChapterSettlement, DeleteChapterError> {
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
        .map_err(delete_chapter_database_error)?;
    let Some(row) = row else {
        return Err(DeleteChapterError::MissingProject);
    };
    let current_lifecycle = match row.get::<_, String>(0).as_str() {
        "active" => ProjectLifecycle::Active,
        "archived" => ProjectLifecycle::Archived,
        other => {
            return Err(DeleteChapterError::Unavailable(Box::new(
                std::io::Error::other(format!("unsupported Project lifecycle {other}")),
            )));
        }
    };
    let current_chapter_revision = row
        .get::<_, String>(1)
        .parse::<u64>()
        .map_err(delete_chapter_parse_error)?;
    let current_chapter_id = row.get::<_, Option<String>>(2);
    let target = client
        .query_opt(
            "SELECT chapter.parent_volume_id::text, removal.chapter_id IS NOT NULL
               FROM storyos.manuscript_objects AS chapter
               LEFT JOIN storyos.chapter_removal_decisions AS removal
                 ON (removal.owner_user_id, removal.project_id, removal.chapter_id) =
                    (chapter.owner_user_id, chapter.project_id, chapter.manuscript_object_id)
              WHERE chapter.owner_user_id = $1::text::uuid AND chapter.project_id = $2::text::uuid
                AND chapter.manuscript_object_id = $3::text::uuid
                AND chapter.object_kind = 'chapter'
              FOR UPDATE OF chapter",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.chapter_id.as_ref(),
            ],
        )
        .await
        .map_err(delete_chapter_database_error)?;
    let (chapter_join, chapter_lifecycle, volume_id) = match target {
        Some(target) => {
            let volume_id = target.get::<_, String>(0);
            let removed = target.get::<_, bool>(1);
            (
                ChapterJoin::ExactScope,
                if removed {
                    ChapterRemovalLifecycle::Removed
                } else {
                    ChapterRemovalLifecycle::Active
                },
                volume_id,
            )
        }
        None => (
            ChapterJoin::Invalid,
            ChapterRemovalLifecycle::Active,
            String::new(),
        ),
    };
    let ordered_rows = client
        .query(
            &format!(
                "SELECT chapter.manuscript_object_id::text
                   FROM storyos.manuscript_objects AS chapter
                   JOIN storyos.manuscript_objects AS volume
                     ON (volume.owner_user_id, volume.project_id, volume.manuscript_object_id) =
                        (chapter.owner_user_id, chapter.project_id, chapter.parent_volume_id)
                    AND volume.object_kind = 'volume'
                  WHERE chapter.owner_user_id = $1::text::uuid AND chapter.project_id = $2::text::uuid
                    AND chapter.object_kind = 'chapter'
                    AND {ACTIVE_CHAPTER_PREDICATE}
                  ORDER BY volume.tree_order, chapter.tree_order
                  FOR UPDATE"
            ),
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(delete_chapter_database_error)?;
    let ordered_active_chapter_ids = ordered_rows
        .iter()
        .map(|chapter| chapter.get::<_, String>(0))
        .collect::<Vec<_>>();
    let classified = classify_delete_chapter(&CoreDeleteChapter {
        presence: ProjectPresence::Present,
        chapter_join,
        chapter_lifecycle,
        expected_chapter_revision: command.expected_chapter_revision,
        current_chapter_revision,
        current_lifecycle,
        chapter_id: command.chapter_id.as_ref().to_owned(),
        current_chapter_id,
        ordered_active_chapter_ids,
    });
    let effect = match classified {
        DeleteChapterResult::Applied {
            tree_revision,
            current,
        } => DeleteChapterSettlementEffect::Applied {
            tree_revision,
            volume_id,
            current,
        },
        DeleteChapterResult::NoEffect { reason } => {
            DeleteChapterSettlementEffect::NoEffect { reason }
        }
        DeleteChapterResult::Conflicted { reason } => {
            DeleteChapterSettlementEffect::Conflicted { reason }
        }
        DeleteChapterResult::Refused {
            reason: storyos_core::DeleteChapterRefusal::MissingProject,
        } => return Err(DeleteChapterError::MissingProject),
        DeleteChapterResult::Refused { reason } => {
            DeleteChapterSettlementEffect::Refused { reason }
        }
    };
    insert_delete_chapter_admission(client, command).await?;
    let (result_kind, result_payload) = match &effect {
        DeleteChapterSettlementEffect::Applied { .. } => ("authoritative_applied", "{}".to_owned()),
        DeleteChapterSettlementEffect::NoEffect { .. } => {
            ("no_effect", r#"{"reason":"already_removed"}"#.to_owned())
        }
        DeleteChapterSettlementEffect::Conflicted { .. } => (
            "conflicted",
            r#"{"reason":"stale_chapter_revision"}"#.to_owned(),
        ),
        DeleteChapterSettlementEffect::Refused { reason } => {
            let refused = match reason {
                storyos_core::DeleteChapterRefusal::ArchivedProject => "archived_project",
                storyos_core::DeleteChapterRefusal::InvalidChapterJoin => "invalid_chapter_join",
                storyos_core::DeleteChapterRefusal::MissingProject => {
                    return Err(DeleteChapterError::MissingProject);
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
                     $5::text::uuid, 'deleteChapter', $6, $7::text::uuid,
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
        .map_err(delete_chapter_database_error)?
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
        .map_err(delete_chapter_database_error)?;
    let mut project_activity_position = 0;
    let mut project_activity_event_id = String::new();
    if let DeleteChapterSettlementEffect::Applied {
        tree_revision,
        volume_id,
        current,
    } = &effect
    {
        persist_removed_chapter(client, command, tree_revision, volume_id, current).await?;
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
            .map_err(delete_chapter_database_error)?
            .get::<_, String>(0)
            .parse::<u64>()
            .map_err(delete_chapter_parse_error)?;
        project_activity_event_id = Uuid::now_v7().to_string();
        let resulting_current = client
            .query_one(
                "SELECT current_chapter_id::text FROM storyos.projects
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                ],
            )
            .await
            .map_err(delete_chapter_database_error)?
            .get::<_, Option<String>>(0);
        let payload = serde_json::json!({
            "kind": "chapter_deleted",
            "chapter_id": command.chapter_id.as_ref(),
            "volume_id": volume_id,
            "tree_revision": tree_revision.to_string(),
            "current_chapter_id": resulting_current,
        })
        .to_string();
        client
            .execute(
                "INSERT INTO storyos.project_activity_event_payloads
                   (owner_user_id, project_id, project_activity_position, project_activity_event_id,
                    event_kind, receipt_id, receipt_result_kind, payload)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                         'chapter_deleted', $5::text::uuid, 'authoritative_applied',
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
            .map_err(delete_chapter_database_error)?;
    }
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'deleteChapter' AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(delete_chapter_database_error)?;
    Ok(DeleteChapterSettlement {
        ids: command.ids.clone(),
        effect,
        receipt_created_at,
        project_activity_position,
        project_activity_event_id,
    })
}

async fn persist_removed_chapter(
    client: &tokio_postgres::Client,
    command: &DeleteChapterCommand,
    tree_revision: &u64,
    volume_id: &str,
    current: &DeleteChapterCurrent,
) -> Result<(), DeleteChapterError> {
    let decision_id = Uuid::now_v7().to_string();
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
                &command.chapter_id.as_ref(),
                &volume_id,
                &tree_revision.to_string(),
            ],
        )
        .await
        .map_err(delete_chapter_database_error)?;
    let updated = match current {
        DeleteChapterCurrent::PreserveExisting => client
            .execute(
                "UPDATE storyos.projects
                    SET tree_revision = $3::text::bigint
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND tree_revision = $4::text::bigint AND lifecycle_state = 'active'",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &tree_revision.to_string(),
                    &command.expected_chapter_revision.to_string(),
                ],
            )
            .await
            .map_err(delete_chapter_database_error)?,
        DeleteChapterCurrent::SelectSuccessor { chapter_id } => client
            .execute(
                "UPDATE storyos.projects
                    SET tree_revision = $3::text::bigint, current_chapter_id = $5::text::uuid
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND tree_revision = $4::text::bigint AND lifecycle_state = 'active'",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &tree_revision.to_string(),
                    &command.expected_chapter_revision.to_string(),
                    &chapter_id,
                ],
            )
            .await
            .map_err(delete_chapter_database_error)?,
        DeleteChapterCurrent::Empty => client
            .execute(
                "UPDATE storyos.projects
                    SET tree_revision = $3::text::bigint, current_chapter_id = NULL
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND tree_revision = $4::text::bigint AND lifecycle_state = 'active'",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &tree_revision.to_string(),
                    &command.expected_chapter_revision.to_string(),
                ],
            )
            .await
            .map_err(delete_chapter_database_error)?,
    };
    if updated != 1 {
        return Err(DeleteChapterError::Unavailable(Box::new(
            std::io::Error::other("tree revision changed under FOR UPDATE"),
        )));
    }
    Ok(())
}

async fn insert_delete_chapter_admission(
    client: &tokio_postgres::Client,
    command: &DeleteChapterCommand,
) -> Result<(), DeleteChapterError> {
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
                    'explicit_project_command', $9, $10, $11, 'deleteChapter',
                    $12, $13::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $14::text::uuid, NULL, NULL, '{}'::uuid[], '{}'::text[], NULL, $7,
                    NULL, NULL, NULL, convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.project_command_challenges AS challenge
              WHERE challenge.owner_user_id = $1::text::uuid
                AND challenge.project_id = $2::text::uuid
                AND challenge.command_kind = 'deleteChapter'
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
        .map_err(delete_chapter_database_error)?;
    if inserted != 1 {
        return Err(DeleteChapterError::InvalidChallenge);
    }
    Ok(())
}
