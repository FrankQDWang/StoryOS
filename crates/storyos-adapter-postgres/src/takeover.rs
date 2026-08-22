use serde_json::Value;
use storyos_application::{
    AuthorCommandAdmissionIds, ProjectCommandChallengeError, ProjectCommandChallengeTransaction,
    ProjectCommandChallengeUse, TakeOverProjectWriterCommand, TakeOverProjectWriterEffect,
    TakeOverProjectWriterError, TakeOverProjectWriterSettlement, TakeOverProjectWriterStore,
};
use uuid::Uuid;

use super::*;

impl TakeOverProjectWriterStore for PostgresProjectReader {
    async fn take_over_project_writer(
        &self,
        command: &TakeOverProjectWriterCommand,
    ) -> Result<TakeOverProjectWriterSettlement, TakeOverProjectWriterError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(takeover_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(takeover_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(takeover_challenge_error)?;
                read_takeover_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(takeover_challenge_error)?;
                Err(TakeOverProjectWriterError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                let settlement = persist_takeover_settlement(&transaction.client, command).await?;
                transaction
                    .commit()
                    .await
                    .map_err(takeover_challenge_error)?;
                Ok(settlement)
            }
        }
    }
}

async fn persist_takeover_settlement(
    client: &tokio_postgres::Client,
    command: &TakeOverProjectWriterCommand,
) -> Result<TakeOverProjectWriterSettlement, TakeOverProjectWriterError> {
    let observed_generation = command.observed_writer_generation.to_string();
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
                    session.editor_session_id, $6::text::numeric,
                    session.client_session_binding_ref, $7::text::numeric,
                    session.client_contract_revision, session.security_policy_revision,
                    'explicit_editor_command', $8, $9, $10, 'takeOverProjectWriter',
                    $11, $12::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $13::text::uuid, NULL, NULL, '{}'::uuid[], '{}'::text[], NULL, $14,
                    NULL, NULL, NULL, convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.editor_sessions AS session
               JOIN LATERAL (
                 SELECT writer.writer_generation, writer.current_editor_session_id
                   FROM storyos.project_writer_generations AS writer
                  WHERE writer.owner_user_id = session.owner_user_id
                    AND writer.project_id = session.project_id
                  ORDER BY writer.writer_generation DESC
                  LIMIT 1
               ) AS current_writer
                 ON current_writer.writer_generation = $6::text::numeric
                AND current_writer.current_editor_session_id <> session.editor_session_id
               JOIN storyos.project_command_challenges AS challenge
                 ON (challenge.owner_user_id, challenge.project_id,
                     challenge.command_kind, challenge.idempotency_key) =
                    (session.owner_user_id, session.project_id,
                     'takeOverProjectWriter', $12::text::uuid)
              WHERE session.owner_user_id = $1::text::uuid
                AND session.project_id = $2::text::uuid
                AND session.editor_session_id = $5::text::uuid
                AND session.client_session_binding_ref = $16
                AND session.client_session_generation = $7::text::numeric
                AND session.client_contract_revision = $17
                AND session.security_policy_revision = $18
                AND challenge.consumed_at IS NOT NULL",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.author_command_admission_id,
                &command.ids.command_id,
                &command.editor_session_id.as_ref(),
                &observed_generation,
                &client_session_generation,
                &command.challenge_binding.method,
                &command.challenge_binding.route_template,
                &command.challenge_binding.command_schema,
                &command.challenge_binding.canonical_command_digest,
                &command.challenge_binding.idempotency_key,
                &command.correlation_id,
                &command.editor_contract_revision,
                &command.canonical_command_bytes.as_slice(),
                &command.client_binding.binding_ref,
                &command.client_binding.client_contract_revision,
                &command.client_binding.security_policy_revision,
            ],
        )
        .await
        .map_err(takeover_database_error)?;
    if inserted != 1 {
        return Err(TakeOverProjectWriterError::BindingConflict);
    }

    // Runtime may INSERT a new generation but cannot UPDATE historical writer rows.
    let writer = client
        .query_opt(
            "SELECT writer.writer_generation::text, writer.current_editor_session_id::text
               FROM storyos.project_writer_generations AS writer
              WHERE writer.owner_user_id = $1::text::uuid
                AND writer.project_id = $2::text::uuid
              ORDER BY writer.writer_generation DESC
              LIMIT 1",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(takeover_database_error)?
        .ok_or(TakeOverProjectWriterError::BindingConflict)?;
    let prior_generation = parse_takeover_u64(writer.get(0))?;
    let prior_session: String = writer.get(1);
    if prior_generation != command.observed_writer_generation
        || prior_session == command.editor_session_id.as_ref()
    {
        return Err(TakeOverProjectWriterError::BindingConflict);
    }
    let resulting_generation = prior_generation
        .checked_add(1)
        .ok_or(TakeOverProjectWriterError::BindingConflict)?;
    let resulting_generation_text = resulting_generation.to_string();

    let head = client
        .query_one(
            "SELECT head.current_revision_id::text
               FROM storyos.projects AS project
               JOIN storyos.authoritative_heads AS head
                 ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                    (project.owner_user_id, project.project_id, project.current_chapter_id)
              WHERE project.owner_user_id = $1::text::uuid
                AND project.project_id = $2::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(takeover_database_error)?;
    let current_head: String = head.get(0);

    let generation_inserts = client
        .execute(
            "INSERT INTO storyos.project_writer_generations
               (owner_user_id, project_id, writer_generation, current_editor_session_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &resulting_generation_text,
                &command.editor_session_id.as_ref(),
            ],
        )
        .await
        .map_err(takeover_database_error)?;
    if generation_inserts != 1 {
        return Err(TakeOverProjectWriterError::BindingConflict);
    }

    let activity_position = parse_takeover_u64(
        client
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
            .map_err(takeover_database_error)?
            .get(0),
    )?;
    let snapshot_id = Uuid::now_v7().to_string();
    crate::snapshot::persist_canonical_snapshot(
        client,
        &command.project_scope,
        &snapshot_id,
        activity_position,
    )
    .await
    .map_err(takeover_database_error)?;

    let result_payload = serde_json::json!({"reason": "writer_takeover_applied"}).to_string();
    let receipt_created_at = client
        .query_one(
            "INSERT INTO storyos.domain_receipts
               (owner_user_id, project_id, receipt_id, author_command_admission_id,
                command_id, command_kind, command_digest, idempotency_key, producer_cause,
                expected_heads, prior_heads, resulting_heads, authoritative_revision_ids,
                proposal_revision_ids, authoritative_commit_ids, draft_artifact_refs,
                artifact_lifecycle_event_refs, condition_refs, result_kind, result_payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'takeOverProjectWriter', $6, $7::text::uuid,
                     'author_command_admission', ARRAY[$8::text::uuid], ARRAY[$8::text::uuid],
                     ARRAY[$8::text::uuid], ARRAY[]::uuid[], ARRAY[]::uuid[], ARRAY[]::uuid[],
                     ARRAY[]::text[], ARRAY[]::text[], ARRAY[]::text[],
                     'no_effect', $9::text::jsonb)
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
                &current_head,
                &result_payload,
            ],
        )
        .await
        .map_err(takeover_database_error)?
        .get::<_, String>(0);
    let settlement_inserts = client
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
        .map_err(takeover_database_error)?;
    if settlement_inserts != 1 {
        return Err(TakeOverProjectWriterError::BindingConflict);
    }

    let activity_event_id = Uuid::now_v7().to_string();
    let activity_position_text = activity_position.to_string();
    let payload = serde_json::json!({
        "kind": "takeover_applied",
        "prior_editor_session_id": prior_session,
        "prior_writer_generation": observed_generation,
        "resulting_editor_session_id": command.editor_session_id.as_ref(),
        "resulting_writer_generation": resulting_generation_text,
        "resulting_snapshot_id": snapshot_id,
        "resulting_snapshot_activity_position": activity_position_text,
        "resulting_heads": [current_head],
    })
    .to_string();
    client
        .execute(
            "INSERT INTO storyos.project_activity_event_payloads
               (owner_user_id, project_id, project_activity_position, project_activity_event_id,
                event_kind, receipt_id, receipt_result_kind, payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                     'writer_takeover_applied', $5::text::uuid, 'no_effect', $6::text::jsonb)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &activity_position_text,
                &activity_event_id,
                &command.ids.receipt_id,
                &payload,
            ],
        )
        .await
        .map_err(takeover_database_error)?;

    let idempotency_updates = client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'takeOverProjectWriter' AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(takeover_database_error)?;
    if idempotency_updates != 1 {
        return Err(TakeOverProjectWriterError::BindingConflict);
    }
    let heads = vec![current_head];
    Ok(TakeOverProjectWriterSettlement {
        ids: command.ids.clone(),
        receipt_created_at,
        expected_heads: heads.clone(),
        prior_heads: heads.clone(),
        resulting_heads: heads,
        effect: TakeOverProjectWriterEffect::TakeoverApplied {
            prior_editor_session_id: prior_session,
            prior_writer_generation: prior_generation,
            resulting_editor_session_id: command.editor_session_id.as_ref().to_owned(),
            resulting_writer_generation: resulting_generation,
            resulting_snapshot_id: snapshot_id,
            resulting_snapshot_activity_position: activity_position,
        },
    })
}

async fn read_takeover_settlement(
    store: &PostgresProjectReader,
    command: &TakeOverProjectWriterCommand,
    receipt_id: &str,
) -> Result<TakeOverProjectWriterSettlement, TakeOverProjectWriterError> {
    let client = store
        .connect_challenge()
        .await
        .map_err(takeover_challenge_error)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(takeover_database_error)?;
    let result = async {
        set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(takeover_challenge_error)?;
        let row = client
            .query_opt(
                "SELECT receipt.command_id::text,
                        receipt.author_command_admission_id::text,
                        receipt.receipt_id::text,
                        to_char(receipt.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        receipt.expected_heads[1]::text,
                        receipt.prior_heads[1]::text,
                        receipt.resulting_heads[1]::text,
                        payload.payload::text
                   FROM storyos.domain_receipts AS receipt
                   JOIN storyos.author_command_admissions AS admission
                     ON (admission.owner_user_id, admission.project_id,
                         admission.author_command_admission_id, admission.command_id,
                         admission.command_kind, admission.canonical_command_digest,
                         admission.idempotency_key) =
                        (receipt.owner_user_id, receipt.project_id,
                         receipt.author_command_admission_id, receipt.command_id,
                         receipt.command_kind, receipt.command_digest,
                         receipt.idempotency_key)
                   JOIN storyos.author_command_admission_settlements AS settlement
                     ON (settlement.owner_user_id, settlement.project_id,
                         settlement.author_command_admission_id, settlement.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id,
                         receipt.author_command_admission_id, receipt.receipt_id)
                   JOIN storyos.command_idempotency AS idempotency
                     ON (idempotency.owner_user_id, idempotency.project_id,
                         idempotency.command_kind, idempotency.idempotency_key,
                         idempotency.canonical_command_digest) =
                        (receipt.owner_user_id, receipt.project_id,
                         receipt.command_kind, receipt.idempotency_key,
                         receipt.command_digest)
                   JOIN storyos.project_activity_event_payloads AS payload
                     ON (payload.owner_user_id, payload.project_id, payload.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $3::text::uuid
                    AND receipt.command_kind = 'takeOverProjectWriter'
                    AND receipt.command_digest = $4
                    AND receipt.idempotency_key = $5::text::uuid
                    AND receipt.result_kind = 'no_effect'
                    AND receipt.result_payload->>'reason' = 'writer_takeover_applied'
                    AND settlement.settlement_kind = 'receipt_settled'
                    AND idempotency.outcome_kind = 'settled'
                    AND idempotency.result_reference = receipt.receipt_id::text
                    AND payload.event_kind = 'writer_takeover_applied'",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &receipt_id,
                    &command.challenge_binding.canonical_command_digest,
                    &command.challenge_binding.idempotency_key,
                ],
            )
            .await
            .map_err(takeover_database_error)?;
        let row = row.ok_or(TakeOverProjectWriterError::BindingConflict)?;
        let payload: Value = serde_json::from_str(&row.get::<_, String>(7))
            .map_err(|error| TakeOverProjectWriterError::Unavailable(Box::new(error)))?;
        let heads = vec![row.get::<_, String>(6)];
        Ok(TakeOverProjectWriterSettlement {
            ids: AuthorCommandAdmissionIds {
                command_id: row.get(0),
                author_command_admission_id: row.get(1),
                receipt_id: row.get(2),
            },
            receipt_created_at: row.get(3),
            expected_heads: vec![row.get(4)],
            prior_heads: vec![row.get(5)],
            resulting_heads: heads.clone(),
            effect: TakeOverProjectWriterEffect::TakeoverApplied {
                prior_editor_session_id: required_payload_string(
                    &payload,
                    "prior_editor_session_id",
                )?,
                prior_writer_generation: parse_takeover_u64(required_payload_string(
                    &payload,
                    "prior_writer_generation",
                )?)?,
                resulting_editor_session_id: required_payload_string(
                    &payload,
                    "resulting_editor_session_id",
                )?,
                resulting_writer_generation: parse_takeover_u64(required_payload_string(
                    &payload,
                    "resulting_writer_generation",
                )?)?,
                resulting_snapshot_id: required_payload_string(&payload, "resulting_snapshot_id")?,
                resulting_snapshot_activity_position: parse_takeover_u64(required_payload_string(
                    &payload,
                    "resulting_snapshot_activity_position",
                )?)?,
            },
        })
    }
    .await;
    match &result {
        Ok(_) => {
            client
                .batch_execute("COMMIT")
                .await
                .map_err(takeover_database_error)?;
        }
        Err(_) => {
            client
                .batch_execute("ROLLBACK")
                .await
                .map_err(takeover_database_error)?;
        }
    }
    result
}

fn required_payload_string(
    payload: &Value,
    key: &str,
) -> Result<String, TakeOverProjectWriterError> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(TakeOverProjectWriterError::BindingConflict)
}

fn parse_takeover_u64(value: String) -> Result<u64, TakeOverProjectWriterError> {
    value
        .parse::<u64>()
        .map_err(|source| TakeOverProjectWriterError::Unavailable(Box::new(source)))
}

fn takeover_challenge_error(error: ProjectCommandChallengeError) -> TakeOverProjectWriterError {
    match error {
        ProjectCommandChallengeError::BindingConflict => {
            TakeOverProjectWriterError::BindingConflict
        }
        ProjectCommandChallengeError::InvalidOrExpired
        | ProjectCommandChallengeError::RateLimited { .. } => {
            TakeOverProjectWriterError::InvalidChallenge
        }
        ProjectCommandChallengeError::Unavailable(source) => {
            TakeOverProjectWriterError::Unavailable(source)
        }
    }
}

fn takeover_database_error(error: tokio_postgres::Error) -> TakeOverProjectWriterError {
    TakeOverProjectWriterError::Unavailable(Box::new(error))
}
