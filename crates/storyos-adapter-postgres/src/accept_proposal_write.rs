use storyos_application::{
    AcceptProposalCommand, AcceptProposalError, AcceptProposalSettlement,
    AcceptProposalSettlementEffect, AuthoritativeAppliedIds, ChapterId, Project,
};
use uuid::Uuid;

use super::{LoadedProposal, accept_database_error, accept_parse_error};
use crate::author_edit::{parse_u64, sha256_hex};
use crate::command_response_project::{
    COMMAND_RESPONSE_PROJECT_FORMAT, encode_command_response_project,
};

pub(super) async fn persist_applied(
    client: &tokio_postgres::Client,
    command: &AcceptProposalCommand,
    loaded: &LoadedProposal,
) -> Result<AcceptProposalSettlement, AcceptProposalError> {
    let prior_head_revision_id = command.expected_authoritative_revision_id.clone();
    let counter_row = client
        .query_one(
            "INSERT INTO storyos.scope_counters AS counters
               (owner_user_id, project_id, author_action_sequence,
                authoritative_commit_sequence, project_activity_position)
             VALUES ($1::text::uuid, $2::text::uuid, 1, 1, 1)
             ON CONFLICT (owner_user_id, project_id)
             DO UPDATE SET
               author_action_sequence = counters.author_action_sequence + 1,
               authoritative_commit_sequence = counters.authoritative_commit_sequence + 1,
               project_activity_position = counters.project_activity_position + 1
             RETURNING counters.author_action_sequence::text,
                       counters.authoritative_commit_sequence::text,
                       counters.project_activity_position::text",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(accept_database_error)?;
    let author_action_sequence = parse_u64(counter_row.get(0)).map_err(accept_parse_error)?;
    let authoritative_commit_sequence =
        parse_u64(counter_row.get(1)).map_err(accept_parse_error)?;
    let project_activity_position = parse_u64(counter_row.get(2)).map_err(accept_parse_error)?;
    let ids = AuthoritativeAppliedIds {
        revision_id: Uuid::now_v7().to_string(),
        payload_id: Uuid::now_v7().to_string(),
        authoritative_commit_id: Uuid::now_v7().to_string(),
        project_activity_event_id: Uuid::now_v7().to_string(),
    };
    let accepted_body = loaded.accepted_body(&command.selected_operation_ids)?;
    persist_authority(
        client,
        command,
        &loaded.chapter_id,
        &prior_head_revision_id,
        &ids,
        &accepted_body,
        authoritative_commit_sequence,
    )
    .await?;
    let copied = crate::manuscript_block::copy_or_upgrade_revision_members(
        client,
        command.project_scope.owner_user_id.as_ref(),
        command.project_scope.project_id.as_ref(),
        &loaded.chapter_id,
        &prior_head_revision_id,
        &ids.revision_id,
    )
    .await
    .map_err(accept_database_error)?;
    if copied == 0 {
        return Err(AcceptProposalError::Unavailable(Box::new(
            std::io::Error::other("successor revision members were not copied"),
        )));
    }
    let blocks = crate::manuscript_block::load_revision_blocks(
        client,
        command.project_scope.owner_user_id.as_ref(),
        command.project_scope.project_id.as_ref(),
        &loaded.chapter_id,
        &ids.revision_id,
        &accepted_body,
    )
    .await
    .map_err(accept_database_error)?;
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
    let pending = client
        .query_one(
            "SELECT count(*)::bigint
               FROM storyos.proposal_operations
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid AND resolution = 'pending'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.proposal_id,
            ],
        )
        .await
        .map_err(accept_database_error)?
        .get::<_, i64>(0);
    if pending == 0 {
        client
            .execute(
                "UPDATE storyos.validation_receipts
                    SET reservation_state = 'resolved'
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND validation_receipt_id = $3::text::uuid",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &command.validation_receipt_id,
                ],
            )
            .await
            .map_err(accept_database_error)?;
    }
    let created_at = insert_receipts(
        client,
        command,
        &AcceptanceReceiptInsert {
            result_kind: "authoritative_applied",
            result_payload: "{}",
            prior_head: &prior_head_revision_id,
            resulting_head: &ids.revision_id,
            revision_ids: std::slice::from_ref(&ids.revision_id),
            commit_ids: std::slice::from_ref(&ids.authoritative_commit_id),
            condition_refs: &[],
        },
    )
    .await?;
    client
        .execute(
            "INSERT INTO storyos.author_action_entries
               (owner_user_id, project_id, author_action_sequence, disposition,
                authoritative_commit_id, receipt_id, receipt_result_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, 'forward',
                     $4::text::uuid, $5::text::uuid, 'authoritative_applied')",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &author_action_sequence.to_string(),
                &ids.authoritative_commit_id,
                &command.ids.receipt_id,
            ],
        )
        .await
        .map_err(accept_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.project_activity_events
               (owner_user_id, project_id, project_activity_position,
                project_activity_event_id, event_kind, receipt_id,
                receipt_result_kind, authoritative_commit_id,
                resulting_revision_id, author_action_sequence)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric,
                     $4::text::uuid, 'authoritative_author_edit_applied',
                     $5::text::uuid, 'authoritative_applied', $6::text::uuid,
                     $7::text::uuid, $8::text::numeric)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &project_activity_position.to_string(),
                &ids.project_activity_event_id,
                &command.ids.receipt_id,
                &ids.authoritative_commit_id,
                &ids.revision_id,
                &author_action_sequence.to_string(),
            ],
        )
        .await
        .map_err(accept_database_error)?;
    let base_snapshot_id = Uuid::now_v7().to_string();
    client
        .execute(
            "UPDATE storyos.editor_session_base_snapshots AS snapshot
                SET snapshot_id = $4::text::uuid,
                    authoritative_revision_id = $5::text::uuid,
                    project_activity_position = $6::text::numeric,
                    created_at = clock_timestamp()
               FROM storyos.project_writer_generations AS writer
              WHERE snapshot.owner_user_id = $1::text::uuid
                AND snapshot.project_id = $2::text::uuid
                AND snapshot.editor_session_id = $3::text::uuid
                AND snapshot.authoritative_revision_id = $7::text::uuid
                AND (writer.owner_user_id, writer.project_id,
                     writer.current_editor_session_id) =
                    (snapshot.owner_user_id, snapshot.project_id, snapshot.editor_session_id)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.editor_session_id.as_ref(),
                &base_snapshot_id,
                &ids.revision_id,
                &project_activity_position.to_string(),
                &prior_head_revision_id,
            ],
        )
        .await
        .map_err(accept_database_error)?;
    crate::snapshot::persist_canonical_snapshot(
        client,
        &command.project_scope,
        &base_snapshot_id,
        project_activity_position,
    )
    .await
    .map_err(accept_database_error)?;
    let response_project = settle_idempotency(client, command).await?;
    Ok(AcceptProposalSettlement {
        ids: command.ids.clone(),
        effect: AcceptProposalSettlementEffect::Applied {
            author_action_sequence,
            authoritative_commit_id: ids.authoritative_commit_id,
            revision_id: ids.revision_id,
            body: crate::manuscript_block::display_body_from_stored(&accepted_body, &blocks),
            blocks,
            project_activity_position,
        },
        receipt_created_at: created_at,
        condition_refs: Vec::new(),
        response_project,
    })
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
                acceptance_receipt_id, validation, conflict_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6, $7::text::uuid)",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &command.proposal_id,
                    &command.proposal_revision_id,
                    &command.ids.receipt_id,
                    &result_kind,
                    &condition_refs.first(),
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

async fn persist_authority(
    client: &tokio_postgres::Client,
    command: &AcceptProposalCommand,
    chapter_id: &str,
    current_revision_id: &str,
    ids: &AuthoritativeAppliedIds,
    body: &str,
    authoritative_commit_sequence: u64,
) -> Result<(), AcceptProposalError> {
    client
        .execute(
            "INSERT INTO storyos.authoritative_payloads
               (owner_user_id, project_id, payload_id, canonical_bytes)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, convert_to($4, 'UTF8'))",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &ids.payload_id,
                &body,
            ],
        )
        .await
        .map_err(accept_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.authoritative_revisions
               (owner_user_id, project_id, manuscript_object_id, revision_id, payload_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &chapter_id,
                &ids.revision_id,
                &ids.payload_id,
            ],
        )
        .await
        .map_err(accept_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.authoritative_revision_envelopes
               (owner_user_id, project_id, manuscript_object_id, revision_id, parent_revision_id,
                schema_revision, creator_kind, creator_ref, receipt_id, receipt_result_kind,
                cause_kind, payload_digest)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'storyos.authoritative-revision-envelope.v1',
                     'author_command_admission', $6::text::uuid, $7::text::uuid,
                     'authoritative_applied', 'direct_author_action', $8)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &chapter_id,
                &ids.revision_id,
                &current_revision_id,
                &command.ids.author_command_admission_id,
                &command.ids.receipt_id,
                &sha256_hex(body.as_bytes()),
            ],
        )
        .await
        .map_err(accept_database_error)?;
    let head_updates = client
        .execute(
            "UPDATE storyos.authoritative_heads SET current_revision_id = $4::text::uuid
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND manuscript_object_id = $3::text::uuid AND current_revision_id = $5::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &chapter_id,
                &ids.revision_id,
                &current_revision_id,
            ],
        )
        .await
        .map_err(accept_database_error)?;
    if head_updates != 1 {
        return Err(AcceptProposalError::BindingConflict);
    }
    client
        .execute(
            "INSERT INTO storyos.authoritative_commits
               (owner_user_id, project_id, authoritative_commit_id, authoritative_commit_sequence,
                manuscript_object_id, prior_revision_id, resulting_revision_id,
                author_command_admission_id, receipt_id, receipt_result_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::numeric,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid,
                     $9::text::uuid, 'authoritative_applied')",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &ids.authoritative_commit_id,
                &authoritative_commit_sequence.to_string(),
                &chapter_id,
                &current_revision_id,
                &ids.revision_id,
                &command.ids.author_command_admission_id,
                &command.ids.receipt_id,
            ],
        )
        .await
        .map_err(accept_database_error)?;
    Ok(())
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
