use storyos_application::{
    AuthorCommandAdmissionIds, ChapterId, Project, ProjectAssistanceRecord,
    ProjectCommandChallengeError, ProjectCommandChallengeUse, ProjectReadError, ProjectScope,
    UpdateProjectAssistanceCommand, UpdateProjectAssistanceError,
    UpdateProjectAssistanceSettlement, UpdateProjectAssistanceSettlementEffect,
    UpdateProjectAssistanceStore,
};
use storyos_core::{
    AssistanceAvailability, AssistanceBindingPresence, ProjectPresence,
    UpdateProjectAssistance as CoreUpdateProjectAssistance, UpdateProjectAssistanceResult,
    update_project_assistance,
};
use uuid::Uuid;

use crate::command_response_project::{
    COMMAND_RESPONSE_PROJECT_FORMAT, CommandResponseProjectEvidence,
    encode_command_response_project, read_command_response_project,
};

use super::*;

pub(crate) const HOST_FAKE_MODEL_REGISTRATION_REVISION: &str =
    "018f0000-0000-7001-8000-00000000fa01";

impl UpdateProjectAssistanceStore for PostgresProjectReader {
    async fn update_project_assistance(
        &self,
        command: &UpdateProjectAssistanceCommand,
    ) -> Result<UpdateProjectAssistanceSettlement, UpdateProjectAssistanceError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(assistance_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(assistance_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(assistance_challenge_error)?;
                read_update_project_assistance_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(assistance_challenge_error)?;
                Err(UpdateProjectAssistanceError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_update_project_assistance(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction
                            .commit()
                            .await
                            .map_err(assistance_challenge_error)?;
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

pub(crate) async fn read_assistance_record(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
) -> Result<Option<ProjectAssistanceRecord>, ProjectReadError> {
    let row = client
        .query_opt(
            "SELECT policy.availability,
                    policy.policy_revision::text,
                    binding.model_registration_revision::text,
                    identity.processing_destination_identity::text,
                    evidence.evidence_revision::text,
                    binding.project_model_use_binding_revision::text,
                    decision.external_compatibility_decision::text
               FROM storyos.project_policy_revisions AS policy
               JOIN storyos.processing_destination_identities AS identity
                 ON (identity.owner_user_id, identity.project_id) =
                    (policy.owner_user_id, policy.project_id)
               JOIN storyos.project_external_use_binding_revisions AS binding
                 ON (binding.owner_user_id, binding.project_id) =
                    (policy.owner_user_id, policy.project_id)
               JOIN storyos.processing_destination_identity_evidence_revisions AS evidence
                 ON (evidence.owner_user_id, evidence.project_id,
                     evidence.processing_destination_identity, evidence.evidence_revision) =
                    (binding.owner_user_id, binding.project_id,
                     binding.processing_destination_identity, binding.evidence_revision)
               JOIN storyos.external_contract_compatibility_decisions AS decision
                 ON (decision.owner_user_id, decision.project_id) =
                    (policy.owner_user_id, policy.project_id)
              WHERE policy.owner_user_id = $1::text::uuid
                AND policy.project_id = $2::text::uuid
                AND policy.policy_revision = (
                      SELECT max(current.policy_revision)
                        FROM storyos.project_policy_revisions AS current
                       WHERE current.owner_user_id = policy.owner_user_id
                         AND current.project_id = policy.project_id
                    )",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(read_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    Ok(Some(ProjectAssistanceRecord {
        availability: parse_availability(&row.get::<_, String>(0))
            .map_err(ProjectReadError::unavailable)?,
        revision: parse_u64(row.get::<_, String>(1)).map_err(ProjectReadError::unavailable)?,
        model_registration_revision: row.get(2),
        processing_destination_identity: row.get(3),
        processing_destination_identity_evidence_revision: parse_u64(row.get::<_, String>(4))
            .map_err(ProjectReadError::unavailable)?,
        project_model_use_binding_revision: row.get(5),
        external_compatibility_decision: row.get(6),
    }))
}

async fn persist_update_project_assistance(
    client: &tokio_postgres::Client,
    command: &UpdateProjectAssistanceCommand,
) -> Result<UpdateProjectAssistanceSettlement, UpdateProjectAssistanceError> {
    let row = client
        .query_opt(
            "SELECT title, lifecycle_state, current_chapter_id::text
               FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              FOR UPDATE",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(assistance_database_error)?;
    let Some(row) = row else {
        return Err(UpdateProjectAssistanceError::MissingProject);
    };
    if row.get::<_, String>(1) != "active" {
        return Err(UpdateProjectAssistanceError::BindingConflict);
    }
    let current_title = row.get::<_, String>(0);
    let current_chapter_id = row.get::<_, Option<String>>(2).map(ChapterId::new);
    let current = read_assistance_record(client, &command.project_scope)
        .await
        .map_err(|error| UpdateProjectAssistanceError::Unavailable(Box::new(error)))?;
    let classified = update_project_assistance(&CoreUpdateProjectAssistance {
        presence: ProjectPresence::Present,
        binding: match &current {
            None => AssistanceBindingPresence::Uninitialized,
            Some(record) => AssistanceBindingPresence::Initialized {
                availability: record.availability,
                revision: record.revision,
            },
        },
        expected_revision: command.expected_revision,
        requested: command.availability,
    });
    let effect = match classified {
        UpdateProjectAssistanceResult::Initialized {
            availability,
            revision,
        } => UpdateProjectAssistanceSettlementEffect::Initialized {
            availability,
            revision,
        },
        UpdateProjectAssistanceResult::Applied {
            availability,
            revision,
        } => UpdateProjectAssistanceSettlementEffect::Applied {
            availability,
            revision,
        },
        UpdateProjectAssistanceResult::NoEffect { reason } => {
            UpdateProjectAssistanceSettlementEffect::NoEffect { reason }
        }
        UpdateProjectAssistanceResult::Conflicted { reason } => {
            UpdateProjectAssistanceSettlementEffect::Conflicted { reason }
        }
        UpdateProjectAssistanceResult::Refused {
            reason: storyos_core::UpdateProjectAssistanceRefusal::MissingProject,
        } => return Err(UpdateProjectAssistanceError::MissingProject),
    };
    insert_update_project_assistance_admission(client, command).await?;
    let (result_kind, result_payload) = match &effect {
        UpdateProjectAssistanceSettlementEffect::Initialized { .. }
        | UpdateProjectAssistanceSettlementEffect::Applied { .. } => {
            ("authoritative_applied", "{}".to_owned())
        }
        UpdateProjectAssistanceSettlementEffect::NoEffect { .. } => (
            "no_effect",
            r#"{"reason":"availability_unchanged"}"#.to_owned(),
        ),
        UpdateProjectAssistanceSettlementEffect::Conflicted { .. } => (
            "conflicted",
            r#"{"reason":"stale_assistance_revision"}"#.to_owned(),
        ),
        UpdateProjectAssistanceSettlementEffect::Refused { .. } => {
            return Err(UpdateProjectAssistanceError::BindingConflict);
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
                     $5::text::uuid, 'updateProjectAssistance', $6, $7::text::uuid,
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
        .map_err(assistance_database_error)?
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
        .map_err(assistance_database_error)?;
    let mut project_activity_position = 0;
    let mut project_activity_event_id = String::new();
    match &effect {
        UpdateProjectAssistanceSettlementEffect::Initialized {
            availability,
            revision,
        } => {
            initialize_host_fake_binding(client, command, *availability, *revision).await?;
            let activity =
                write_assistance_activity(client, command, *availability, *revision).await?;
            project_activity_position = activity.0;
            project_activity_event_id = activity.1;
        }
        UpdateProjectAssistanceSettlementEffect::Applied {
            availability,
            revision,
        } => {
            insert_policy_revision(client, command, *availability, *revision).await?;
            let activity =
                write_assistance_activity(client, command, *availability, *revision).await?;
            project_activity_position = activity.0;
            project_activity_event_id = activity.1;
        }
        UpdateProjectAssistanceSettlementEffect::NoEffect { .. }
        | UpdateProjectAssistanceSettlementEffect::Conflicted { .. }
        | UpdateProjectAssistanceSettlementEffect::Refused { .. } => {}
    }
    let assistance = read_assistance_record(client, &command.project_scope)
        .await
        .map_err(|error| UpdateProjectAssistanceError::Unavailable(Box::new(error)))?;
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
                AND command_kind = 'updateProjectAssistance' AND idempotency_key = $4::text::uuid",
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
        .map_err(assistance_database_error)?;
    Ok(UpdateProjectAssistanceSettlement {
        ids: command.ids.clone(),
        effect,
        receipt_created_at,
        project_activity_position,
        project_activity_event_id,
        response_project,
        assistance,
    })
}

async fn initialize_host_fake_binding(
    client: &tokio_postgres::Client,
    command: &UpdateProjectAssistanceCommand,
    availability: AssistanceAvailability,
    revision: u64,
) -> Result<(), UpdateProjectAssistanceError> {
    client
        .execute(
            "INSERT INTO storyos.model_registration_revisions
               (model_registration_revision, model_kind)
             VALUES ($1::text::uuid, 'host_fake')
             ON CONFLICT (model_registration_revision) DO NOTHING",
            &[&HOST_FAKE_MODEL_REGISTRATION_REVISION],
        )
        .await
        .map_err(assistance_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.model_registration_heads
               (model_kind, model_registration_revision)
             VALUES ('host_fake', $1::text::uuid)
             ON CONFLICT (model_kind) DO NOTHING",
            &[&HOST_FAKE_MODEL_REGISTRATION_REVISION],
        )
        .await
        .map_err(assistance_database_error)?;
    let identity = Uuid::now_v7().to_string();
    let grant_id = Uuid::now_v7().to_string();
    let binding_revision = Uuid::now_v7().to_string();
    let decision = Uuid::now_v7().to_string();
    client
        .execute(
            "INSERT INTO storyos.processing_destination_identities
               (owner_user_id, project_id, processing_destination_identity, destination_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 'host_fake')",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &identity,
            ],
        )
        .await
        .map_err(assistance_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.processing_destination_identity_evidence_revisions
               (owner_user_id, project_id, processing_destination_identity,
                evidence_revision, evidence_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 1, 'host_fake_boundary')",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &identity,
            ],
        )
        .await
        .map_err(assistance_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.project_destination_grants
               (owner_user_id, project_id, grant_id, processing_destination_identity, grant_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 'host_fake_use')",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &grant_id,
                &identity,
            ],
        )
        .await
        .map_err(assistance_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.project_external_use_binding_revisions
               (owner_user_id, project_id, project_model_use_binding_revision,
                processing_destination_identity, evidence_revision, grant_id,
                model_registration_revision)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 1,
                     $5::text::uuid, $6::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &binding_revision,
                &identity,
                &grant_id,
                &HOST_FAKE_MODEL_REGISTRATION_REVISION,
            ],
        )
        .await
        .map_err(assistance_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.external_contract_compatibility_decisions
               (owner_user_id, project_id, external_compatibility_decision,
                project_model_use_binding_revision, decision_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     'host_fake_compatible')",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &decision,
                &binding_revision,
            ],
        )
        .await
        .map_err(assistance_database_error)?;
    insert_policy_revision(client, command, availability, revision).await
}

async fn insert_policy_revision(
    client: &tokio_postgres::Client,
    command: &UpdateProjectAssistanceCommand,
    availability: AssistanceAvailability,
    revision: u64,
) -> Result<(), UpdateProjectAssistanceError> {
    client
        .execute(
            "INSERT INTO storyos.project_policy_revisions
               (owner_user_id, project_id, policy_revision, availability, receipt_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4, $5::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &revision.to_string(),
                &availability_text(availability),
                &command.ids.receipt_id,
            ],
        )
        .await
        .map_err(assistance_database_error)?;
    Ok(())
}

async fn write_assistance_activity(
    client: &tokio_postgres::Client,
    command: &UpdateProjectAssistanceCommand,
    availability: AssistanceAvailability,
    revision: u64,
) -> Result<(u64, String), UpdateProjectAssistanceError> {
    let project_activity_position = client
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
        .map_err(assistance_database_error)?
        .get::<_, String>(0)
        .parse::<u64>()
        .map_err(assistance_parse_error)?;
    let project_activity_event_id = Uuid::now_v7().to_string();
    let payload = serde_json::json!({
        "kind": "project_assistance_updated",
        "availability": availability_text(availability),
        "revision": revision.to_string(),
    })
    .to_string();
    client
        .execute(
            "INSERT INTO storyos.project_activity_event_payloads
               (owner_user_id, project_id, project_activity_position, project_activity_event_id,
                event_kind, receipt_id, receipt_result_kind, payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                     'project_assistance_updated', $5::text::uuid, 'authoritative_applied',
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
        .map_err(assistance_database_error)?;
    Ok((project_activity_position, project_activity_event_id))
}

async fn insert_update_project_assistance_admission(
    client: &tokio_postgres::Client,
    command: &UpdateProjectAssistanceCommand,
) -> Result<(), UpdateProjectAssistanceError> {
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
                    'explicit_project_command', $9, $10, $11, 'updateProjectAssistance',
                    $12, $13::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $14::text::uuid, NULL, NULL, '{}'::uuid[], '{}'::text[], NULL, $7,
                    NULL, NULL, NULL, convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.project_command_challenges AS challenge
              WHERE challenge.owner_user_id = $1::text::uuid
                AND challenge.project_id = $2::text::uuid
                AND challenge.command_kind = 'updateProjectAssistance'
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
        .map_err(assistance_database_error)?;
    if inserted != 1 {
        return Err(UpdateProjectAssistanceError::InvalidChallenge);
    }
    Ok(())
}

async fn read_update_project_assistance_settlement(
    store: &PostgresProjectReader,
    command: &UpdateProjectAssistanceCommand,
    receipt_id: &str,
) -> Result<UpdateProjectAssistanceSettlement, UpdateProjectAssistanceError> {
    let client = store
        .connect_challenge()
        .await
        .map_err(assistance_challenge_error)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(assistance_database_error)?;
    let result = async {
        set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(assistance_challenge_error)?;
        let row = client
            .query_opt(
                "SELECT receipt.command_id::text,
                        receipt.author_command_admission_id::text,
                        receipt.receipt_id::text,
                        to_char(receipt.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        receipt.result_kind,
                        receipt.result_payload->>'reason',
                        payload.payload->>'availability',
                        payload.payload->>'revision',
                        payload.project_activity_position::text,
                        payload.project_activity_event_id::text,
                        idempotency.acknowledgement_format,
                        idempotency.response_project::text
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
                    AND receipt.command_kind = 'updateProjectAssistance'
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
            .map_err(assistance_database_error)?
            .ok_or(UpdateProjectAssistanceError::BindingConflict)?;
        let result_kind = row.get::<_, String>(4);
        let reason = row.get::<_, Option<String>>(5);
        let effect = match (result_kind.as_str(), reason.as_deref()) {
            ("authoritative_applied", None) => {
                let availability = parse_availability(
                    row.get::<_, Option<String>>(6)
                        .as_deref()
                        .ok_or(UpdateProjectAssistanceError::BindingConflict)?,
                )
                .map_err(assistance_parse_error)?;
                let revision = row
                    .get::<_, Option<String>>(7)
                    .ok_or(UpdateProjectAssistanceError::BindingConflict)?
                    .parse::<u64>()
                    .map_err(assistance_parse_error)?;
                if command.expected_revision == 0 && revision == 1 {
                    UpdateProjectAssistanceSettlementEffect::Initialized {
                        availability,
                        revision,
                    }
                } else {
                    UpdateProjectAssistanceSettlementEffect::Applied {
                        availability,
                        revision,
                    }
                }
            }
            ("no_effect", Some("availability_unchanged")) => {
                UpdateProjectAssistanceSettlementEffect::NoEffect {
                    reason: storyos_core::UpdateProjectAssistanceNoEffect::AvailabilityUnchanged,
                }
            }
            ("conflicted", Some("stale_assistance_revision")) => {
                UpdateProjectAssistanceSettlementEffect::Conflicted {
                    reason: storyos_core::UpdateProjectAssistanceConflict::StaleAssistanceRevision,
                }
            }
            _ => return Err(UpdateProjectAssistanceError::BindingConflict),
        };
        let response_project = match read_command_response_project(
            row.get::<_, Option<String>>(10).as_deref(),
            row.get::<_, Option<String>>(11).as_deref(),
        ) {
            Ok(CommandResponseProjectEvidence::Captured(project)) => project,
            Ok(CommandResponseProjectEvidence::HistoricalUnavailable) => {
                return Err(UpdateProjectAssistanceError::HistoricalAcknowledgementUnavailable);
            }
            Err(()) => {
                return Err(UpdateProjectAssistanceError::Unavailable(Box::new(
                    std::io::Error::other(
                        "Update Project Assistance acknowledgement evidence is damaged",
                    ),
                )));
            }
        };
        let assistance = read_assistance_record(&client, &command.project_scope)
            .await
            .map_err(|error| UpdateProjectAssistanceError::Unavailable(Box::new(error)))?;
        Ok(UpdateProjectAssistanceSettlement {
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
                .map_err(assistance_parse_error)?,
            project_activity_event_id: row.get::<_, Option<String>>(9).unwrap_or_default(),
            response_project,
            assistance,
        })
    }
    .await;
    match &result {
        Ok(_) => client
            .batch_execute("COMMIT")
            .await
            .map_err(assistance_database_error)?,
        Err(_) => {
            let _rollback = client.batch_execute("ROLLBACK").await;
        }
    }
    result
}

fn availability_text(availability: AssistanceAvailability) -> &'static str {
    match availability {
        AssistanceAvailability::Available => "available",
        AssistanceAvailability::Unavailable => "unavailable",
    }
}

fn parse_availability(value: &str) -> Result<AssistanceAvailability, std::io::Error> {
    match value {
        "available" => Ok(AssistanceAvailability::Available),
        "unavailable" => Ok(AssistanceAvailability::Unavailable),
        _ => Err(std::io::Error::other(
            "Project assistance availability is damaged",
        )),
    }
}

fn parse_u64(value: String) -> Result<u64, std::io::Error> {
    value
        .parse()
        .map_err(|_| std::io::Error::other("Project assistance revision is damaged"))
}

fn assistance_challenge_error(error: ProjectCommandChallengeError) -> UpdateProjectAssistanceError {
    match error {
        ProjectCommandChallengeError::BindingConflict => {
            UpdateProjectAssistanceError::BindingConflict
        }
        ProjectCommandChallengeError::InvalidOrExpired => {
            UpdateProjectAssistanceError::InvalidChallenge
        }
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => {
            UpdateProjectAssistanceError::Unavailable(Box::new(error))
        }
    }
}

fn assistance_database_error(error: tokio_postgres::Error) -> UpdateProjectAssistanceError {
    UpdateProjectAssistanceError::Unavailable(Box::new(error))
}

fn assistance_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> UpdateProjectAssistanceError {
    UpdateProjectAssistanceError::Unavailable(Box::new(error))
}
