use storyos_application::{
    ArchiveProjectCommand, ArchiveProjectError, ArchiveProjectSettlement,
    ArchiveProjectSettlementEffect, ArchiveProjectStore, AuthorCommandAdmissionIds,
    ProjectCommandChallengeError, ProjectCommandChallengeUse,
};
use storyos_core::{
    ArchiveProject as CoreArchiveProject, ArchiveProjectResult, ProjectLifecycle, ProjectPresence,
    archive_project,
};
use uuid::Uuid;

use super::*;

impl ArchiveProjectStore for PostgresProjectReader {
    async fn archive_project(
        &self,
        command: &ArchiveProjectCommand,
    ) -> Result<ArchiveProjectSettlement, ArchiveProjectError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(archive_project_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(archive_project_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(archive_project_challenge_error)?;
                read_archive_project_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(archive_project_challenge_error)?;
                Err(ArchiveProjectError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_archive_project(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction
                            .commit()
                            .await
                            .map_err(archive_project_challenge_error)?;
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

async fn persist_archive_project(
    client: &tokio_postgres::Client,
    command: &ArchiveProjectCommand,
) -> Result<ArchiveProjectSettlement, ArchiveProjectError> {
    let row = client
        .query_opt(
            "SELECT lifecycle_state, revision::text FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              FOR UPDATE",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(archive_project_database_error)?;
    let Some(row) = row else {
        return Err(ArchiveProjectError::MissingProject);
    };
    let current_lifecycle = match row.get::<_, String>(0).as_str() {
        "active" => ProjectLifecycle::Active,
        "archived" => ProjectLifecycle::Archived,
        other => {
            return Err(ArchiveProjectError::Unavailable(Box::new(
                std::io::Error::other(format!("unsupported Project lifecycle {other}")),
            )));
        }
    };
    let current_revision = row
        .get::<_, String>(1)
        .parse::<u64>()
        .map_err(archive_project_parse_error)?;
    let classified = archive_project(&CoreArchiveProject {
        presence: ProjectPresence::Present,
        expected_revision: command.expected_revision,
        current_revision,
        current_lifecycle,
    });
    let effect = match classified {
        ArchiveProjectResult::Applied { revision } => {
            ArchiveProjectSettlementEffect::Applied { revision }
        }
        ArchiveProjectResult::NoEffect { reason } => {
            ArchiveProjectSettlementEffect::NoEffect { reason }
        }
        ArchiveProjectResult::Conflicted { reason } => {
            ArchiveProjectSettlementEffect::Conflicted { reason }
        }
        ArchiveProjectResult::Refused {
            reason: storyos_core::ArchiveProjectRefusal::MissingProject,
        } => return Err(ArchiveProjectError::MissingProject),
    };
    insert_archive_project_admission(client, command).await?;
    let (result_kind, result_payload) = match &effect {
        ArchiveProjectSettlementEffect::Applied { .. } => {
            ("authoritative_applied", "{}".to_owned())
        }
        ArchiveProjectSettlementEffect::NoEffect { .. } => {
            ("no_effect", r#"{"reason":"already_archived"}"#.to_owned())
        }
        ArchiveProjectSettlementEffect::Conflicted { .. } => (
            "conflicted",
            r#"{"reason":"stale_project_revision"}"#.to_owned(),
        ),
        ArchiveProjectSettlementEffect::Refused { .. } => {
            return Err(ArchiveProjectError::BindingConflict);
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
                     $5::text::uuid, 'archiveProject', $6, $7::text::uuid,
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
        .map_err(archive_project_database_error)?
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
        .map_err(archive_project_database_error)?;
    let mut project_activity_position = 0;
    let mut project_activity_event_id = String::new();
    if let ArchiveProjectSettlementEffect::Applied { revision } = &effect {
        let updated = client
            .execute(
                "UPDATE storyos.projects
                    SET lifecycle_state = 'archived', revision = $3::text::bigint
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND revision = $4::text::bigint AND lifecycle_state = 'active'",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &revision.to_string(),
                    &command.expected_revision.to_string(),
                ],
            )
            .await
            .map_err(archive_project_database_error)?;
        if updated != 1 {
            return Err(ArchiveProjectError::Unavailable(Box::new(
                std::io::Error::other("Project revision changed under FOR UPDATE"),
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
            .map_err(archive_project_database_error)?
            .get::<_, String>(0)
            .parse::<u64>()
            .map_err(archive_project_parse_error)?;
        project_activity_event_id = Uuid::now_v7().to_string();
        let payload = serde_json::json!({
            "kind": "project_archival_changed",
            "lifecycle": "archived",
            "revision": revision.to_string(),
        })
        .to_string();
        client
            .execute(
                "INSERT INTO storyos.project_activity_event_payloads
                   (owner_user_id, project_id, project_activity_position, project_activity_event_id,
                    event_kind, receipt_id, receipt_result_kind, payload)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                         'project_archival_changed', $5::text::uuid, 'authoritative_applied',
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
            .map_err(archive_project_database_error)?;
        client
            .execute(
                "INSERT INTO storyos.project_archival_decisions
                   (owner_user_id, project_id, archival_decision_id, receipt_id,
                    prior_lifecycle_state, resulting_lifecycle_state, project_revision)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         'active', 'archived', $5::text::bigint)",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &Uuid::now_v7().to_string(),
                    &command.ids.receipt_id,
                    &revision.to_string(),
                ],
            )
            .await
            .map_err(archive_project_database_error)?;
    }
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'archiveProject' AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(archive_project_database_error)?;
    Ok(ArchiveProjectSettlement {
        ids: command.ids.clone(),
        effect,
        receipt_created_at,
        project_activity_position,
        project_activity_event_id,
    })
}

async fn insert_archive_project_admission(
    client: &tokio_postgres::Client,
    command: &ArchiveProjectCommand,
) -> Result<(), ArchiveProjectError> {
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
                    'explicit_project_command', $9, $10, $11, 'archiveProject',
                    $12, $13::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $14::text::uuid, NULL, NULL, '{}'::uuid[], '{}'::text[], NULL, $7,
                    NULL, NULL, NULL, convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.project_command_challenges AS challenge
              WHERE challenge.owner_user_id = $1::text::uuid
                AND challenge.project_id = $2::text::uuid
                AND challenge.command_kind = 'archiveProject'
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
        .map_err(archive_project_database_error)?;
    if inserted != 1 {
        return Err(ArchiveProjectError::InvalidChallenge);
    }
    Ok(())
}

async fn read_archive_project_settlement(
    store: &PostgresProjectReader,
    command: &ArchiveProjectCommand,
    receipt_id: &str,
) -> Result<ArchiveProjectSettlement, ArchiveProjectError> {
    let client = store
        .connect_challenge()
        .await
        .map_err(archive_project_challenge_error)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(archive_project_database_error)?;
    let result = async {
        set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(archive_project_challenge_error)?;
        let row = client
            .query_opt(
                "SELECT receipt.command_id::text,
                        receipt.author_command_admission_id::text,
                        receipt.receipt_id::text,
                        to_char(receipt.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        receipt.result_kind,
                        receipt.result_payload->>'reason',
                        payload.payload->>'revision',
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
                    AND receipt.command_kind = 'archiveProject'
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
            .map_err(archive_project_database_error)?
            .ok_or(ArchiveProjectError::BindingConflict)?;
        let result_kind = row.get::<_, String>(4);
        let reason = row.get::<_, Option<String>>(5);
        let applied_revision = row.get::<_, Option<String>>(6);
        let effect = match (result_kind.as_str(), reason.as_deref()) {
            ("authoritative_applied", None) => {
                let revision = applied_revision
                    .ok_or(ArchiveProjectError::BindingConflict)?
                    .parse::<u64>()
                    .map_err(archive_project_parse_error)?;
                ArchiveProjectSettlementEffect::Applied { revision }
            }
            ("no_effect", Some("already_archived")) => ArchiveProjectSettlementEffect::NoEffect {
                reason: storyos_core::ArchiveProjectNoEffect::AlreadyArchived,
            },
            ("conflicted", Some("stale_project_revision")) => {
                ArchiveProjectSettlementEffect::Conflicted {
                    reason: storyos_core::ArchiveProjectConflict::StaleProjectRevision,
                }
            }
            _ => return Err(ArchiveProjectError::BindingConflict),
        };
        Ok(ArchiveProjectSettlement {
            ids: AuthorCommandAdmissionIds {
                command_id: row.get(0),
                author_command_admission_id: row.get(1),
                receipt_id: row.get(2),
            },
            receipt_created_at: row.get(3),
            effect,
            project_activity_position: row
                .get::<_, Option<String>>(7)
                .unwrap_or_else(|| "0".to_owned())
                .parse::<u64>()
                .map_err(archive_project_parse_error)?,
            project_activity_event_id: row.get::<_, Option<String>>(8).unwrap_or_default(),
        })
    }
    .await;
    match &result {
        Ok(_) => client
            .batch_execute("COMMIT")
            .await
            .map_err(archive_project_database_error)?,
        Err(_) => {
            let _rollback = client.batch_execute("ROLLBACK").await;
        }
    }
    result
}

fn archive_project_challenge_error(error: ProjectCommandChallengeError) -> ArchiveProjectError {
    match error {
        ProjectCommandChallengeError::BindingConflict => ArchiveProjectError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired => ArchiveProjectError::InvalidChallenge,
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => {
            ArchiveProjectError::Unavailable(Box::new(error))
        }
    }
}

fn archive_project_database_error(error: tokio_postgres::Error) -> ArchiveProjectError {
    ArchiveProjectError::Unavailable(Box::new(error))
}

fn archive_project_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> ArchiveProjectError {
    ArchiveProjectError::Unavailable(Box::new(error))
}
