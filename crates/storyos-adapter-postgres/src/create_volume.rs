use storyos_application::{
    AuthorCommandAdmissionIds, CreateVolumeCommand, CreateVolumeError, CreateVolumePublicOrder,
    CreateVolumeSettlement, CreateVolumeSettlementEffect, CreateVolumeStore,
    ProjectCommandChallengeError, ProjectCommandChallengeUse,
};
use storyos_core::{
    CreateVolume as CoreCreateVolume, CreateVolumeResult, ProjectLifecycle, ProjectPresence,
    create_volume as classify_create_volume,
};
use uuid::Uuid;

use super::*;

impl CreateVolumeStore for PostgresProjectReader {
    async fn create_volume(
        &self,
        command: &CreateVolumeCommand,
    ) -> Result<CreateVolumeSettlement, CreateVolumeError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(create_volume_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(create_volume_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(create_volume_challenge_error)?;
                read_create_volume_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(create_volume_challenge_error)?;
                Err(CreateVolumeError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_create_volume(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction
                            .commit()
                            .await
                            .map_err(create_volume_challenge_error)?;
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

async fn persist_create_volume(
    client: &tokio_postgres::Client,
    command: &CreateVolumeCommand,
) -> Result<CreateVolumeSettlement, CreateVolumeError> {
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
        .map_err(create_volume_database_error)?;
    let Some(row) = row else {
        return Err(CreateVolumeError::MissingProject);
    };
    let current_lifecycle = match row.get::<_, String>(0).as_str() {
        "active" => ProjectLifecycle::Active,
        "archived" => ProjectLifecycle::Archived,
        other => {
            return Err(CreateVolumeError::Unavailable(Box::new(
                std::io::Error::other(format!("unsupported Project lifecycle {other}")),
            )));
        }
    };
    let current_tree_revision = row
        .get::<_, String>(1)
        .parse::<u64>()
        .map_err(create_volume_parse_error)?;
    let classified = classify_create_volume(&CoreCreateVolume {
        presence: ProjectPresence::Present,
        expected_tree_revision: command.expected_tree_revision,
        current_tree_revision,
        current_lifecycle,
        title: command.title.clone(),
    });
    let effect = match classified {
        CreateVolumeResult::Applied { tree_revision } => {
            let volume_id = Uuid::now_v7().to_string();
            let tree_order = client
                .query_one(
                    "SELECT (COALESCE(MAX(tree_order), 0) + 1)::text
                       FROM storyos.manuscript_objects
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND object_kind = 'volume'",
                    &[
                        &command.project_scope.owner_user_id.as_ref(),
                        &command.project_scope.project_id.as_ref(),
                    ],
                )
                .await
                .map_err(create_volume_database_error)?
                .get::<_, String>(0);
            client
                .execute(
                    "INSERT INTO storyos.manuscript_objects
                       (owner_user_id, project_id, manuscript_object_id, object_kind, title, tree_order)
                     VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 'volume', $4, $5::text::bigint)",
                    &[
                        &command.project_scope.owner_user_id.as_ref(),
                        &command.project_scope.project_id.as_ref(),
                        &volume_id,
                        &command.title,
                        &tree_order,
                    ],
                )
                .await
                .map_err(create_volume_database_error)?;
            let canonical_sibling_order = client
                .query_one(
                    "SELECT count(*)::text
                       FROM storyos.manuscript_objects AS volume
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND object_kind = 'volume'
                        AND NOT EXISTS (
                          SELECT 1 FROM storyos.volume_removal_decisions AS removal
                           WHERE removal.owner_user_id = volume.owner_user_id
                             AND removal.project_id = volume.project_id
                             AND removal.volume_id = volume.manuscript_object_id
                        )",
                    &[
                        &command.project_scope.owner_user_id.as_ref(),
                        &command.project_scope.project_id.as_ref(),
                    ],
                )
                .await
                .map_err(create_volume_database_error)?
                .get::<_, String>(0)
                .parse::<u64>()
                .map_err(create_volume_parse_error)?;
            CreateVolumeSettlementEffect::Applied {
                tree_revision,
                volume_id,
                order: CreateVolumePublicOrder::CanonicalSiblingOrder(canonical_sibling_order),
            }
        }
        CreateVolumeResult::Conflicted { reason } => {
            CreateVolumeSettlementEffect::Conflicted { reason }
        }
        CreateVolumeResult::Refused {
            reason: storyos_core::CreateVolumeRefusal::MissingProject,
        } => return Err(CreateVolumeError::MissingProject),
        CreateVolumeResult::Refused { reason } => CreateVolumeSettlementEffect::Refused { reason },
    };
    insert_create_volume_admission(client, command).await?;
    let (result_kind, result_payload) = match &effect {
        CreateVolumeSettlementEffect::Applied {
            order: CreateVolumePublicOrder::CanonicalSiblingOrder(order),
            ..
        } => ("authoritative_applied", format!(r#"{{"order":"{order}"}}"#)),
        CreateVolumeSettlementEffect::Applied {
            order: CreateVolumePublicOrder::HistoricalCreateVolumeAck,
            ..
        } => {
            return Err(CreateVolumeError::Unavailable(Box::new(
                std::io::Error::other("Create Volume first use cannot replay a historical ack"),
            )));
        }
        CreateVolumeSettlementEffect::Conflicted { .. } => (
            "conflicted",
            r#"{"reason":"stale_tree_revision"}"#.to_owned(),
        ),
        CreateVolumeSettlementEffect::Refused { reason } => {
            let refused = match reason {
                storyos_core::CreateVolumeRefusal::ArchivedProject => "archived_project",
                storyos_core::CreateVolumeRefusal::InvalidTitle => "invalid_title",
                storyos_core::CreateVolumeRefusal::MissingProject => {
                    return Err(CreateVolumeError::MissingProject);
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
                     $5::text::uuid, 'createVolume', $6, $7::text::uuid,
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
        .map_err(create_volume_database_error)?
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
        .map_err(create_volume_database_error)?;
    let mut project_activity_position = 0;
    let mut project_activity_event_id = String::new();
    if let CreateVolumeSettlementEffect::Applied {
        tree_revision,
        volume_id,
        order: CreateVolumePublicOrder::CanonicalSiblingOrder(order),
    } = &effect
    {
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
                    &command.expected_tree_revision.to_string(),
                ],
            )
            .await
            .map_err(create_volume_database_error)?;
        if updated != 1 {
            return Err(CreateVolumeError::Unavailable(Box::new(
                std::io::Error::other("tree revision changed under FOR UPDATE"),
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
            .map_err(create_volume_database_error)?
            .get::<_, String>(0)
            .parse::<u64>()
            .map_err(create_volume_parse_error)?;
        project_activity_event_id = Uuid::now_v7().to_string();
        let payload = serde_json::json!({
            "kind": "volume_created",
            "volume_id": volume_id,
            "title": command.title,
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
                         'volume_created', $5::text::uuid, 'authoritative_applied',
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
            .map_err(create_volume_database_error)?;
    }
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'createVolume' AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(create_volume_database_error)?;
    Ok(CreateVolumeSettlement {
        ids: command.ids.clone(),
        effect,
        receipt_created_at,
        project_activity_position,
        project_activity_event_id,
    })
}

async fn insert_create_volume_admission(
    client: &tokio_postgres::Client,
    command: &CreateVolumeCommand,
) -> Result<(), CreateVolumeError> {
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
                    'explicit_project_command', $9, $10, $11, 'createVolume',
                    $12, $13::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $14::text::uuid, NULL, NULL, '{}'::uuid[], '{}'::text[], NULL, $7,
                    NULL, NULL, NULL, convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.project_command_challenges AS challenge
              WHERE challenge.owner_user_id = $1::text::uuid
                AND challenge.project_id = $2::text::uuid
                AND challenge.command_kind = 'createVolume'
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
        .map_err(create_volume_database_error)?;
    if inserted != 1 {
        return Err(CreateVolumeError::InvalidChallenge);
    }
    Ok(())
}

async fn read_create_volume_settlement(
    store: &PostgresProjectReader,
    command: &CreateVolumeCommand,
    receipt_id: &str,
) -> Result<CreateVolumeSettlement, CreateVolumeError> {
    let client = store
        .connect_challenge()
        .await
        .map_err(create_volume_challenge_error)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(create_volume_database_error)?;
    let result = async {
        set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(create_volume_challenge_error)?;
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
                        payload.payload->>'volume_id',
                        payload.project_activity_position::text,
                        payload.project_activity_event_id::text,
                        receipt.result_payload->>'order'
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
                    AND receipt.command_kind = 'createVolume'
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
            .map_err(create_volume_database_error)?
            .ok_or(CreateVolumeError::BindingConflict)?;
        let result_kind = row.get::<_, String>(4);
        let reason = row.get::<_, Option<String>>(5);
        let effect = match (result_kind.as_str(), reason.as_deref()) {
            ("authoritative_applied", None) => {
                let tree_revision = row
                    .get::<_, Option<String>>(6)
                    .ok_or(CreateVolumeError::BindingConflict)?
                    .parse::<u64>()
                    .map_err(create_volume_parse_error)?;
                let volume_id = row
                    .get::<_, Option<String>>(7)
                    .ok_or(CreateVolumeError::BindingConflict)?;
                let order = match row.get::<_, Option<String>>(10).as_deref() {
                    Some(order) => {
                        let rank = order.parse::<u64>().map_err(create_volume_parse_error)?;
                        if rank < 1 {
                            return Err(CreateVolumeError::BindingConflict);
                        }
                        CreateVolumePublicOrder::CanonicalSiblingOrder(rank)
                    }
                    None => CreateVolumePublicOrder::HistoricalCreateVolumeAck,
                };
                CreateVolumeSettlementEffect::Applied {
                    tree_revision,
                    volume_id,
                    order,
                }
            }
            ("conflicted", Some("stale_tree_revision")) => {
                CreateVolumeSettlementEffect::Conflicted {
                    reason: storyos_core::CreateVolumeConflict::StaleTreeRevision,
                }
            }
            ("refused", Some("archived_project")) => CreateVolumeSettlementEffect::Refused {
                reason: storyos_core::CreateVolumeRefusal::ArchivedProject,
            },
            ("refused", Some("invalid_title")) => CreateVolumeSettlementEffect::Refused {
                reason: storyos_core::CreateVolumeRefusal::InvalidTitle,
            },
            _ => return Err(CreateVolumeError::BindingConflict),
        };
        Ok(CreateVolumeSettlement {
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
                .map_err(create_volume_parse_error)?,
            project_activity_event_id: row.get::<_, Option<String>>(9).unwrap_or_default(),
        })
    }
    .await;
    match &result {
        Ok(_) => client
            .batch_execute("COMMIT")
            .await
            .map_err(create_volume_database_error)?,
        Err(_) => {
            let _rollback = client.batch_execute("ROLLBACK").await;
        }
    }
    result
}

fn create_volume_challenge_error(error: ProjectCommandChallengeError) -> CreateVolumeError {
    match error {
        ProjectCommandChallengeError::BindingConflict => CreateVolumeError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired => CreateVolumeError::InvalidChallenge,
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => {
            CreateVolumeError::Unavailable(Box::new(error))
        }
    }
}

fn create_volume_database_error(error: tokio_postgres::Error) -> CreateVolumeError {
    CreateVolumeError::Unavailable(Box::new(error))
}

fn create_volume_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> CreateVolumeError {
    CreateVolumeError::Unavailable(Box::new(error))
}
