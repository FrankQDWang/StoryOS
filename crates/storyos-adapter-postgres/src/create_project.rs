use storyos_application::{
    AuthorCommandAdmissionIds, CreateProjectCommand, CreateProjectError, CreateProjectSettlement,
    CreateProjectStore, ProjectCommandChallengeError,
};
use uuid::Uuid;

use super::*;

impl CreateProjectStore for PostgresProjectReader {
    async fn create_project(
        &self,
        command: &CreateProjectCommand,
    ) -> Result<CreateProjectSettlement, CreateProjectError> {
        let client = self
            .connect_challenge()
            .await
            .map_err(create_project_challenge_error)?;
        client
            .batch_execute("BEGIN ISOLATION LEVEL SERIALIZABLE")
            .await
            .map_err(create_project_database_error)?;
        client
            .execute(
                "SELECT set_config('storyos.user_id', $1, true)",
                &[&command.project_scope.owner_user_id.as_ref()],
            )
            .await
            .map_err(create_project_database_error)?;
        let challenge_use = match consume_create_project_challenge(&client, command).await {
            Ok(challenge_use) => challenge_use,
            Err(error) => {
                let _rollback = client.batch_execute("ROLLBACK").await;
                return Err(error);
            }
        };
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                client
                    .batch_execute("ROLLBACK")
                    .await
                    .map_err(create_project_database_error)?;
                read_create_project_settlement(&client, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                let _rollback = client.batch_execute("ROLLBACK").await;
                Err(CreateProjectError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_create_project(&client, command).await {
                    Ok(settlement) => {
                        client
                            .batch_execute("COMMIT")
                            .await
                            .map_err(create_project_database_error)?;
                        Ok(settlement)
                    }
                    Err(error) => {
                        let _rollback = client.batch_execute("ROLLBACK").await;
                        Err(error)
                    }
                }
            }
        }
    }
}

async fn consume_create_project_challenge(
    client: &tokio_postgres::Client,
    command: &CreateProjectCommand,
) -> Result<ProjectCommandChallengeUse, CreateProjectError> {
    let binding = &command.challenge_binding;
    let client_session_generation = binding.client_session_generation.to_string();
    let row = client
        .query_opt(
            "SELECT challenge.client_session_binding_digest,
                    challenge.client_session_generation::text,
                    challenge.client_contract_revision, challenge.security_policy_revision,
                    challenge.limit_profile_revision, challenge.method,
                    challenge.route_template, challenge.command_schema, challenge.command_kind,
                    challenge.canonical_command_digest, challenge.nonce_digest,
                    challenge.prospective_project_id::text,
                    challenge.expires_at > clock_timestamp(), challenge.consumed_at IS NOT NULL,
                    idempotency.outcome_kind, idempotency.result_reference
               FROM storyos.create_project_challenges AS challenge
               JOIN storyos.create_project_idempotency AS idempotency
                 USING (user_id, idempotency_key)
              WHERE challenge.user_id = $1::text::uuid
                AND challenge.idempotency_key = $2::text::uuid
              FOR UPDATE OF challenge, idempotency",
            &[&binding.owner_user_id.as_ref(), &binding.idempotency_key],
        )
        .await
        .map_err(create_project_database_error)?
        .ok_or(CreateProjectError::InvalidChallenge)?;
    let exact = row.get::<_, String>(0) == binding.client_session_binding_digest
        && row.get::<_, String>(1) == client_session_generation
        && row.get::<_, String>(2) == binding.client_contract_revision
        && row.get::<_, String>(3) == binding.security_policy_revision
        && row.get::<_, String>(4) == binding.limit_profile_revision
        && row.get::<_, String>(5) == binding.method
        && row.get::<_, String>(6) == binding.route_template
        && row.get::<_, String>(7) == binding.command_schema
        && row.get::<_, String>(8) == binding.command_kind
        && row.get::<_, String>(9) == binding.canonical_command_digest
        && row.get::<_, String>(10) == command.nonce_digest
        && row.get::<_, String>(11) == command.project_scope.project_id.as_ref();
    let unexpired = row.get::<_, bool>(12);
    let consumed = row.get::<_, bool>(13);
    if !exact || (!consumed && !unexpired) {
        return Err(CreateProjectError::InvalidChallenge);
    }
    let outcome = row.get::<_, String>(14);
    let result_reference = row.get::<_, Option<String>>(15);
    if !consumed {
        client
            .execute(
                "UPDATE storyos.create_project_challenges SET consumed_at = clock_timestamp()
                  WHERE user_id = $1::text::uuid AND idempotency_key = $2::text::uuid",
                &[&binding.owner_user_id.as_ref(), &binding.idempotency_key],
            )
            .await
            .map_err(create_project_database_error)?;
        client
            .execute(
                "UPDATE storyos.create_project_idempotency SET outcome_kind = 'in_progress'
                  WHERE user_id = $1::text::uuid AND idempotency_key = $2::text::uuid",
                &[&binding.owner_user_id.as_ref(), &binding.idempotency_key],
            )
            .await
            .map_err(create_project_database_error)?;
        Ok(ProjectCommandChallengeUse::FirstUse)
    } else if outcome == "settled" {
        Ok(ProjectCommandChallengeUse::ExactRetrySettled {
            result_reference: result_reference.ok_or(CreateProjectError::InvalidChallenge)?,
        })
    } else {
        Ok(ProjectCommandChallengeUse::ExactRetryInProgress)
    }
}

async fn persist_create_project(
    client: &tokio_postgres::Client,
    command: &CreateProjectCommand,
) -> Result<CreateProjectSettlement, CreateProjectError> {
    set_challenge_scope_on_client(client, &command.project_scope)
        .await
        .map_err(create_project_challenge_error)?;
    let exists = client
        .query_one(
            "SELECT EXISTS (
               SELECT 1 FROM storyos.projects
                WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
             )",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(create_project_database_error)?
        .get::<_, bool>(0);
    let presence = if exists {
        storyos_core::ProjectPresence::Present
    } else {
        storyos_core::ProjectPresence::Absent
    };
    if storyos_core::create_project(presence) == storyos_core::CreateProjectResult::ExistingProject {
        return Err(CreateProjectError::ExistingProject);
    }
    let client_session_generation = command.client_binding.session_generation.to_string();
    client
        .execute(
            "INSERT INTO storyos.projects (owner_user_id, project_id, title, current_chapter_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3, NULL)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.title,
            ],
        )
        .await
        .map_err(create_project_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.command_idempotency
               (owner_user_id, project_id, command_kind, idempotency_key,
                canonical_command_digest, outcome_kind)
             VALUES ($1::text::uuid, $2::text::uuid, 'createProject', $3::text::uuid, $4,
                     'in_progress')",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.challenge_binding.idempotency_key,
                &command.challenge_binding.canonical_command_digest,
            ],
        )
        .await
        .map_err(create_project_database_error)?;
    client
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
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     NULL, NULL, $5, $6::text::numeric, $7, $8,
                     'explicit_project_command', $9, $10, $11, 'createProject',
                     $12, $13::text::uuid, NULL, NULL, $14::text::uuid, NULL, NULL,
                     '{}'::uuid[], '{}'::text[], NULL, $7, NULL, NULL, NULL,
                     convert_from($15::bytea, 'UTF8')::jsonb)",
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
        .map_err(create_project_database_error)?;
    let receipt_created_at = client
        .query_one(
            "INSERT INTO storyos.domain_receipts
               (owner_user_id, project_id, receipt_id, author_command_admission_id,
                command_id, command_kind, command_digest, idempotency_key, producer_cause,
                expected_heads, prior_heads, resulting_heads, authoritative_revision_ids,
                proposal_revision_ids, authoritative_commit_ids, draft_artifact_refs,
                artifact_lifecycle_event_refs, condition_refs, result_kind, result_payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'createProject', $6, $7::text::uuid,
                     'author_command_admission', '{}'::uuid[], '{}'::uuid[], '{}'::uuid[],
                     '{}'::uuid[], '{}'::uuid[], '{}'::uuid[], '{}'::text[], '{}'::text[],
                     '{}'::text[], 'authoritative_applied', '{}'::jsonb)
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
            ],
        )
        .await
        .map_err(create_project_database_error)?
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
        .map_err(create_project_database_error)?;
    let activity_position = client
        .query_one(
            "INSERT INTO storyos.scope_counters
               (owner_user_id, project_id, project_activity_position)
             VALUES ($1::text::uuid, $2::text::uuid, 1)
             RETURNING project_activity_position::text",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(create_project_database_error)?
        .get::<_, String>(0)
        .parse::<u64>()
        .map_err(|error| CreateProjectError::Unavailable(Box::new(error)))?;
    let activity_event_id = Uuid::now_v7().to_string();
    let payload = serde_json::json!({
        "kind": "project_created",
        "open_kind": "empty",
        "title": command.title,
    })
    .to_string();
    client
        .execute(
            "INSERT INTO storyos.project_activity_event_payloads
               (owner_user_id, project_id, project_activity_position, project_activity_event_id,
                event_kind, receipt_id, receipt_result_kind, payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                     'project_created', $5::text::uuid, 'authoritative_applied', $6::text::jsonb)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &activity_position.to_string(),
                &activity_event_id,
                &command.ids.receipt_id,
                &payload,
            ],
        )
        .await
        .map_err(create_project_database_error)?;
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'createProject' AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(create_project_database_error)?;
    client
        .execute(
            "UPDATE storyos.create_project_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE user_id = $1::text::uuid AND idempotency_key = $2::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.challenge_binding.idempotency_key,
                &command.ids.receipt_id,
            ],
        )
        .await
        .map_err(create_project_database_error)?;
    Ok(CreateProjectSettlement {
        ids: command.ids.clone(),
        title: command.title.clone(),
        receipt_created_at,
        project_activity_position: activity_position,
        project_activity_event_id: activity_event_id,
    })
}

async fn read_create_project_settlement(
    client: &tokio_postgres::Client,
    command: &CreateProjectCommand,
    receipt_id: &str,
) -> Result<CreateProjectSettlement, CreateProjectError> {
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(create_project_database_error)?;
    let result = async {
        set_challenge_scope_on_client(client, &command.project_scope)
            .await
            .map_err(create_project_challenge_error)?;
        let row = client
            .query_opt(
                "SELECT receipt.command_id::text,
                        receipt.author_command_admission_id::text,
                        receipt.receipt_id::text,
                        to_char(receipt.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        project.title,
                        payload.project_activity_position::text,
                        payload.project_activity_event_id::text
                   FROM storyos.domain_receipts AS receipt
                   JOIN storyos.projects AS project
                     ON (project.owner_user_id, project.project_id) =
                        (receipt.owner_user_id, receipt.project_id)
                   JOIN storyos.author_command_admission_settlements AS settlement
                     ON (settlement.owner_user_id, settlement.project_id,
                         settlement.author_command_admission_id, settlement.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id,
                         receipt.author_command_admission_id, receipt.receipt_id)
                   JOIN storyos.create_project_idempotency AS idempotency
                     ON (idempotency.user_id, idempotency.idempotency_key,
                         idempotency.result_reference) =
                        (receipt.owner_user_id, receipt.idempotency_key, receipt.receipt_id::text)
                   JOIN storyos.project_activity_event_payloads AS payload
                     ON (payload.owner_user_id, payload.project_id, payload.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $3::text::uuid
                    AND receipt.command_kind = 'createProject'
                    AND receipt.command_digest = $4
                    AND receipt.idempotency_key = $5::text::uuid
                    AND receipt.result_kind = 'authoritative_applied'
                    AND settlement.settlement_kind = 'receipt_settled'
                    AND idempotency.outcome_kind = 'settled'
                    AND payload.event_kind = 'project_created'",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &receipt_id,
                    &command.challenge_binding.canonical_command_digest,
                    &command.challenge_binding.idempotency_key,
                ],
            )
            .await
            .map_err(create_project_database_error)?
            .ok_or(CreateProjectError::BindingConflict)?;
        Ok(CreateProjectSettlement {
            ids: AuthorCommandAdmissionIds {
                command_id: row.get(0),
                author_command_admission_id: row.get(1),
                receipt_id: row.get(2),
            },
            receipt_created_at: row.get(3),
            title: row.get(4),
            project_activity_position: row
                .get::<_, String>(5)
                .parse::<u64>()
                .map_err(|error| CreateProjectError::Unavailable(Box::new(error)))?,
            project_activity_event_id: row.get(6),
        })
    }
    .await;
    match &result {
        Ok(_) => client
            .batch_execute("COMMIT")
            .await
            .map_err(create_project_database_error)?,
        Err(_) => {
            let _rollback = client.batch_execute("ROLLBACK").await;
        }
    }
    result
}

fn create_project_challenge_error(error: ProjectCommandChallengeError) -> CreateProjectError {
    match error {
        ProjectCommandChallengeError::BindingConflict => CreateProjectError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired => CreateProjectError::InvalidChallenge,
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => {
            CreateProjectError::Unavailable(Box::new(error))
        }
    }
}

fn create_project_database_error(error: tokio_postgres::Error) -> CreateProjectError {
    CreateProjectError::Unavailable(Box::new(error))
}
