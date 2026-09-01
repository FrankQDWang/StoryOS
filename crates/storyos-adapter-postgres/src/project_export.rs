use storyos_application::{
    AuthorCommandAdmissionIds, CanonicalSnapshot, ExportOperationPage, ExportOperationReader,
    ExportProjectArchiveCommand, ExportProjectArchiveError, ExportProjectArchiveRefusal,
    ExportProjectArchiveSettlement, ExportProjectArchiveSettlementEffect,
    ExportProjectArchiveStore, GetExportOperation, PROJECT_EXPORT_ARCHIVE_PATH_PROFILE,
    PROJECT_EXPORT_ARCHIVE_PROFILE, ProjectCommandChallengeError, ProjectCommandChallengeUse,
    ProjectReadError, ProjectScope,
};
use storyos_core::{
    ExportProjectArchive as CoreExport, ExportProjectArchiveResult, ProjectLifecycle,
    ProjectPresence, export_project_archive as classify_export,
};
use uuid::Uuid;

use super::*;

impl ExportProjectArchiveStore for PostgresProjectReader {
    async fn export_project_archive(
        &self,
        command: &ExportProjectArchiveCommand,
    ) -> Result<ExportProjectArchiveSettlement, ExportProjectArchiveError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(export_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(export_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(export_challenge_error)?;
                read_export_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(export_challenge_error)?;
                Err(ExportProjectArchiveError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_export(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction.commit().await.map_err(export_challenge_error)?;
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

impl ExportOperationReader for PostgresProjectReader {
    async fn read_export_operation(
        &self,
        scope: &ProjectScope,
        export_id: &str,
    ) -> Result<GetExportOperation, ProjectReadError> {
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        let Some(project) = transaction
            .query_opt(
                "SELECT lifecycle_state FROM storyos.projects
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .map_err(read_error)?
        else {
            transaction.commit().await.map_err(read_error)?;
            return Ok(GetExportOperation::Missing);
        };
        if project.get::<_, String>(0) == "archived" {
            transaction.commit().await.map_err(read_error)?;
            return Ok(GetExportOperation::Archived);
        }
        let Some(row) = transaction
            .query_opt(
                "SELECT export.export_id::text,
                        export.archive_profile,
                        export.archive_path_profile,
                        export.immutable_root,
                        snapshot.snapshot_id::text,
                        snapshot.project_activity_position::text,
                        snapshot.replay_generation::text,
                        snapshot.redaction_profile,
                        snapshot.schema_profile,
                        to_char(snapshot.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        CASE WHEN snapshot.expires_at IS NULL THEN NULL
                             ELSE to_char(snapshot.expires_at AT TIME ZONE 'UTC',
                                          'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
                        END,
                        current_floor.floor_position::text,
                        snapshot.expires_at IS NOT NULL
                          AND snapshot.expires_at <= clock_timestamp()
                   FROM storyos.project_export_manifests AS export
                   JOIN storyos.project_snapshots AS snapshot
                     ON snapshot.owner_user_id = export.owner_user_id
                    AND snapshot.project_id = export.project_id
                    AND snapshot.snapshot_id::text = export.source_snapshot_id
                   JOIN LATERAL (
                     SELECT replay_generation FROM storyos.replay_generations
                      WHERE owner_user_id = snapshot.owner_user_id
                        AND project_id = snapshot.project_id
                      ORDER BY replay_generation DESC LIMIT 1
                   ) AS current_generation ON true
                   JOIN storyos.replay_floors AS current_floor
                     ON (current_floor.owner_user_id, current_floor.project_id,
                         current_floor.replay_generation) =
                        (snapshot.owner_user_id, snapshot.project_id,
                         current_generation.replay_generation)
                  WHERE export.owner_user_id = $1::text::uuid
                    AND export.project_id = $2::text::uuid
                    AND export.export_id = $3::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &export_id,
                ],
            )
            .await
            .map_err(read_error)?
        else {
            transaction.commit().await.map_err(read_error)?;
            return Ok(GetExportOperation::Missing);
        };
        if row.get::<_, bool>(12) {
            transaction.commit().await.map_err(read_error)?;
            return Ok(GetExportOperation::Expired);
        }
        transaction.commit().await.map_err(read_error)?;
        Ok(GetExportOperation::InProgress(Box::new(
            ExportOperationPage {
                project_scope: scope.clone(),
                export_id: row.get(0),
                archive_profile: row.get(1),
                archive_path_profile: row.get(2),
                immutable_root: row.get(3),
                source_snapshot: CanonicalSnapshot {
                    snapshot_id: row.get(4),
                    project_activity_position: row
                        .get::<_, String>(5)
                        .parse()
                        .map_err(ProjectReadError::unavailable)?,
                    replay_generation: row
                        .get::<_, String>(6)
                        .parse()
                        .map_err(ProjectReadError::unavailable)?,
                    redaction_profile: row.get(7),
                    schema_profile: row.get(8),
                    created_at: row.get(9),
                    expires_at: row.get(10),
                    floor_position: row
                        .get::<_, String>(11)
                        .parse()
                        .map_err(ProjectReadError::unavailable)?,
                },
            },
        )))
    }
}

async fn persist_export(
    client: &tokio_postgres::Client,
    command: &ExportProjectArchiveCommand,
) -> Result<ExportProjectArchiveSettlement, ExportProjectArchiveError> {
    let row = client
        .query_opt(
            "SELECT lifecycle_state FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              FOR UPDATE",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(export_database_error)?;
    let Some(row) = row else {
        return Err(ExportProjectArchiveError::MissingProject);
    };
    let current_lifecycle = match row.get::<_, String>(0).as_str() {
        "active" => ProjectLifecycle::Active,
        "archived" => ProjectLifecycle::Archived,
        other => {
            return Err(ExportProjectArchiveError::Unavailable(Box::new(
                std::io::Error::other(format!("unsupported Project lifecycle {other}")),
            )));
        }
    };
    let classified = classify_export(&CoreExport {
        presence: ProjectPresence::Present,
        current_lifecycle,
    });
    let effect = match classified {
        ExportProjectArchiveResult::Admitted => {
            let snapshot =
                crate::snapshot::load_latest_canonical_snapshot(client, &command.project_scope)
                    .await
                    .map_err(export_read_error)?
                    .ok_or_else(|| {
                        ExportProjectArchiveError::Unavailable(Box::new(std::io::Error::other(
                            "canonical snapshot is required for Project Export admission",
                        )))
                    })?;
            ExportProjectArchiveSettlementEffect::Admitted {
                archive_profile: PROJECT_EXPORT_ARCHIVE_PROFILE.to_owned(),
                archive_path_profile: PROJECT_EXPORT_ARCHIVE_PATH_PROFILE.to_owned(),
                source_snapshot: snapshot,
            }
        }
        ExportProjectArchiveResult::Refused {
            reason: storyos_core::ExportProjectArchiveRefusal::MissingProject,
        } => return Err(ExportProjectArchiveError::MissingProject),
        ExportProjectArchiveResult::Refused { reason } => {
            ExportProjectArchiveSettlementEffect::Refused {
                reason: match reason {
                    storyos_core::ExportProjectArchiveRefusal::ArchivedProject => {
                        ExportProjectArchiveRefusal::ArchivedProject
                    }
                    storyos_core::ExportProjectArchiveRefusal::MissingProject => {
                        ExportProjectArchiveRefusal::MissingProject
                    }
                },
            }
        }
    };
    insert_export_admission(client, command).await?;
    let (result_kind, result_payload) = match &effect {
        ExportProjectArchiveSettlementEffect::Admitted { .. } => {
            ("authoritative_applied", "{}".to_owned())
        }
        ExportProjectArchiveSettlementEffect::Refused { reason } => {
            let refused = match reason {
                ExportProjectArchiveRefusal::ArchivedProject => "archived_project",
                ExportProjectArchiveRefusal::MissingProject => {
                    return Err(ExportProjectArchiveError::MissingProject);
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
                     $5::text::uuid, 'exportProjectArchive', $6, $7::text::uuid,
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
        .map_err(export_database_error)?
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
        .map_err(export_database_error)?;
    let mut project_activity_position = 0;
    let mut project_activity_event_id = String::new();
    if let ExportProjectArchiveSettlementEffect::Admitted {
        archive_profile,
        archive_path_profile,
        source_snapshot,
    } = &effect
    {
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
            .map_err(export_database_error)?
            .get::<_, String>(0)
            .parse::<u64>()
            .map_err(export_parse_error)?;
        project_activity_event_id = Uuid::now_v7().to_string();
        let payload = serde_json::json!({
            "kind": "project_export_settled",
            "export_id": command.export_id,
            "archive_profile": archive_profile,
            "archive_path_profile": archive_path_profile,
        })
        .to_string();
        client
            .execute(
                "INSERT INTO storyos.project_activity_event_payloads
                   (owner_user_id, project_id, project_activity_position, project_activity_event_id,
                    event_kind, receipt_id, receipt_result_kind, payload)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                         'project_export_settled', $5::text::uuid,
                         'authoritative_applied', $6::text::jsonb)",
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
            .map_err(export_database_error)?;
        client
            .execute(
                "INSERT INTO storyos.project_export_manifests
                   (owner_user_id, project_id, export_id, receipt_id, source_snapshot_id,
                    source_activity_position, archive_profile, archive_path_profile)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5,
                         $6::text::bigint, $7, $8)",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &command.export_id,
                    &command.ids.receipt_id,
                    &source_snapshot.snapshot_id,
                    &source_snapshot.project_activity_position.to_string(),
                    archive_profile,
                    archive_path_profile,
                ],
            )
            .await
            .map_err(export_database_error)?;
    }
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'exportProjectArchive' AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(export_database_error)?;
    if let ExportProjectArchiveSettlementEffect::Admitted {
        source_snapshot, ..
    } = &effect
    {
        crate::project_archive_build::persist_export_archive(
            client,
            command,
            source_snapshot,
            &receipt_created_at,
        )
        .await?;
    }
    Ok(ExportProjectArchiveSettlement {
        ids: command.ids.clone(),
        export_id: command.export_id.clone(),
        effect,
        receipt_created_at,
        project_activity_position,
        project_activity_event_id,
    })
}

async fn insert_export_admission(
    client: &tokio_postgres::Client,
    command: &ExportProjectArchiveCommand,
) -> Result<(), ExportProjectArchiveError> {
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
                    'explicit_project_command', $9, $10, $11, 'exportProjectArchive',
                    $12, $13::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $14::text::uuid, NULL, NULL, '{}'::uuid[], '{}'::text[], NULL, $7,
                    NULL, NULL, NULL, convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.project_command_challenges AS challenge
              WHERE challenge.owner_user_id = $1::text::uuid
                AND challenge.project_id = $2::text::uuid
                AND challenge.command_kind = 'exportProjectArchive'
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
        .map_err(export_database_error)?;
    if inserted != 1 {
        return Err(ExportProjectArchiveError::InvalidChallenge);
    }
    Ok(())
}

async fn read_export_settlement(
    store: &PostgresProjectReader,
    command: &ExportProjectArchiveCommand,
    receipt_id: &str,
) -> Result<ExportProjectArchiveSettlement, ExportProjectArchiveError> {
    let client = store
        .connect_challenge()
        .await
        .map_err(export_challenge_error)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(export_database_error)?;
    let result = async {
        set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(export_challenge_error)?;
        let row = client
            .query_opt(
                "SELECT receipt.command_id::text,
                        receipt.author_command_admission_id::text,
                        receipt.receipt_id::text,
                        to_char(receipt.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        receipt.result_kind,
                        receipt.result_payload->>'reason',
                        payload.payload->>'export_id',
                        payload.project_activity_position::text,
                        payload.project_activity_event_id::text,
                        export.archive_profile,
                        export.archive_path_profile,
                        export.source_snapshot_id,
                        snapshot.project_activity_position::text,
                        snapshot.replay_generation::text,
                        snapshot.redaction_profile,
                        snapshot.schema_profile,
                        to_char(snapshot.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        CASE WHEN snapshot.expires_at IS NULL THEN NULL
                             ELSE to_char(snapshot.expires_at AT TIME ZONE 'UTC',
                                          'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
                        END,
                        current_floor.floor_position::text
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
              LEFT JOIN storyos.project_export_manifests AS export
                     ON (export.owner_user_id, export.project_id, export.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
              LEFT JOIN storyos.project_snapshots AS snapshot
                     ON snapshot.owner_user_id = export.owner_user_id
                    AND snapshot.project_id = export.project_id
                    AND snapshot.snapshot_id::text = export.source_snapshot_id
              LEFT JOIN LATERAL (
                     SELECT replay_generation FROM storyos.replay_generations
                      WHERE owner_user_id = snapshot.owner_user_id
                        AND project_id = snapshot.project_id
                      ORDER BY replay_generation DESC LIMIT 1
                   ) AS current_generation ON snapshot.snapshot_id IS NOT NULL
              LEFT JOIN storyos.replay_floors AS current_floor
                     ON (current_floor.owner_user_id, current_floor.project_id,
                         current_floor.replay_generation) =
                        (snapshot.owner_user_id, snapshot.project_id,
                         current_generation.replay_generation)
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $3::text::uuid
                    AND receipt.command_kind = 'exportProjectArchive'
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
            .map_err(export_database_error)?
            .ok_or(ExportProjectArchiveError::BindingConflict)?;
        let result_kind = row.get::<_, String>(4);
        let reason = row.get::<_, Option<String>>(5);
        let effect = match (result_kind.as_str(), reason.as_deref()) {
            ("authoritative_applied", None) => ExportProjectArchiveSettlementEffect::Admitted {
                archive_profile: row
                    .get::<_, Option<String>>(9)
                    .ok_or(ExportProjectArchiveError::BindingConflict)?,
                archive_path_profile: row
                    .get::<_, Option<String>>(10)
                    .ok_or(ExportProjectArchiveError::BindingConflict)?,
                source_snapshot: CanonicalSnapshot {
                    snapshot_id: row
                        .get::<_, Option<String>>(11)
                        .ok_or(ExportProjectArchiveError::BindingConflict)?,
                    project_activity_position: row
                        .get::<_, Option<String>>(12)
                        .ok_or(ExportProjectArchiveError::BindingConflict)?
                        .parse()
                        .map_err(export_parse_error)?,
                    replay_generation: row
                        .get::<_, Option<String>>(13)
                        .ok_or(ExportProjectArchiveError::BindingConflict)?
                        .parse()
                        .map_err(export_parse_error)?,
                    redaction_profile: row
                        .get::<_, Option<String>>(14)
                        .ok_or(ExportProjectArchiveError::BindingConflict)?,
                    schema_profile: row
                        .get::<_, Option<String>>(15)
                        .ok_or(ExportProjectArchiveError::BindingConflict)?,
                    created_at: row
                        .get::<_, Option<String>>(16)
                        .ok_or(ExportProjectArchiveError::BindingConflict)?,
                    expires_at: row.get(17),
                    floor_position: row
                        .get::<_, Option<String>>(18)
                        .ok_or(ExportProjectArchiveError::BindingConflict)?
                        .parse()
                        .map_err(export_parse_error)?,
                },
            },
            ("refused", Some("archived_project")) => {
                ExportProjectArchiveSettlementEffect::Refused {
                    reason: ExportProjectArchiveRefusal::ArchivedProject,
                }
            }
            _ => return Err(ExportProjectArchiveError::BindingConflict),
        };
        let export_id = row.get::<_, Option<String>>(6).unwrap_or_default();
        Ok(ExportProjectArchiveSettlement {
            ids: AuthorCommandAdmissionIds {
                command_id: row.get(0),
                author_command_admission_id: row.get(1),
                receipt_id: row.get(2),
            },
            export_id: if export_id.is_empty() {
                command.export_id.clone()
            } else {
                export_id
            },
            receipt_created_at: row.get(3),
            effect,
            project_activity_position: row
                .get::<_, Option<String>>(7)
                .unwrap_or_else(|| "0".to_owned())
                .parse::<u64>()
                .map_err(export_parse_error)?,
            project_activity_event_id: row.get::<_, Option<String>>(8).unwrap_or_default(),
        })
    }
    .await;
    match &result {
        Ok(_) => client
            .batch_execute("COMMIT")
            .await
            .map_err(export_database_error)?,
        Err(_) => {
            let _rollback = client.batch_execute("ROLLBACK").await;
        }
    }
    result
}

fn export_challenge_error(error: ProjectCommandChallengeError) -> ExportProjectArchiveError {
    match error {
        ProjectCommandChallengeError::BindingConflict => ExportProjectArchiveError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired => {
            ExportProjectArchiveError::InvalidChallenge
        }
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => {
            ExportProjectArchiveError::Unavailable(Box::new(error))
        }
    }
}

fn export_database_error(error: tokio_postgres::Error) -> ExportProjectArchiveError {
    ExportProjectArchiveError::Unavailable(Box::new(error))
}

fn export_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> ExportProjectArchiveError {
    ExportProjectArchiveError::Unavailable(Box::new(error))
}

fn export_read_error(error: ProjectReadError) -> ExportProjectArchiveError {
    ExportProjectArchiveError::Unavailable(Box::new(error))
}
