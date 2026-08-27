use storyos_application::{
    UpdateChapterCommand, UpdateChapterError, UpdateChapterSettlement,
    UpdateChapterSettlementEffect,
};
use storyos_core::{
    ChapterJoin, ProjectLifecycle, ProjectPresence, UpdateChapter as CoreUpdateChapter,
    UpdateChapterResult, update_chapter as classify_update_chapter,
};
use uuid::Uuid;

use super::{update_chapter_database_error, update_chapter_parse_error};

struct ChapterSiblings<'a> {
    ordered_ids: &'a [String],
    parent_volume_id: &'a str,
}

pub(super) async fn persist_update_chapter(
    client: &tokio_postgres::Client,
    command: &UpdateChapterCommand,
) -> Result<UpdateChapterSettlement, UpdateChapterError> {
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
        .map_err(update_chapter_database_error)?;
    let Some(row) = row else {
        return Err(UpdateChapterError::MissingProject);
    };
    let current_lifecycle = match row.get::<_, String>(0).as_str() {
        "active" => ProjectLifecycle::Active,
        "archived" => ProjectLifecycle::Archived,
        other => {
            return Err(UpdateChapterError::Unavailable(Box::new(
                std::io::Error::other(format!("unsupported Project lifecycle {other}")),
            )));
        }
    };
    let current_chapter_revision = row
        .get::<_, String>(1)
        .parse::<u64>()
        .map_err(update_chapter_parse_error)?;
    let chapters = client
        .query(
            "SELECT manuscript_object_id::text, title, tree_order::text, parent_volume_id::text
               FROM storyos.manuscript_objects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND object_kind = 'chapter'
              ORDER BY parent_volume_id, tree_order
              FOR UPDATE",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(update_chapter_database_error)?;
    let current = chapters
        .iter()
        .find(|chapter| chapter.get::<_, String>(0) == command.chapter_id.as_ref());
    let (chapter_join, current_title, current_order, ordered_ids, parent_volume_id) = match current
    {
        Some(chapter) => {
            let parent = chapter.get::<_, String>(3);
            (
                ChapterJoin::ExactScope,
                chapter.get::<_, String>(1),
                chapter
                    .get::<_, String>(2)
                    .parse::<u64>()
                    .map_err(update_chapter_parse_error)?,
                chapters
                    .iter()
                    .filter(|row| row.get::<_, String>(3) == parent)
                    .map(|row| row.get::<_, String>(0))
                    .collect::<Vec<_>>(),
                parent,
            )
        }
        None => (
            ChapterJoin::Invalid,
            String::new(),
            1,
            Vec::new(),
            String::new(),
        ),
    };
    let chapter_count = ordered_ids.len() as u64;
    let classified = classify_update_chapter(&CoreUpdateChapter {
        presence: ProjectPresence::Present,
        chapter_join,
        expected_chapter_revision: command.expected_chapter_revision,
        current_chapter_revision,
        current_lifecycle,
        title: command.title.clone(),
        current_title,
        order: command.order,
        current_order,
        chapter_count,
    });
    let effect = match classified {
        UpdateChapterResult::Applied {
            title,
            order,
            tree_revision,
        } => UpdateChapterSettlementEffect::Applied {
            title,
            order,
            tree_revision,
        },
        UpdateChapterResult::NoEffect { reason } => {
            UpdateChapterSettlementEffect::NoEffect { reason }
        }
        UpdateChapterResult::Conflicted { reason } => {
            UpdateChapterSettlementEffect::Conflicted { reason }
        }
        UpdateChapterResult::Refused {
            reason: storyos_core::UpdateChapterRefusal::MissingProject,
        } => return Err(UpdateChapterError::MissingProject),
        UpdateChapterResult::Refused { reason } => {
            UpdateChapterSettlementEffect::Refused { reason }
        }
    };
    insert_update_chapter_admission(client, command).await?;
    let (result_kind, result_payload) = match &effect {
        UpdateChapterSettlementEffect::Applied { .. } => ("authoritative_applied", "{}".to_owned()),
        UpdateChapterSettlementEffect::NoEffect { .. } => {
            ("no_effect", r#"{"reason":"unchanged"}"#.to_owned())
        }
        UpdateChapterSettlementEffect::Conflicted { .. } => (
            "conflicted",
            r#"{"reason":"stale_chapter_revision"}"#.to_owned(),
        ),
        UpdateChapterSettlementEffect::Refused { reason } => {
            let refused = match reason {
                storyos_core::UpdateChapterRefusal::ArchivedProject => "archived_project",
                storyos_core::UpdateChapterRefusal::InvalidTitle => "invalid_title",
                storyos_core::UpdateChapterRefusal::InvalidOrder => "invalid_order",
                storyos_core::UpdateChapterRefusal::InvalidChapterJoin => "invalid_chapter_join",
                storyos_core::UpdateChapterRefusal::MissingProject => {
                    return Err(UpdateChapterError::MissingProject);
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
                     $5::text::uuid, 'updateChapter', $6, $7::text::uuid,
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
        .map_err(update_chapter_database_error)?
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
        .map_err(update_chapter_database_error)?;
    let mut project_activity_position = 0;
    let mut project_activity_event_id = String::new();
    if let UpdateChapterSettlementEffect::Applied {
        title,
        order,
        tree_revision,
    } = &effect
    {
        apply_chapter_tree(
            client,
            command,
            ChapterSiblings {
                ordered_ids: &ordered_ids,
                parent_volume_id: &parent_volume_id,
            },
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
            .map_err(update_chapter_database_error)?
            .get::<_, String>(0)
            .parse::<u64>()
            .map_err(update_chapter_parse_error)?;
        project_activity_event_id = Uuid::now_v7().to_string();
        let payload = serde_json::json!({
            "kind": "chapter_updated",
            "chapter_id": command.chapter_id.as_ref(),
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
                         'chapter_updated', $5::text::uuid, 'authoritative_applied',
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
            .map_err(update_chapter_database_error)?;
    }
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'updateChapter' AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(update_chapter_database_error)?;
    Ok(UpdateChapterSettlement {
        ids: command.ids.clone(),
        effect,
        receipt_created_at,
        project_activity_position,
        project_activity_event_id,
    })
}

async fn apply_chapter_tree(
    client: &tokio_postgres::Client,
    command: &UpdateChapterCommand,
    siblings: ChapterSiblings<'_>,
    current_order: u64,
    title: &str,
    order: u64,
    tree_revision: u64,
) -> Result<(), UpdateChapterError> {
    let updated = client
        .execute(
            "UPDATE storyos.manuscript_objects
                SET title = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND manuscript_object_id = $4::text::uuid AND object_kind = 'chapter'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &title,
                &command.chapter_id.as_ref(),
            ],
        )
        .await
        .map_err(update_chapter_database_error)?;
    if updated != 1 {
        return Err(UpdateChapterError::Unavailable(Box::new(
            std::io::Error::other("Chapter row changed under FOR UPDATE"),
        )));
    }
    if current_order != order {
        let mut ids = siblings.ordered_ids.to_vec();
        let moved = ids.remove((current_order - 1) as usize);
        ids.insert((order - 1) as usize, moved);
        client
            .execute(
                "UPDATE storyos.manuscript_objects
                    SET tree_order = tree_order + 1000000
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND object_kind = 'chapter' AND parent_volume_id = $3::text::uuid",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &siblings.parent_volume_id,
                ],
            )
            .await
            .map_err(update_chapter_database_error)?;
        for (index, chapter_id) in ids.iter().enumerate() {
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
                        chapter_id,
                    ],
                )
                .await
                .map_err(update_chapter_database_error)?;
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
                &command.expected_chapter_revision.to_string(),
            ],
        )
        .await
        .map_err(update_chapter_database_error)?;
    if bumped != 1 {
        return Err(UpdateChapterError::Unavailable(Box::new(
            std::io::Error::other("tree revision changed under FOR UPDATE"),
        )));
    }
    Ok(())
}

async fn insert_update_chapter_admission(
    client: &tokio_postgres::Client,
    command: &UpdateChapterCommand,
) -> Result<(), UpdateChapterError> {
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
                    'explicit_project_command', $9, $10, $11, 'updateChapter',
                    $12, $13::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $14::text::uuid, NULL, NULL, '{}'::uuid[], '{}'::text[], NULL, $7,
                    NULL, NULL, NULL, convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.project_command_challenges AS challenge
              WHERE challenge.owner_user_id = $1::text::uuid
                AND challenge.project_id = $2::text::uuid
                AND challenge.command_kind = 'updateChapter'
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
        .map_err(update_chapter_database_error)?;
    if inserted != 1 {
        return Err(UpdateChapterError::InvalidChallenge);
    }
    Ok(())
}
