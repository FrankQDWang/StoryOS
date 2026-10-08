use storyos_application::{
    AcceptProposalCommand, AcceptProposalError, AcceptProposalSettlement,
    AcceptProposalSettlementEffect, ChapterId, Project, ProjectCommandError,
};
use uuid::Uuid;

use super::{LoadedProposal, accept_database_error};
use crate::command_response_project::{
    COMMAND_RESPONSE_PROJECT_FORMAT, encode_command_response_project,
};
use crate::command_sequence::{
    ActionDisposition, AuthoritativeRevision, RevisionMembers, RevisionWrite, SettlementProfile,
    write_revision,
};

pub(super) async fn persist_applied(
    client: &tokio_postgres::Client,
    command: &AcceptProposalCommand,
    loaded: &LoadedProposal,
) -> Result<AcceptProposalSettlement, AcceptProposalError> {
    let prior_head_revision_id = command.expected_authoritative_revision_id.clone();
    let sequences = AuthoritativeRevision::allocate(client, &command.project_scope)
        .await
        .map_err(accept_sequence_error)?;
    let accepted_body = loaded.accepted_body(&command.selected_operation_ids)?;
    let updated = client
        .execute(
            "UPDATE storyos.proposal_operations
                SET resolution = 'applied', reservation_state = 'resolved'
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid
                AND operation_id = ANY($4::text[]::uuid[])
                AND resolution = 'pending'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.proposal_id,
                &command.selected_operation_ids,
            ],
        )
        .await
        .map_err(accept_database_error)?;
    if updated != u64::try_from(command.selected_operation_ids.len()).unwrap_or(0) {
        return Err(AcceptProposalError::BindingConflict);
    }
    let created_at = insert_receipts(
        client,
        command,
        &AcceptanceReceiptInsert {
            result_kind: "authoritative_applied",
            result_payload: "{}",
            prior_head: &prior_head_revision_id,
            resulting_head: &sequences.ids.revision_id,
            revision_ids: std::slice::from_ref(&sequences.ids.revision_id),
            commit_ids: &AuthoritativeRevision::commit_ids(&sequences),
            condition_refs: &[],
        },
    )
    .await?;
    let applied = write_revision(
        client,
        &command.project_scope,
        &command.ids,
        sequences,
        RevisionWrite {
            effect: (),
            chapter_id: loaded.chapter_id.clone(),
            prior_revision_id: prior_head_revision_id,
            payload: accepted_body,
            members: RevisionMembers::CopyFrom(command.expected_authoritative_revision_id.clone()),
            disposition: ActionDisposition::Forward,
            editor_session_id: command.editor_session_id.as_ref().to_owned(),
        },
    )
    .await
    .map_err(accept_sequence_error)?;
    let response_project = settle_idempotency(client, command).await?;
    Ok(AcceptProposalSettlement {
        ids: command.ids.clone(),
        effect: AcceptProposalSettlementEffect::Applied {
            author_action_sequence: applied.author_action_sequence,
            authoritative_commit_id: applied.ids.authoritative_commit_id,
            revision_id: applied.ids.revision_id,
            body: applied.body,
            blocks: applied.blocks,
            project_activity_position: applied.project_activity_position,
        },
        receipt_created_at: created_at,
        condition_refs: Vec::new(),
        response_project,
    })
}

/// The Acceptance error of a profile write.
fn accept_sequence_error(error: ProjectCommandError) -> AcceptProposalError {
    match error {
        ProjectCommandError::BindingConflict | ProjectCommandError::WriterIneligible => {
            AcceptProposalError::BindingConflict
        }
        ProjectCommandError::HistoricalAcknowledgementUnavailable => {
            AcceptProposalError::HistoricalAcknowledgementUnavailable
        }
        ProjectCommandError::InvalidChallenge => AcceptProposalError::InvalidChallenge,
        ProjectCommandError::MissingProject => AcceptProposalError::MissingProject,
        ProjectCommandError::Unavailable(source) => AcceptProposalError::Unavailable(source),
    }
}

pub(super) async fn persist_zero(
    client: &tokio_postgres::Client,
    command: &AcceptProposalCommand,
    result_kind: &str,
    reason: &str,
    effect: AcceptProposalSettlementEffect,
) -> Result<AcceptProposalSettlement, AcceptProposalError> {
    let head = command.expected_authoritative_revision_id.clone();
    let result_payload = serde_json::json!({ "reason": reason }).to_string();
    let condition_refs = if matches!(effect, AcceptProposalSettlementEffect::Conflicted { .. }) {
        vec![Uuid::now_v7().to_string()]
    } else {
        Vec::new()
    };
    let created_at = insert_receipts(
        client,
        command,
        &AcceptanceReceiptInsert {
            result_kind,
            result_payload: &result_payload,
            prior_head: &head,
            resulting_head: &head,
            revision_ids: &[],
            commit_ids: &[],
            condition_refs: &condition_refs,
        },
    )
    .await?;
    if matches!(
        effect,
        AcceptProposalSettlementEffect::Invalid { .. }
            | AcceptProposalSettlementEffect::Conflicted { .. }
    ) {
        client
            .execute(
                "INSERT INTO storyos.proposal_validation_conditions
               (owner_user_id, project_id, proposal_id, proposal_revision_id,
                acceptance_receipt_id, validation, conflict_id, condition_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6, $7::text::uuid, $8)",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &command.proposal_id,
                    &command.proposal_revision_id,
                    &command.ids.receipt_id,
                    &result_kind,
                    &condition_refs.first(),
                    &if result_kind == "conflicted" {
                        Some("proposal_conflict")
                    } else {
                        None
                    },
                ],
            )
            .await
            .map_err(accept_database_error)?;
    }
    let response_project = settle_idempotency(client, command).await?;
    Ok(AcceptProposalSettlement {
        ids: command.ids.clone(),
        effect,
        receipt_created_at: created_at,
        condition_refs,
        response_project,
    })
}

struct AcceptanceReceiptInsert<'a> {
    result_kind: &'a str,
    result_payload: &'a str,
    prior_head: &'a str,
    resulting_head: &'a str,
    revision_ids: &'a [String],
    commit_ids: &'a [String],
    condition_refs: &'a [String],
}

async fn insert_receipts(
    client: &tokio_postgres::Client,
    command: &AcceptProposalCommand,
    insert: &AcceptanceReceiptInsert<'_>,
) -> Result<String, AcceptProposalError> {
    let created_at = client
        .query_one(
            "INSERT INTO storyos.domain_receipts
               (owner_user_id, project_id, receipt_id, author_command_admission_id,
                command_id, command_kind, command_digest, idempotency_key, producer_cause,
                expected_heads, prior_heads, resulting_heads, authoritative_revision_ids,
                proposal_revision_ids, authoritative_commit_ids, draft_artifact_refs,
                artifact_lifecycle_event_refs, condition_refs, result_kind, result_payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'acceptProposal', $6, $7::text::uuid,
                     'author_command_admission', ARRAY[$8::text::uuid], ARRAY[$9::text::uuid],
                     ARRAY[$10::text::uuid], $11::text[]::uuid[], '{}'::uuid[],
                     $12::text[]::uuid[], '{}'::text[], '{}'::text[], $15::text[],
                     $13, $14::text::jsonb)
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
                &insert.prior_head,
                &insert.resulting_head,
                &insert.revision_ids,
                &insert.commit_ids,
                &insert.result_kind,
                &insert.result_payload,
                &insert.condition_refs,
            ],
        )
        .await
        .map_err(accept_database_error)?
        .get::<_, String>(0);
    client
        .execute(
            "INSERT INTO storyos.acceptance_receipts
               (owner_user_id, project_id, acceptance_receipt_id, proposal_id,
                proposal_revision_id, validation_receipt_id, selected_operation_ids, result)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text[]::uuid[], $8)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.proposal_id,
                &command.proposal_revision_id,
                &command.validation_receipt_id,
                &command.selected_operation_ids,
                &insert.result_kind,
            ],
        )
        .await
        .map_err(accept_database_error)?;
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
        .map_err(accept_database_error)?;
    Ok(created_at)
}

pub(super) async fn insert_accept_admission(
    client: &tokio_postgres::Client,
    command: &AcceptProposalCommand,
    chapter_id: &str,
) -> Result<(), AcceptProposalError> {
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
                    'explicit_editor_command', $7, $8, $9, 'acceptProposal',
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
                     'acceptProposal', $11::text::uuid)
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
        .map_err(accept_database_error)?;
    if inserted != 1 {
        let session = client.query_opt(
            "SELECT client_session_binding_ref = $4 AND client_session_generation = $5::text::numeric
                    AND client_contract_revision = $6 AND security_policy_revision = $7
             FROM storyos.editor_sessions WHERE owner_user_id = $1::text::uuid
               AND project_id = $2::text::uuid AND editor_session_id = $3::text::uuid",
            &[&command.project_scope.owner_user_id.as_ref(), &command.project_scope.project_id.as_ref(),
              &command.editor_session_id.as_ref(), &command.client_binding.binding_ref, &client_session_generation,
              &command.client_binding.client_contract_revision, &command.client_binding.security_policy_revision],
        ).await.map_err(accept_database_error)?;
        let Some(session) = session else {
            return Err(AcceptProposalError::InvalidChallenge);
        };
        let reason = if session.get::<_, bool>(0) {
            storyos_application::AcceptanceRefusalReason::StaleWriter
        } else {
            storyos_application::AcceptanceRefusalReason::SessionChanged
        };
        return Err(AcceptProposalError::PreAdmissionRefused { reason });
    }
    Ok(())
}

async fn settle_idempotency(
    client: &tokio_postgres::Client,
    command: &AcceptProposalCommand,
) -> Result<Project, AcceptProposalError> {
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
        .map_err(accept_database_error)?;
    let Some(row) = row else {
        return Err(AcceptProposalError::MissingProject);
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
                AND command_kind = 'acceptProposal'
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
        .map_err(accept_database_error)?;
    Ok(response_project)
}
