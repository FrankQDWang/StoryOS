use storyos_application::{
    AuthorCommandAdmissionIds, CreateChapterCommand, CreateChapterError, CreateChapterSettlement,
    CreateChapterSettlementEffect, CreateChapterStore, ProjectCommandChallengeError,
    ProjectCommandChallengeUse,
};
use storyos_core::{
    CreateChapter as CoreCreateChapter, CreateChapterCurrent, CreateChapterOpen,
    CreateChapterResult, ProjectLifecycle, ProjectPresence, VolumeJoin,
    create_chapter as classify_create_chapter,
};
use uuid::Uuid;

use super::*;

mod persist;
use persist::{insert_create_chapter_admission, next_chapter_order, persist_created_chapter};

impl CreateChapterStore for PostgresProjectReader {
    async fn create_chapter(
        &self,
        command: &CreateChapterCommand,
    ) -> Result<CreateChapterSettlement, CreateChapterError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(create_chapter_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(create_chapter_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(create_chapter_challenge_error)?;
                read_create_chapter_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(create_chapter_challenge_error)?;
                Err(CreateChapterError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_create_chapter(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction
                            .commit()
                            .await
                            .map_err(create_chapter_challenge_error)?;
                        Ok(settlement)
                    }
                    Err(error) => {
                        let _rollback = transaction.rollback().await;
                        Err(error)
                    }
                }
            }
        }
    }
}

async fn persist_create_chapter(
    client: &tokio_postgres::Client,
    command: &CreateChapterCommand,
) -> Result<CreateChapterSettlement, CreateChapterError> {
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
        .map_err(create_chapter_database_error)?;
    let Some(row) = row else {
        return Err(CreateChapterError::MissingProject);
    };
    let current_lifecycle = match row.get::<_, String>(0).as_str() {
        "active" => ProjectLifecycle::Active,
        "archived" => ProjectLifecycle::Archived,
        other => {
            return Err(CreateChapterError::Unavailable(Box::new(
                std::io::Error::other(format!("unsupported Project lifecycle {other}")),
            )));
        }
    };
    let current_tree_revision = row
        .get::<_, String>(1)
        .parse::<u64>()
        .map_err(create_chapter_parse_error)?;
    let current_chapter_id = row.get::<_, Option<String>>(2);
    let volume_join = if client
        .query_opt(
            "SELECT manuscript_object_id
               FROM storyos.manuscript_objects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND manuscript_object_id = $3::text::uuid AND object_kind = 'volume'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.volume_id,
            ],
        )
        .await
        .map_err(create_chapter_database_error)?
        .is_some()
    {
        VolumeJoin::ExactScope
    } else {
        VolumeJoin::Invalid
    };
    let classified = classify_create_chapter(&CoreCreateChapter {
        presence: ProjectPresence::Present,
        volume_join,
        expected_tree_revision: command.expected_tree_revision,
        current_tree_revision,
        current_lifecycle,
        current_open: match current_chapter_id {
            None => CreateChapterOpen::Empty,
            Some(_) => CreateChapterOpen::CurrentChapter,
        },
        title: command.title.clone(),
    });
    let effect = match classified {
        CreateChapterResult::Applied {
            tree_revision,
            current,
        } => CreateChapterSettlementEffect::Applied {
            tree_revision,
            chapter_id: Uuid::now_v7().to_string(),
            current,
            order: next_chapter_order(client, command).await?,
        },
        CreateChapterResult::Conflicted { reason } => {
            CreateChapterSettlementEffect::Conflicted { reason }
        }
        CreateChapterResult::Refused {
            reason: storyos_core::CreateChapterRefusal::MissingProject,
        } => return Err(CreateChapterError::MissingProject),
        CreateChapterResult::Refused { reason } => {
            CreateChapterSettlementEffect::Refused { reason }
        }
    };
    insert_create_chapter_admission(client, command).await?;
    let (result_kind, result_payload) = match &effect {
        CreateChapterSettlementEffect::Applied { .. } => ("authoritative_applied", "{}".to_owned()),
        CreateChapterSettlementEffect::Conflicted { .. } => (
            "conflicted",
            r#"{"reason":"stale_tree_revision"}"#.to_owned(),
        ),
        CreateChapterSettlementEffect::Refused { reason } => {
            let refused = match reason {
                storyos_core::CreateChapterRefusal::ArchivedProject => "archived_project",
                storyos_core::CreateChapterRefusal::InvalidTitle => "invalid_title",
                storyos_core::CreateChapterRefusal::InvalidVolumeJoin => "invalid_volume_join",
                storyos_core::CreateChapterRefusal::MissingProject => {
                    return Err(CreateChapterError::MissingProject);
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
                     $5::text::uuid, 'createChapter', $6, $7::text::uuid,
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
        .map_err(create_chapter_database_error)?
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
        .map_err(create_chapter_database_error)?;
    let mut project_activity_position = 0;
    let mut project_activity_event_id = String::new();
    if let CreateChapterSettlementEffect::Applied {
        tree_revision,
        chapter_id,
        current,
        order,
    } = &effect
    {
        persist_created_chapter(client, command, tree_revision, chapter_id, current, *order)
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
            .map_err(create_chapter_database_error)?
            .get::<_, String>(0)
            .parse::<u64>()
            .map_err(create_chapter_parse_error)?;
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
            .map_err(create_chapter_database_error)?
            .get::<_, String>(0);
        let payload = serde_json::json!({
            "kind": "chapter_created",
            "volume_id": command.volume_id,
            "chapter_id": chapter_id,
            "title": command.title,
            "tree_revision": tree_revision.to_string(),
            "order": order.to_string(),
            "current_chapter_id": resulting_current,
        })
        .to_string();
        client
            .execute(
                "INSERT INTO storyos.project_activity_event_payloads
                   (owner_user_id, project_id, project_activity_position, project_activity_event_id,
                    event_kind, receipt_id, receipt_result_kind, payload)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                         'chapter_created', $5::text::uuid, 'authoritative_applied',
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
            .map_err(create_chapter_database_error)?;
    }
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'createChapter' AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(create_chapter_database_error)?;
    Ok(CreateChapterSettlement {
        ids: command.ids.clone(),
        effect,
        receipt_created_at,
        project_activity_position,
        project_activity_event_id,
    })
}

async fn read_create_chapter_settlement(
    store: &PostgresProjectReader,
    command: &CreateChapterCommand,
    receipt_id: &str,
) -> Result<CreateChapterSettlement, CreateChapterError> {
    let client = store
        .connect_challenge()
        .await
        .map_err(create_chapter_challenge_error)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(create_chapter_database_error)?;
    let result = async {
        set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(create_chapter_challenge_error)?;
        let row = client
            .query_opt(
                "SELECT receipt.command_id::text,
                        receipt.author_command_admission_id::text,
                        receipt.receipt_id::text,
                        to_char(receipt.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        receipt.result_kind,
                        receipt.result_payload->>'reason',
                        payload.payload->>'tree_revision',
                        payload.payload->>'chapter_id',
                        payload.payload->>'current_chapter_id',
                        payload.payload->>'order',
                        payload.project_activity_position::text,
                        payload.project_activity_event_id::text
                   FROM storyos.domain_receipts AS receipt
                   JOIN storyos.author_command_admission_settlements AS settlement
                     ON (settlement.owner_user_id, settlement.project_id,
                         settlement.author_command_admission_id, settlement.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id,
                         receipt.author_command_admission_id, receipt.receipt_id)
                   JOIN storyos.command_idempotency AS idempotency
                     ON (idempotency.owner_user_id, idempotency.project_id,
                         idempotency.command_kind, idempotency.idempotency_key,
                         idempotency.result_reference) =
                        (receipt.owner_user_id, receipt.project_id, receipt.command_kind,
                         receipt.idempotency_key, receipt.receipt_id::text)
              LEFT JOIN storyos.project_activity_event_payloads AS payload
                     ON (payload.owner_user_id, payload.project_id, payload.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $3::text::uuid
                    AND receipt.command_kind = 'createChapter'
                    AND receipt.command_digest = $4
                    AND receipt.idempotency_key = $5::text::uuid
                    AND settlement.settlement_kind = 'receipt_settled'
                    AND idempotency.outcome_kind = 'settled'",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &receipt_id,
                    &command.challenge_binding.canonical_command_digest,
                    &command.challenge_binding.idempotency_key,
                ],
            )
            .await
            .map_err(create_chapter_database_error)?
            .ok_or(CreateChapterError::BindingConflict)?;
        let result_kind = row.get::<_, String>(4);
        let reason = row.get::<_, Option<String>>(5);
        let effect = match (result_kind.as_str(), reason.as_deref()) {
            ("authoritative_applied", None) => {
                let tree_revision = row
                    .get::<_, Option<String>>(6)
                    .ok_or(CreateChapterError::BindingConflict)?
                    .parse::<u64>()
                    .map_err(create_chapter_parse_error)?;
                let chapter_id = row
                    .get::<_, Option<String>>(7)
                    .ok_or(CreateChapterError::BindingConflict)?;
                let resulting_current = row
                    .get::<_, Option<String>>(8)
                    .ok_or(CreateChapterError::BindingConflict)?;
                let order = row
                    .get::<_, Option<String>>(9)
                    .ok_or(CreateChapterError::BindingConflict)?
                    .parse::<u64>()
                    .map_err(create_chapter_parse_error)?;
                CreateChapterSettlementEffect::Applied {
                    tree_revision,
                    current: if resulting_current == chapter_id {
                        CreateChapterCurrent::SelectCreated
                    } else {
                        CreateChapterCurrent::PreserveExisting
                    },
                    chapter_id,
                    order,
                }
            }
            ("conflicted", Some("stale_tree_revision")) => {
                CreateChapterSettlementEffect::Conflicted {
                    reason: storyos_core::CreateChapterConflict::StaleTreeRevision,
                }
            }
            ("refused", Some("archived_project")) => CreateChapterSettlementEffect::Refused {
                reason: storyos_core::CreateChapterRefusal::ArchivedProject,
            },
            ("refused", Some("invalid_title")) => CreateChapterSettlementEffect::Refused {
                reason: storyos_core::CreateChapterRefusal::InvalidTitle,
            },
            ("refused", Some("invalid_volume_join")) => CreateChapterSettlementEffect::Refused {
                reason: storyos_core::CreateChapterRefusal::InvalidVolumeJoin,
            },
            _ => return Err(CreateChapterError::BindingConflict),
        };
        Ok(CreateChapterSettlement {
            ids: AuthorCommandAdmissionIds {
                command_id: row.get(0),
                author_command_admission_id: row.get(1),
                receipt_id: row.get(2),
            },
            receipt_created_at: row.get(3),
            effect,
            project_activity_position: row
                .get::<_, Option<String>>(10)
                .unwrap_or_else(|| "0".to_owned())
                .parse::<u64>()
                .map_err(create_chapter_parse_error)?,
            project_activity_event_id: row.get::<_, Option<String>>(11).unwrap_or_default(),
        })
    }
    .await;
    match &result {
        Ok(_) => client
            .batch_execute("COMMIT")
            .await
            .map_err(create_chapter_database_error)?,
        Err(_) => {
            let _rollback = client.batch_execute("ROLLBACK").await;
        }
    }
    result
}

pub(super) fn create_chapter_challenge_error(
    error: ProjectCommandChallengeError,
) -> CreateChapterError {
    match error {
        ProjectCommandChallengeError::BindingConflict => CreateChapterError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired => CreateChapterError::InvalidChallenge,
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => {
            CreateChapterError::Unavailable(Box::new(error))
        }
    }
}

pub(super) fn create_chapter_database_error(error: tokio_postgres::Error) -> CreateChapterError {
    CreateChapterError::Unavailable(Box::new(error))
}

pub(super) fn create_chapter_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> CreateChapterError {
    CreateChapterError::Unavailable(Box::new(error))
}
