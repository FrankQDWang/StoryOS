use crate::command_response_project::{
    COMMAND_RESPONSE_PROJECT_FORMAT, encode_command_response_project,
};
use storyos_application::{
    ChapterId, CreateVolumeAuthority, CreateVolumeCommand, CreateVolumeError,
    CreateVolumePublicOrder, CreateVolumeSettlement, CreateVolumeSettlementEffect,
    CreateVolumeStore, Project, ProjectCommandChallengeError, ProjectCommandChallengeUse,
};
use storyos_core::{
    CreateVolume as CoreCreateVolume, CreateVolumeApplied, CreateVolumeResult, ProjectLifecycle,
    create_volume as classify_create_volume,
};
use uuid::Uuid;

use crate::structural_authority_settlement::receipt_reason_payload;

use super::*;
use crate::command_replay::{CommandReplay, ReplayFault, read_command_replay};
use storyos_core::TransitionOutcome;

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
            "SELECT lifecycle_state, tree_revision::text, title, current_chapter_id::text
               FROM storyos.projects
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
    let current_title = row.get::<_, String>(2);
    let current_chapter_id = row.get::<_, Option<String>>(3).map(ChapterId::new);
    let classified = classify_create_volume(&CoreCreateVolume {
        expected_tree_revision: command.expected_tree_revision,
        current_tree_revision,
        current_lifecycle,
        title: command.title.clone(),
    });
    let receipt_result = classified.receipt_result().code();
    let receipt_reason = classified.reason_code();
    let effect = match classified {
        CreateVolumeResult::Applied(CreateVolumeApplied { tree_revision }) => {
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
        CreateVolumeResult::NoEffect(reason) => match reason {},
        CreateVolumeResult::Conflicted(reason) => {
            CreateVolumeSettlementEffect::Conflicted { reason }
        }
        CreateVolumeResult::Refused(reason) => CreateVolumeSettlementEffect::Refused { reason },
    };
    insert_create_volume_admission(client, command).await?;
    let authority_sequences = match &effect {
        CreateVolumeSettlementEffect::Applied { volume_id, .. } => {
            let sequences =
                crate::structural_authority_settlement::allocate_structure_transition_sequences(
                    client,
                    &command.project_scope,
                )
                .await
                .map_err(CreateVolumeError::Unavailable)?;
            Some((sequences, volume_id.clone()))
        }
        CreateVolumeSettlementEffect::Conflicted { .. }
        | CreateVolumeSettlementEffect::Refused { .. } => None,
    };
    let receipt_payload = match &effect {
        CreateVolumeSettlementEffect::Applied {
            order: CreateVolumePublicOrder::CanonicalSiblingOrder(order),
            ..
        } => format!(r#"{{"order":"{order}"}}"#),
        CreateVolumeSettlementEffect::Applied {
            order: CreateVolumePublicOrder::HistoricalCreateVolumeAck,
            ..
        } => {
            return Err(CreateVolumeError::Unavailable(Box::new(
                std::io::Error::other("Create Volume first use cannot replay a historical ack"),
            )));
        }
        CreateVolumeSettlementEffect::Conflicted { .. }
        | CreateVolumeSettlementEffect::Refused { .. } => receipt_reason_payload(receipt_reason),
    };
    let commit_ids = authority_sequences
        .as_ref()
        .map(|(sequences, _)| vec![sequences.authoritative_commit_id.clone()])
        .unwrap_or_default();
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
                     '{}'::uuid[], '{}'::uuid[], $10::text[]::uuid[], '{}'::text[], '{}'::text[],
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
                &receipt_result,
                &receipt_payload,
                &commit_ids,
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
    let mut authority = None;
    if let (
        CreateVolumeSettlementEffect::Applied {
            tree_revision,
            volume_id,
            order: CreateVolumePublicOrder::CanonicalSiblingOrder(order),
        },
        Some((sequences, _)),
    ) = (&effect, authority_sequences)
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
        project_activity_position = sequences.project_activity_position;
        project_activity_event_id = sequences.project_activity_event_id.clone();
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
        crate::structural_authority_settlement::persist_structure_commit(
            client,
            &command.project_scope,
            &sequences,
            &command.ids.author_command_admission_id,
            &command.ids.receipt_id,
            crate::structural_authority_settlement::StructureCommitBinding {
                prior_manuscript_tree_revision: command.expected_tree_revision,
                resulting_manuscript_tree_revision: *tree_revision,
                identity:
                    crate::structural_authority_settlement::StructureAffectedIdentity::Volume {
                        volume_id,
                    },
            },
        )
        .await
        .map_err(create_volume_database_error)?;
        crate::structural_authority_settlement::persist_forward_author_action(
            client,
            &command.project_scope,
            &sequences,
            &command.ids.receipt_id,
        )
        .await
        .map_err(create_volume_database_error)?;
        crate::snapshot::persist_canonical_snapshot(
            client,
            &command.project_scope,
            &sequences.snapshot_id,
            project_activity_position,
        )
        .await
        .map_err(create_volume_database_error)?;
        authority = Some(CreateVolumeAuthority {
            authoritative_commit_id: sequences.authoritative_commit_id,
            author_action_sequence: sequences.author_action_sequence,
            snapshot_id: sequences.snapshot_id,
            prior_manuscript_tree_revision: command.expected_tree_revision,
            resulting_manuscript_tree_revision: *tree_revision,
        });
    }
    let response_project = Project {
        project_id: command.project_scope.project_id.clone(),
        title: current_title,
        current_chapter_id,
    };
    let encoded_project = encode_command_response_project(&response_project);
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled',
                    result_reference = $3,
                    acknowledgement_format = $5,
                    response_project = $6::text::jsonb
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'createVolume' AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
                &COMMAND_RESPONSE_PROJECT_FORMAT,
                &encoded_project,
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
        authority,
        response_project,
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
    read_command_replay(store, &command.challenge_binding, receipt_id)
        .await
        .and_then(create_volume_replay)
        .map_err(|fault| match fault {
            ReplayFault::BindingConflict => CreateVolumeError::BindingConflict,
            ReplayFault::HistoricalAcknowledgementUnavailable => {
                CreateVolumeError::HistoricalAcknowledgementUnavailable
            }
            ReplayFault::Unavailable(source) => CreateVolumeError::Unavailable(source),
        })
}

fn create_volume_replay(replay: CommandReplay) -> Result<CreateVolumeSettlement, ReplayFault> {
    let effect = match replay.outcome::<std::convert::Infallible, _, _>()? {
        TransitionOutcome::Applied(()) => {
            let tree_revision = replay.activity_u64("tree_revision")?;
            let volume_id = replay.activity_text("volume_id")?;
            let order = match replay.receipt_text("order") {
                Some(order) => {
                    let rank = order
                        .parse::<u64>()
                        .map_err(|error| ReplayFault::Unavailable(Box::new(error)))?;
                    if rank < 1 {
                        return Err(ReplayFault::BindingConflict);
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
        TransitionOutcome::NoEffect(reason) => match reason {},
        TransitionOutcome::Conflicted(reason) => {
            CreateVolumeSettlementEffect::Conflicted { reason }
        }
        TransitionOutcome::Refused(reason) => CreateVolumeSettlementEffect::Refused { reason },
    };
    let response_project = replay.response_project()?;
    Ok(CreateVolumeSettlement {
        authority: replay.authority.map(|authority| CreateVolumeAuthority {
            authoritative_commit_id: authority.authoritative_commit_id,
            author_action_sequence: authority.author_action_sequence,
            snapshot_id: authority.snapshot_id,
            prior_manuscript_tree_revision: authority.prior_manuscript_tree_revision,
            resulting_manuscript_tree_revision: authority.resulting_manuscript_tree_revision,
        }),
        ids: replay.ids,
        receipt_created_at: replay.receipt_created_at,
        effect,
        project_activity_position: replay.project_activity_position,
        project_activity_event_id: replay.project_activity_event_id,
        response_project,
    })
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
