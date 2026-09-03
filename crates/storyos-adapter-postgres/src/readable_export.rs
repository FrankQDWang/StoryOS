use storyos_application::{
    AuthorCommandAdmissionIds, CanonicalSnapshot, ExportHumanReadableManuscriptCommand,
    ExportHumanReadableManuscriptError, ExportHumanReadableManuscriptSettlement,
    ExportHumanReadableManuscriptSettlementEffect, ExportHumanReadableManuscriptStore,
    GetHumanReadableManuscriptExport, HumanReadableManuscriptExportPage,
    HumanReadableManuscriptExportProgress, HumanReadableManuscriptExportReader,
    ProjectCommandChallengeError, ProjectCommandChallengeUse, ProjectReadError, ProjectScope,
};
use storyos_core::{
    ExportHumanReadableManuscript as CoreExport, ExportHumanReadableManuscriptResult,
    ProjectLifecycle, ProjectPresence, READABLE_EXPORT_PROFILE,
    export_human_readable_manuscript as classify_export,
};

use super::*;

impl ExportHumanReadableManuscriptStore for PostgresProjectReader {
    async fn export_human_readable_manuscript(
        &self,
        command: &ExportHumanReadableManuscriptCommand,
    ) -> Result<ExportHumanReadableManuscriptSettlement, ExportHumanReadableManuscriptError> {
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
                read_settled_admission(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(export_challenge_error)?;
                read_admitted_operation(self, command).await
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

impl HumanReadableManuscriptExportReader for PostgresProjectReader {
    async fn read_human_readable_manuscript_export(
        &self,
        scope: &ProjectScope,
        export_id: &str,
    ) -> Result<GetHumanReadableManuscriptExport, ProjectReadError> {
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
            return Ok(GetHumanReadableManuscriptExport::Missing);
        };
        let archived = project.get::<_, String>(0) == "archived";
        let ready = transaction
            .query_opt(
                "SELECT export.export_id::text,
                        export.export_profile,
                        export.content_sha256,
                        export.manuscript_utf8,
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
                   FROM storyos.human_readable_manuscript_exports AS export
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
            .map_err(read_error)?;
        if let Some(row) = ready {
            if row.get::<_, bool>(12) {
                transaction.commit().await.map_err(read_error)?;
                return Ok(GetHumanReadableManuscriptExport::Expired);
            }
            transaction.commit().await.map_err(read_error)?;
            return Ok(GetHumanReadableManuscriptExport::Ready(Box::new(
                HumanReadableManuscriptExportPage {
                    project_scope: scope.clone(),
                    export_id: row.get(0),
                    export_profile: row.get(1),
                    content_sha256: row.get(2),
                    manuscript_utf8: row.get(3),
                    source_snapshot: snapshot_from_joined_row(&row, /*start*/ 4)?,
                },
            )));
        }
        let Some(operation) = transaction
            .query_opt(
                "SELECT operation.export_id::text,
                        operation.export_profile,
                        operation.source_snapshot_id::text,
                        operation.claim_generation,
                        operation.settled_result,
                        operation.lease_expires_at IS NOT NULL
                          AND operation.lease_expires_at <= clock_timestamp()
                   FROM storyos.human_readable_manuscript_export_operations AS operation
                  WHERE operation.owner_user_id = $1::text::uuid
                    AND operation.project_id = $2::text::uuid
                    AND operation.export_id = $3::text::uuid",
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
            return Ok(if archived {
                GetHumanReadableManuscriptExport::Archived
            } else {
                GetHumanReadableManuscriptExport::Missing
            });
        };
        let snapshot_id = operation.get::<_, String>(2);
        let source_snapshot =
            crate::snapshot::load_canonical_snapshot_by_id(&transaction, scope, &snapshot_id)
                .await?
                .ok_or_else(|| {
                    ProjectReadError::unavailable(std::io::Error::other(
                        "pinned Snapshot is required for an admitted human-readable export",
                    ))
                })?;
        let progress = HumanReadableManuscriptExportProgress {
            project_scope: scope.clone(),
            export_id: operation.get(0),
            export_profile: operation.get(1),
            source_snapshot,
        };
        let settled_result = operation.get::<_, Option<String>>(4);
        let claim_generation: i64 = operation.get(3);
        let lease_expired: bool = operation.get(5);
        let status = match (settled_result.as_deref(), claim_generation, lease_expired) {
            (Some("failed"), _, _) => GetHumanReadableManuscriptExport::Failed(Box::new(progress)),
            (None, generation, true) if generation > 0 => {
                GetHumanReadableManuscriptExport::OutcomeUnknown(Box::new(progress))
            }
            (None, _, _) => GetHumanReadableManuscriptExport::InProgress(Box::new(progress)),
            (Some(_), _, _) => {
                return Err(ProjectReadError::unavailable(std::io::Error::other(
                    "unsupported human-readable export settled_result",
                )));
            }
        };
        transaction.commit().await.map_err(read_error)?;
        Ok(status)
    }
}

async fn persist_export(
    client: &tokio_postgres::Client,
    command: &ExportHumanReadableManuscriptCommand,
) -> Result<ExportHumanReadableManuscriptSettlement, ExportHumanReadableManuscriptError> {
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
        return Err(ExportHumanReadableManuscriptError::MissingProject);
    };
    let current_lifecycle = match row.get::<_, String>(0).as_str() {
        "active" => ProjectLifecycle::Active,
        "archived" => ProjectLifecycle::Archived,
        other => {
            return Err(ExportHumanReadableManuscriptError::Unavailable(Box::new(
                std::io::Error::other(format!("unsupported Project lifecycle {other}")),
            )));
        }
    };
    match classify_export(&CoreExport {
        presence: ProjectPresence::Present,
        current_lifecycle,
    }) {
        ExportHumanReadableManuscriptResult::Applied => {}
        ExportHumanReadableManuscriptResult::Refused {
            reason: storyos_core::ExportHumanReadableManuscriptRefusal::MissingProject,
        } => return Err(ExportHumanReadableManuscriptError::MissingProject),
        ExportHumanReadableManuscriptResult::Refused {
            reason: storyos_core::ExportHumanReadableManuscriptRefusal::ArchivedProject,
        } => return Err(ExportHumanReadableManuscriptError::ArchivedProject),
    }
    let snapshot = crate::snapshot::load_latest_canonical_snapshot(client, &command.project_scope)
        .await
        .map_err(export_read_error)?
        .ok_or_else(|| {
            ExportHumanReadableManuscriptError::Unavailable(Box::new(std::io::Error::other(
                "canonical snapshot is required for human-readable export",
            )))
        })?;
    insert_export_admission(client, command).await?;
    insert_export_operation(client, command, &snapshot).await?;
    Ok(ExportHumanReadableManuscriptSettlement {
        ids: command.ids.clone(),
        export_id: command.export_id.clone(),
        effect: ExportHumanReadableManuscriptSettlementEffect::Admitted {
            source_snapshot: Box::new(snapshot),
        },
    })
}

async fn insert_export_admission(
    client: &tokio_postgres::Client,
    command: &ExportHumanReadableManuscriptCommand,
) -> Result<(), ExportHumanReadableManuscriptError> {
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
                    'explicit_project_command', $9, $10, $11, 'exportHumanReadableManuscript',
                    $12, $13::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $14::text::uuid, NULL, NULL, '{}'::uuid[], '{}'::text[], NULL, $7,
                    NULL, NULL, NULL, convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.project_command_challenges AS challenge
              WHERE challenge.owner_user_id = $1::text::uuid
                AND challenge.project_id = $2::text::uuid
                AND challenge.command_kind = 'exportHumanReadableManuscript'
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
        return Err(ExportHumanReadableManuscriptError::InvalidChallenge);
    }
    Ok(())
}

async fn insert_export_operation(
    client: &tokio_postgres::Client,
    command: &ExportHumanReadableManuscriptCommand,
    snapshot: &CanonicalSnapshot,
) -> Result<(), ExportHumanReadableManuscriptError> {
    let source_activity_position = snapshot.project_activity_position.to_string();
    client
        .execute(
            "INSERT INTO storyos.human_readable_manuscript_export_operations
               (owner_user_id, project_id, export_id, author_command_admission_id, command_id,
                command_kind, idempotency_key, source_snapshot_id, source_activity_position,
                export_profile)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5::text::uuid,
                     'exportHumanReadableManuscript', $6::text::uuid, $7::text::uuid,
                     $8::text::bigint, $9)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.export_id,
                &command.ids.author_command_admission_id,
                &command.ids.command_id,
                &command.challenge_binding.idempotency_key,
                &snapshot.snapshot_id,
                &source_activity_position,
                &READABLE_EXPORT_PROFILE,
            ],
        )
        .await
        .map_err(export_database_error)?;
    Ok(())
}

async fn read_admitted_operation(
    store: &PostgresProjectReader,
    command: &ExportHumanReadableManuscriptCommand,
) -> Result<ExportHumanReadableManuscriptSettlement, ExportHumanReadableManuscriptError> {
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
                "SELECT operation.export_id::text,
                        operation.command_id::text,
                        operation.author_command_admission_id::text,
                        operation.source_snapshot_id::text
                   FROM storyos.human_readable_manuscript_export_operations AS operation
                   JOIN storyos.command_idempotency AS idempotency
                     ON (idempotency.owner_user_id, idempotency.project_id,
                         idempotency.command_kind, idempotency.idempotency_key) =
                        (operation.owner_user_id, operation.project_id,
                         operation.command_kind, operation.idempotency_key)
                  WHERE operation.owner_user_id = $1::text::uuid
                    AND operation.project_id = $2::text::uuid
                    AND operation.command_kind = 'exportHumanReadableManuscript'
                    AND operation.idempotency_key = $3::text::uuid
                    AND idempotency.outcome_kind = 'in_progress'
                    AND idempotency.canonical_command_digest = $4",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &command.challenge_binding.idempotency_key,
                    &command.challenge_binding.canonical_command_digest,
                ],
            )
            .await
            .map_err(export_database_error)?
            .ok_or(ExportHumanReadableManuscriptError::BindingConflict)?;
        let snapshot_id = row.get::<_, String>(3);
        let source_snapshot = crate::snapshot::load_canonical_snapshot_by_id(
            &client,
            &command.project_scope,
            &snapshot_id,
        )
        .await
        .map_err(export_read_error)?
        .ok_or(ExportHumanReadableManuscriptError::BindingConflict)?;
        Ok(ExportHumanReadableManuscriptSettlement {
            ids: AuthorCommandAdmissionIds {
                command_id: row.get(1),
                author_command_admission_id: row.get(2),
                receipt_id: command.ids.receipt_id.clone(),
            },
            export_id: row.get(0),
            effect: ExportHumanReadableManuscriptSettlementEffect::Admitted {
                source_snapshot: Box::new(source_snapshot),
            },
        })
    }
    .await;
    finish_readonly(&client, &result).await?;
    result
}

async fn read_settled_admission(
    store: &PostgresProjectReader,
    command: &ExportHumanReadableManuscriptCommand,
    receipt_id: &str,
) -> Result<ExportHumanReadableManuscriptSettlement, ExportHumanReadableManuscriptError> {
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
                        receipt.result_kind,
                        receipt.result_payload->>'reason',
                        export.export_id::text,
                        export.source_snapshot_id,
                        operation.export_id::text,
                        operation.source_snapshot_id::text
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
              LEFT JOIN storyos.human_readable_manuscript_exports AS export
                     ON (export.owner_user_id, export.project_id, export.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
              LEFT JOIN storyos.human_readable_manuscript_export_operations AS operation
                     ON (operation.owner_user_id, operation.project_id,
                         operation.author_command_admission_id) =
                        (receipt.owner_user_id, receipt.project_id,
                         receipt.author_command_admission_id)
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $3::text::uuid
                    AND receipt.command_kind = 'exportHumanReadableManuscript'
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
            .ok_or(ExportHumanReadableManuscriptError::BindingConflict)?;
        let result_kind = row.get::<_, String>(3);
        let source_snapshot_id = match result_kind.as_str() {
            "authoritative_applied" => row
                .get::<_, Option<String>>(6)
                .or_else(|| row.get::<_, Option<String>>(8)),
            "refused" => row.get::<_, Option<String>>(8),
            _ => None,
        }
        .ok_or(ExportHumanReadableManuscriptError::BindingConflict)?;
        let export_id = match result_kind.as_str() {
            "authoritative_applied" => row
                .get::<_, Option<String>>(5)
                .or_else(|| row.get::<_, Option<String>>(7)),
            "refused" => row.get::<_, Option<String>>(7),
            _ => None,
        }
        .ok_or(ExportHumanReadableManuscriptError::BindingConflict)?;
        let source_snapshot = crate::snapshot::load_canonical_snapshot_by_id(
            &client,
            &command.project_scope,
            &source_snapshot_id,
        )
        .await
        .map_err(export_read_error)?
        .ok_or(ExportHumanReadableManuscriptError::BindingConflict)?;
        Ok(ExportHumanReadableManuscriptSettlement {
            ids: AuthorCommandAdmissionIds {
                command_id: row.get(0),
                author_command_admission_id: row.get(1),
                receipt_id: row.get(2),
            },
            export_id,
            effect: ExportHumanReadableManuscriptSettlementEffect::Admitted {
                source_snapshot: Box::new(source_snapshot),
            },
        })
    }
    .await;
    finish_readonly(&client, &result).await?;
    result
}

async fn finish_readonly(
    client: &tokio_postgres::Client,
    result: &Result<ExportHumanReadableManuscriptSettlement, ExportHumanReadableManuscriptError>,
) -> Result<(), ExportHumanReadableManuscriptError> {
    match result {
        Ok(_) => client
            .batch_execute("COMMIT")
            .await
            .map_err(export_database_error),
        Err(_) => {
            let _rollback = client.batch_execute("ROLLBACK").await;
            Ok(())
        }
    }
}

fn snapshot_from_joined_row(
    row: &tokio_postgres::Row,
    start: usize,
) -> Result<CanonicalSnapshot, ProjectReadError> {
    Ok(CanonicalSnapshot {
        snapshot_id: row.get(start),
        project_activity_position: row
            .get::<_, String>(start + 1)
            .parse()
            .map_err(ProjectReadError::unavailable)?,
        replay_generation: row
            .get::<_, String>(start + 2)
            .parse()
            .map_err(ProjectReadError::unavailable)?,
        redaction_profile: row.get(start + 3),
        schema_profile: row.get(start + 4),
        created_at: row.get(start + 5),
        expires_at: row.get(start + 6),
        floor_position: row
            .get::<_, String>(start + 7)
            .parse()
            .map_err(ProjectReadError::unavailable)?,
    })
}

fn export_challenge_error(
    error: ProjectCommandChallengeError,
) -> ExportHumanReadableManuscriptError {
    match error {
        ProjectCommandChallengeError::BindingConflict => {
            ExportHumanReadableManuscriptError::BindingConflict
        }
        ProjectCommandChallengeError::InvalidOrExpired => {
            ExportHumanReadableManuscriptError::InvalidChallenge
        }
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => {
            ExportHumanReadableManuscriptError::Unavailable(Box::new(error))
        }
    }
}

fn export_database_error(error: tokio_postgres::Error) -> ExportHumanReadableManuscriptError {
    ExportHumanReadableManuscriptError::Unavailable(Box::new(error))
}

fn export_read_error(error: ProjectReadError) -> ExportHumanReadableManuscriptError {
    ExportHumanReadableManuscriptError::Unavailable(Box::new(error))
}
