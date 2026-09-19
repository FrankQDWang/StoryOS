use storyos_application::{
    ChapterId, Project, ReopenRejectedOperationsCommand, ReopenRejectedOperationsError,
    ReopenRejectedOperationsSettlement, ReopenRejectedOperationsSettlementEffect,
};
use uuid::Uuid;

use super::{LoadedProposal, reopen_database_error, reopen_parse_error};
use crate::author_edit::parse_u64;
use crate::command_response_project::{
    COMMAND_RESPONSE_PROJECT_FORMAT, encode_command_response_project,
};

pub(super) async fn persist_resolved(
    client: &tokio_postgres::Client,
    command: &ReopenRejectedOperationsCommand,
    loaded: &LoadedProposal,
) -> Result<ReopenRejectedOperationsSettlement, ReopenRejectedOperationsError> {
    let counter_row = client
        .query_one(
            "UPDATE storyos.scope_counters
                SET author_action_sequence = author_action_sequence + 1
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
          RETURNING author_action_sequence::text",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(reopen_database_error)?;
    let author_action_sequence = parse_u64(counter_row.get(0)).map_err(reopen_parse_error)?;
    let resulting_proposal_revision_id = Uuid::now_v7().to_string();
    client
        .execute(
            "INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id, parent_revision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5,
                     'pending', $6, $7, $8::text::uuid, $9::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.proposal_id,
                &resulting_proposal_revision_id,
                &loaded.generation,
                &loaded.closure,
                &loaded.candidate_text,
                &loaded.base_authoritative_revision_id,
                &loaded.current_revision_id,
            ],
        )
        .await
        .map_err(reopen_database_error)?;
    let head_updates = client
        .execute(
            "UPDATE storyos.proposal_heads
                SET current_revision_id = $4::text::uuid
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid AND current_revision_id = $5::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.proposal_id,
                &resulting_proposal_revision_id,
                &loaded.current_revision_id,
            ],
        )
        .await
        .map_err(reopen_database_error)?;
    if head_updates != 1 {
        return Err(ReopenRejectedOperationsError::BindingConflict);
    }
    let updated = client
        .execute(
            "UPDATE storyos.proposal_operations
                SET resolution = 'pending', reservation_state = 'unresolved'
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid AND operation_id = $4::text::uuid
                AND resolution = 'rejected'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.proposal_id,
                &command.selected_rejected_operation_id,
            ],
        )
        .await
        .map_err(reopen_database_error)?;
    if updated != 1 {
        return Err(ReopenRejectedOperationsError::BindingConflict);
    }
    let state_event_id = Uuid::now_v7().to_string();
    let created_at = insert_receipts(
        client,
        command,
        "proposal_revised",
        r#"{"transition":"reopen_rejected"}"#,
        Some(&resulting_proposal_revision_id),
        Some(&state_event_id),
        Some(author_action_sequence),
    )
    .await?;
    client
        .execute(
            "INSERT INTO storyos.author_action_entries
               (owner_user_id, project_id, author_action_sequence, disposition,
                receipt_id, receipt_result_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, 'forward',
                     $4::text::uuid, 'proposal_revised')",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &author_action_sequence.to_string(),
                &command.ids.receipt_id,
            ],
        )
        .await
        .map_err(reopen_database_error)?;
    let response_project = settle_idempotency(client, command).await?;
    Ok(ReopenRejectedOperationsSettlement {
        ids: command.ids.clone(),
        effect: ReopenRejectedOperationsSettlementEffect::Resolved {
            author_action_sequence,
            operation_id: command.selected_rejected_operation_id.clone(),
            rejection_event_id: command.rejection_event_id.clone(),
            resulting_proposal_revision_id,
            preserved_generation: loaded.generation.clone(),
            preserved_closure: loaded.closure.clone(),
            state_event_id,
        },
        receipt_created_at: created_at,
        response_project,
    })
}

pub(super) async fn persist_zero(
    client: &tokio_postgres::Client,
    command: &ReopenRejectedOperationsCommand,
    result_kind: &str,
    reason: &str,
    effect: ReopenRejectedOperationsSettlementEffect,
) -> Result<ReopenRejectedOperationsSettlement, ReopenRejectedOperationsError> {
    let result_payload = serde_json::json!({ "reason": reason }).to_string();
    let created_at = insert_receipts(
        client,
        command,
        result_kind,
        &result_payload,
        /*resulting_proposal_revision_id*/ None,
        /*state_event_id*/ None,
        /*author_action_sequence*/ None,
    )
    .await?;
    let response_project = settle_idempotency(client, command).await?;
    Ok(ReopenRejectedOperationsSettlement {
        ids: command.ids.clone(),
        effect,
        receipt_created_at: created_at,
        response_project,
    })
}

async fn insert_receipts(
    client: &tokio_postgres::Client,
    command: &ReopenRejectedOperationsCommand,
    result_kind: &str,
    result_payload: &str,
    resulting_proposal_revision_id: Option<&str>,
    state_event_id: Option<&str>,
    author_action_sequence: Option<u64>,
) -> Result<String, ReopenRejectedOperationsError> {
    let created_at = client
        .query_one(
            "INSERT INTO storyos.domain_receipts
               (owner_user_id, project_id, receipt_id, author_command_admission_id,
                command_id, command_kind, command_digest, idempotency_key, producer_cause,
                expected_heads, prior_heads, resulting_heads, authoritative_revision_ids,
                proposal_revision_ids, authoritative_commit_ids, draft_artifact_refs,
                artifact_lifecycle_event_refs, condition_refs, result_kind, result_payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'reopenRejectedOperations', $6, $7::text::uuid,
                     'author_command_admission', ARRAY[$8::text::uuid], ARRAY[$8::text::uuid],
                     ARRAY[$8::text::uuid], '{}'::uuid[],
                     CASE WHEN $9::text IS NULL THEN '{}'::uuid[] ELSE ARRAY[$9::text::uuid] END,
                     '{}'::uuid[], '{}'::text[], '{}'::text[], '{}'::text[],
                     $10, $11::text::jsonb)
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
                &command.expected_authoritative_revision_id,
                &resulting_proposal_revision_id,
                &result_kind,
                &result_payload,
            ],
        )
        .await
        .map_err(reopen_database_error)?
        .get::<_, String>(0);
    if let (Some(event_id), Some(revision_id), Some(sequence)) = (
        state_event_id,
        resulting_proposal_revision_id,
        author_action_sequence,
    ) {
        client
            .execute(
                "INSERT INTO storyos.proposal_operation_reopenings
                   (owner_user_id, project_id, reopen_event_id, proposal_id,
                    source_proposal_revision_id, resulting_proposal_revision_id, operation_id,
                    rejection_event_id, prior_resolution, resulting_resolution,
                    reopen_receipt_id, author_action_sequence)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                         'rejected', 'pending', $9::text::uuid, $10::text::numeric)",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &event_id,
                    &command.proposal_id,
                    &command.proposal_revision_id,
                    &revision_id,
                    &command.selected_rejected_operation_id,
                    &command.rejection_event_id,
                    &command.ids.receipt_id,
                    &sequence.to_string(),
                ],
            )
            .await
            .map_err(reopen_database_error)?;
    }
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
        .map_err(reopen_database_error)?;
    Ok(created_at)
}

pub(super) async fn insert_reopen_admission(
    client: &tokio_postgres::Client,
    command: &ReopenRejectedOperationsCommand,
    chapter_id: &str,
) -> Result<(), ReopenRejectedOperationsError> {
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
                    'explicit_editor_command', $7, $8, $9, 'reopenRejectedOperations',
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
                     'reopenRejectedOperations', $11::text::uuid)
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
                &chapter_id,
                &command.expected_authoritative_revision_id,
                &command.canonical_command_bytes.as_slice(),
                &command.client_binding.binding_ref,
                &command.client_binding.client_contract_revision,
                &command.client_binding.security_policy_revision,
            ],
        )
        .await
        .map_err(reopen_database_error)?;
    if inserted != 1 {
        return Err(ReopenRejectedOperationsError::InvalidChallenge);
    }
    Ok(())
}

async fn settle_idempotency(
    client: &tokio_postgres::Client,
    command: &ReopenRejectedOperationsCommand,
) -> Result<Project, ReopenRejectedOperationsError> {
    let row = client
        .query_opt(
            "SELECT title, current_chapter_id::text
               FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(reopen_database_error)?;
    let Some(row) = row else {
        return Err(ReopenRejectedOperationsError::MissingProject);
    };
    let response_project = Project {
        project_id: command.project_scope.project_id.clone(),
        title: row.get(0),
        current_chapter_id: row.get::<_, Option<String>>(1).map(ChapterId::new),
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
                AND command_kind = 'reopenRejectedOperations'
                AND idempotency_key = $4::text::uuid",
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
        .map_err(reopen_database_error)?;
    Ok(response_project)
}
