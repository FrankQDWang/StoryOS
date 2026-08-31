use storyos_application::{
    AuthorCommandAdmissionIds, ProjectCommandChallengeError, ProjectCommandChallengeUse,
    SetCurrentChapterCommand, SetCurrentChapterError, SetCurrentChapterSettlement,
    SetCurrentChapterSettlementEffect, SetCurrentChapterStore,
};
use storyos_core::{
    ChapterJoin, ProjectLifecycle, ProjectPresence, SetCurrentChapter as CoreSetCurrentChapter,
    SetCurrentChapterResult, set_current_chapter as classify_set_current_chapter,
};
use uuid::Uuid;

use super::*;

impl SetCurrentChapterStore for PostgresProjectReader {
    async fn set_current_chapter(
        &self,
        command: &SetCurrentChapterCommand,
    ) -> Result<SetCurrentChapterSettlement, SetCurrentChapterError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(set_current_chapter_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(set_current_chapter_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(set_current_chapter_challenge_error)?;
                read_set_current_chapter_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(set_current_chapter_challenge_error)?;
                Err(SetCurrentChapterError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_set_current_chapter(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction
                            .commit()
                            .await
                            .map_err(set_current_chapter_challenge_error)?;
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

async fn persist_set_current_chapter(
    client: &tokio_postgres::Client,
    command: &SetCurrentChapterCommand,
) -> Result<SetCurrentChapterSettlement, SetCurrentChapterError> {
    let row = client
        .query_opt(
            "SELECT project.lifecycle_state,
                    project.current_chapter_id::text,
                    chapter.manuscript_object_id::text,
                    head.current_revision_id::text,
                    expected.revision_id::text
               FROM storyos.projects AS project
          LEFT JOIN storyos.manuscript_objects AS chapter
                 ON chapter.owner_user_id = project.owner_user_id
                AND chapter.project_id = project.project_id
                AND chapter.object_kind = 'chapter'
                AND chapter.manuscript_object_id = $3::text::uuid
                AND NOT EXISTS (
                  SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                   WHERE removal.owner_user_id = chapter.owner_user_id
                     AND removal.project_id = chapter.project_id
                     AND removal.chapter_id = chapter.manuscript_object_id
                )
          LEFT JOIN storyos.authoritative_heads AS head
                 ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                    (chapter.owner_user_id, chapter.project_id, chapter.manuscript_object_id)
          LEFT JOIN storyos.authoritative_revisions AS expected
                 ON (expected.owner_user_id, expected.project_id,
                     expected.manuscript_object_id, expected.revision_id) =
                    (chapter.owner_user_id, chapter.project_id,
                     chapter.manuscript_object_id, $4::text::uuid)
              WHERE project.owner_user_id = $1::text::uuid
                AND project.project_id = $2::text::uuid
              FOR UPDATE OF project",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.chapter_id,
                &command.expected_target_revision_id,
            ],
        )
        .await
        .map_err(set_current_chapter_database_error)?;
    let Some(row) = row else {
        return Err(SetCurrentChapterError::MissingProject);
    };
    let current_lifecycle = match row.get::<_, String>(0).as_str() {
        "active" => ProjectLifecycle::Active,
        "archived" => ProjectLifecycle::Archived,
        other => {
            return Err(SetCurrentChapterError::Unavailable(Box::new(
                std::io::Error::other(format!("unsupported Project lifecycle {other}")),
            )));
        }
    };
    let current_chapter_id = row.get::<_, Option<String>>(1);
    let chapter_join = if row.get::<_, Option<String>>(2).is_some() {
        ChapterJoin::ExactScope
    } else {
        ChapterJoin::Invalid
    };
    let current_target_revision_id = row.get::<_, Option<String>>(3).unwrap_or_default();
    let expected_revision_exists = row.get::<_, Option<String>>(4).is_some();
    let classified = classify_set_current_chapter(&CoreSetCurrentChapter {
        presence: ProjectPresence::Present,
        chapter_join: chapter_join.clone(),
        current_lifecycle,
        current_chapter_id: current_chapter_id.clone(),
        expected_current_chapter_id: command.expected_current_chapter_id.clone(),
        target_chapter_id: command.chapter_id.clone(),
        expected_target_revision_id: command.expected_target_revision_id.clone(),
        current_target_revision_id: current_target_revision_id.clone(),
    });
    let effect = match classified {
        SetCurrentChapterResult::Applied { current_chapter_id } => {
            SetCurrentChapterSettlementEffect::Applied {
                current_chapter_id,
                base_snapshot_id: Uuid::now_v7().to_string(),
            }
        }
        SetCurrentChapterResult::NoEffect { reason } => {
            SetCurrentChapterSettlementEffect::NoEffect { reason }
        }
        SetCurrentChapterResult::Conflicted { reason } => {
            SetCurrentChapterSettlementEffect::Conflicted { reason }
        }
        SetCurrentChapterResult::Refused {
            reason: storyos_core::SetCurrentChapterRefusal::MissingProject,
        } => return Err(SetCurrentChapterError::MissingProject),
        SetCurrentChapterResult::Refused { reason } => {
            SetCurrentChapterSettlementEffect::Refused { reason }
        }
    };
    let admission_chapter =
        (chapter_join == ChapterJoin::ExactScope).then_some(command.chapter_id.as_str());
    let admission_expected = match (chapter_join, expected_revision_exists) {
        (ChapterJoin::Invalid, _) => None,
        (ChapterJoin::ExactScope, true) => Some(command.expected_target_revision_id.as_str()),
        (ChapterJoin::ExactScope, false) => Some(current_target_revision_id.as_str()),
    };
    insert_set_current_chapter_admission(client, command, admission_chapter, admission_expected)
        .await?;
    let (result_kind, result_payload) = match &effect {
        SetCurrentChapterSettlementEffect::Applied { .. } => {
            ("authoritative_applied", "{}".to_owned())
        }
        SetCurrentChapterSettlementEffect::NoEffect { .. } => {
            ("no_effect", r#"{"reason":"already_current"}"#.to_owned())
        }
        SetCurrentChapterSettlementEffect::Conflicted { reason } => {
            let conflicted = match reason {
                storyos_core::SetCurrentChapterConflict::StaleCurrentChapter => {
                    "stale_current_chapter"
                }
                storyos_core::SetCurrentChapterConflict::WrongTargetHead => "wrong_target_head",
            };
            ("conflicted", format!(r#"{{"reason":"{conflicted}"}}"#))
        }
        SetCurrentChapterSettlementEffect::Refused { reason } => {
            let refused = match reason {
                storyos_core::SetCurrentChapterRefusal::ArchivedProject => "archived_project",
                storyos_core::SetCurrentChapterRefusal::InvalidChapterJoin => {
                    "invalid_chapter_join"
                }
                storyos_core::SetCurrentChapterRefusal::EmptyProject => "empty_project",
                storyos_core::SetCurrentChapterRefusal::MissingProject => {
                    return Err(SetCurrentChapterError::MissingProject);
                }
            };
            ("refused", format!(r#"{{"reason":"{refused}"}}"#))
        }
    };
    let resulting_head = if current_target_revision_id.is_empty() {
        command.expected_target_revision_id.as_str()
    } else {
        current_target_revision_id.as_str()
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
                     $5::text::uuid, 'setCurrentChapter', $6, $7::text::uuid,
                     'author_command_admission', ARRAY[$8::text::uuid], ARRAY[$9::text::uuid],
                     ARRAY[$9::text::uuid], '{}'::uuid[], '{}'::uuid[], '{}'::uuid[],
                     '{}'::text[], '{}'::text[], '{}'::text[], $10, $11::text::jsonb)
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
                &command.expected_target_revision_id,
                &resulting_head,
                &result_kind,
                &result_payload,
            ],
        )
        .await
        .map_err(set_current_chapter_database_error)?
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
        .map_err(set_current_chapter_database_error)?;
    let mut project_activity_position = 0;
    let mut project_activity_event_id = String::new();
    if let SetCurrentChapterSettlementEffect::Applied {
        current_chapter_id,
        base_snapshot_id,
    } = &effect
    {
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
                    &current_chapter_id,
                    &command.expected_current_chapter_id,
                ],
            )
            .await
            .map_err(set_current_chapter_database_error)?;
        if updated != 1 {
            return Err(SetCurrentChapterError::Unavailable(Box::new(
                std::io::Error::other("current Chapter changed under FOR UPDATE"),
            )));
        }
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
            .map_err(set_current_chapter_database_error)?
            .get::<_, String>(0)
            .parse::<u64>()
            .map_err(set_current_chapter_parse_error)?;
        project_activity_event_id = Uuid::now_v7().to_string();
        crate::snapshot::persist_canonical_snapshot(
            client,
            &command.project_scope,
            base_snapshot_id,
            project_activity_position,
        )
        .await
        .map_err(set_current_chapter_database_error)?;
        let activity_position_text = project_activity_position.to_string();
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
                    &base_snapshot_id,
                    &current_chapter_id,
                    &resulting_head,
                    &activity_position_text,
                ],
            )
            .await
            .map_err(set_current_chapter_database_error)?;
        if base_updates != 1 {
            return Err(SetCurrentChapterError::BindingConflict);
        }
        let payload = serde_json::json!({
            "kind": "current_chapter_set",
            "prior_chapter_id": command.expected_current_chapter_id,
            "current_chapter_id": current_chapter_id,
            "base_snapshot_id": base_snapshot_id,
        })
        .to_string();
        client
            .execute(
                "INSERT INTO storyos.project_activity_event_payloads
                   (owner_user_id, project_id, project_activity_position, project_activity_event_id,
                    event_kind, receipt_id, receipt_result_kind, payload)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                         'current_chapter_set', $5::text::uuid, 'authoritative_applied',
                         $6::text::jsonb)",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &activity_position_text,
                    &project_activity_event_id,
                    &command.ids.receipt_id,
                    &payload,
                ],
            )
            .await
            .map_err(set_current_chapter_database_error)?;
    }
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'setCurrentChapter' AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(set_current_chapter_database_error)?;
    Ok(SetCurrentChapterSettlement {
        ids: command.ids.clone(),
        effect,
        receipt_created_at,
        project_activity_position,
        project_activity_event_id,
    })
}

async fn insert_set_current_chapter_admission(
    client: &tokio_postgres::Client,
    command: &SetCurrentChapterCommand,
    chapter_object_id: Option<&str>,
    expected_authoritative_revision_id: Option<&str>,
) -> Result<(), SetCurrentChapterError> {
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
                    session.editor_session_id, writer.writer_generation,
                    session.client_session_binding_ref, $6::text::numeric,
                    session.client_contract_revision, session.security_policy_revision,
                    'explicit_editor_command', $7, $8, $9, 'setCurrentChapter',
                    $10, $11::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $12::text::uuid, $13::text::uuid, $14::text::uuid, '{}'::uuid[], '{}'::text[],
                    NULL, session.client_contract_revision, NULL, NULL, NULL,
                    convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.editor_sessions AS session
               JOIN storyos.project_writer_generations AS writer
                 ON (writer.owner_user_id, writer.project_id,
                     writer.current_editor_session_id) =
                    (session.owner_user_id, session.project_id, session.editor_session_id)
                AND writer.writer_generation = (
                  SELECT max(current_writer.writer_generation)
                    FROM storyos.project_writer_generations AS current_writer
                   WHERE current_writer.owner_user_id = session.owner_user_id
                     AND current_writer.project_id = session.project_id
                )
               JOIN storyos.project_command_challenges AS challenge
                 ON (challenge.owner_user_id, challenge.project_id,
                     challenge.command_kind, challenge.idempotency_key) =
                    (session.owner_user_id, session.project_id,
                     'setCurrentChapter', $11::text::uuid)
              WHERE session.owner_user_id = $1::text::uuid
                AND session.project_id = $2::text::uuid
                AND session.editor_session_id = $5::text::uuid
                AND session.client_session_binding_ref = $16
                AND session.client_session_generation = $6::text::numeric
                AND session.client_contract_revision = $17
                AND session.security_policy_revision = $18
                AND challenge.consumed_at IS NOT NULL",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.author_command_admission_id,
                &command.ids.command_id,
                &command.editor_session_id.as_ref(),
                &client_session_generation,
                &command.challenge_binding.method,
                &command.challenge_binding.route_template,
                &command.challenge_binding.command_schema,
                &command.challenge_binding.canonical_command_digest,
                &command.challenge_binding.idempotency_key,
                &command.correlation_id,
                &chapter_object_id,
                &expected_authoritative_revision_id,
                &command.canonical_command_bytes.as_slice(),
                &command.client_binding.binding_ref,
                &command.client_binding.client_contract_revision,
                &command.client_binding.security_policy_revision,
            ],
        )
        .await
        .map_err(set_current_chapter_database_error)?;
    if inserted != 1 {
        return Err(SetCurrentChapterError::InvalidChallenge);
    }
    Ok(())
}

async fn read_set_current_chapter_settlement(
    store: &PostgresProjectReader,
    command: &SetCurrentChapterCommand,
    receipt_id: &str,
) -> Result<SetCurrentChapterSettlement, SetCurrentChapterError> {
    let client = store
        .connect_challenge()
        .await
        .map_err(set_current_chapter_challenge_error)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(set_current_chapter_database_error)?;
    let result = async {
        set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(set_current_chapter_challenge_error)?;
        let row = client
            .query_opt(
                "SELECT receipt.command_id::text,
                        receipt.author_command_admission_id::text,
                        receipt.receipt_id::text,
                        to_char(receipt.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        receipt.result_kind,
                        receipt.result_payload->>'reason',
                        payload.payload->>'current_chapter_id',
                        payload.payload->>'base_snapshot_id',
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
                    AND receipt.command_kind = 'setCurrentChapter'
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
            .map_err(set_current_chapter_database_error)?
            .ok_or(SetCurrentChapterError::BindingConflict)?;
        let result_kind = row.get::<_, String>(4);
        let reason = row.get::<_, Option<String>>(5);
        let effect = match (result_kind.as_str(), reason.as_deref()) {
            ("authoritative_applied", None) => SetCurrentChapterSettlementEffect::Applied {
                current_chapter_id: row
                    .get::<_, Option<String>>(6)
                    .ok_or(SetCurrentChapterError::BindingConflict)?,
                base_snapshot_id: row
                    .get::<_, Option<String>>(7)
                    .ok_or(SetCurrentChapterError::BindingConflict)?,
            },
            ("no_effect", Some("already_current")) => SetCurrentChapterSettlementEffect::NoEffect {
                reason: storyos_core::SetCurrentChapterNoEffect::AlreadyCurrent,
            },
            ("conflicted", Some("stale_current_chapter")) => {
                SetCurrentChapterSettlementEffect::Conflicted {
                    reason: storyos_core::SetCurrentChapterConflict::StaleCurrentChapter,
                }
            }
            ("conflicted", Some("wrong_target_head")) => {
                SetCurrentChapterSettlementEffect::Conflicted {
                    reason: storyos_core::SetCurrentChapterConflict::WrongTargetHead,
                }
            }
            ("refused", Some("archived_project")) => SetCurrentChapterSettlementEffect::Refused {
                reason: storyos_core::SetCurrentChapterRefusal::ArchivedProject,
            },
            ("refused", Some("invalid_chapter_join")) => {
                SetCurrentChapterSettlementEffect::Refused {
                    reason: storyos_core::SetCurrentChapterRefusal::InvalidChapterJoin,
                }
            }
            ("refused", Some("empty_project")) => SetCurrentChapterSettlementEffect::Refused {
                reason: storyos_core::SetCurrentChapterRefusal::EmptyProject,
            },
            _ => return Err(SetCurrentChapterError::BindingConflict),
        };
        Ok(SetCurrentChapterSettlement {
            ids: AuthorCommandAdmissionIds {
                command_id: row.get(0),
                author_command_admission_id: row.get(1),
                receipt_id: row.get(2),
            },
            receipt_created_at: row.get(3),
            effect,
            project_activity_position: row
                .get::<_, Option<String>>(8)
                .unwrap_or_else(|| "0".to_owned())
                .parse::<u64>()
                .map_err(set_current_chapter_parse_error)?,
            project_activity_event_id: row.get::<_, Option<String>>(9).unwrap_or_default(),
        })
    }
    .await;
    match &result {
        Ok(_) => client
            .batch_execute("COMMIT")
            .await
            .map_err(set_current_chapter_database_error)?,
        Err(_) => {
            let _rollback = client.batch_execute("ROLLBACK").await;
        }
    }
    result
}

fn set_current_chapter_challenge_error(
    error: ProjectCommandChallengeError,
) -> SetCurrentChapterError {
    match error {
        ProjectCommandChallengeError::BindingConflict => SetCurrentChapterError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired => SetCurrentChapterError::InvalidChallenge,
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => {
            SetCurrentChapterError::Unavailable(Box::new(error))
        }
    }
}

fn set_current_chapter_database_error(error: tokio_postgres::Error) -> SetCurrentChapterError {
    SetCurrentChapterError::Unavailable(Box::new(error))
}

fn set_current_chapter_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> SetCurrentChapterError {
    SetCurrentChapterError::Unavailable(Box::new(error))
}
