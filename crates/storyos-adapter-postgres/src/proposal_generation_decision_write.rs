use super::{LoadedGeneration, ReceiptBinding, database_error};
use storyos_application::ProposalGenerationDecisionError;

#[allow(clippy::too_many_arguments)]
pub(super) async fn insert_transition(
    client: &tokio_postgres::Client,
    scope: &storyos_application::ProjectScope,
    event_id: &str,
    proposal_id: &str,
    loaded: &LoadedGeneration,
    resulting_generation_id: &str,
    resulting_state: &str,
    resulting_run_id: &str,
    receipt_id: &str,
    sequence: u64,
) -> Result<(), ProposalGenerationDecisionError> {
    client
        .execute(
            "INSERT INTO storyos.proposal_generation_transitions
               (owner_user_id, project_id, transition_id, proposal_id, proposal_revision_id,
                prior_generation_id, resulting_generation_id, prior_generation_state,
                resulting_generation_state, prior_run_id, resulting_run_id,
                preserved_validation, preserved_closure, preserved_operation_resolution,
                receipt_id, author_action_sequence)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::uuid, $8, $9,
                     $10::text::uuid, $11::text::uuid, $12, $13, $14, $15::text::uuid,
                     $16::text::numeric)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &event_id,
                &proposal_id,
                &loaded.revision_id,
                &loaded.generation_id,
                &resulting_generation_id,
                &loaded.generation_state,
                &resulting_state,
                &loaded.source_run_id,
                &resulting_run_id,
                &loaded.validation,
                &loaded.closure,
                &loaded.operation_resolution,
                &receipt_id,
                &sequence.to_string(),
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(())
}

pub(super) async fn insert_receipt(
    client: &tokio_postgres::Client,
    binding: &ReceiptBinding<'_>,
    command_kind: &str,
    result_kind: &str,
    result_payload: &str,
    author_action_sequence: Option<u64>,
) -> Result<String, ProposalGenerationDecisionError> {
    let created_at = client
        .query_one(
            "INSERT INTO storyos.domain_receipts
               (owner_user_id, project_id, receipt_id, author_command_admission_id,
                command_id, command_kind, command_digest, idempotency_key, producer_cause,
                expected_heads, prior_heads, resulting_heads, authoritative_revision_ids,
                proposal_revision_ids, authoritative_commit_ids, draft_artifact_refs,
                artifact_lifecycle_event_refs, condition_refs, result_kind, result_payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6, $7, $8::text::uuid, 'author_command_admission',
                     ARRAY[$9::text::uuid], ARRAY[$9::text::uuid], ARRAY[$9::text::uuid],
                     '{}'::uuid[], '{}'::uuid[], '{}'::uuid[], '{}'::text[], '{}'::text[],
                     '{}'::text[], $10, $11::text::jsonb)
          RETURNING to_char(created_at AT TIME ZONE 'UTC',
                            'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')",
            &[
                &binding.scope.owner_user_id.as_ref(),
                &binding.scope.project_id.as_ref(),
                &binding.ids.receipt_id,
                &binding.ids.author_command_admission_id,
                &binding.ids.command_id,
                &command_kind,
                &binding.challenge.canonical_command_digest,
                &binding.challenge.idempotency_key,
                &binding.head,
                &result_kind,
                &result_payload,
            ],
        )
        .await
        .map_err(database_error)?
        .get(0);
    if let Some(sequence) = author_action_sequence {
        client
            .execute(
                "INSERT INTO storyos.author_action_entries
                   (owner_user_id, project_id, author_action_sequence, disposition,
                    receipt_id, receipt_result_kind)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, 'forward',
                         $4::text::uuid, $5)",
                &[
                    &binding.scope.owner_user_id.as_ref(),
                    &binding.scope.project_id.as_ref(),
                    &sequence.to_string(),
                    &binding.ids.receipt_id,
                    &result_kind,
                ],
            )
            .await
            .map_err(database_error)?;
    }
    client
        .execute(
            "INSERT INTO storyos.author_command_admission_settlements
               (owner_user_id, project_id, author_command_admission_id, settlement_kind, receipt_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 'receipt_settled', $4::text::uuid)",
            &[
                &binding.scope.owner_user_id.as_ref(),
                &binding.scope.project_id.as_ref(),
                &binding.ids.author_command_admission_id,
                &binding.ids.receipt_id,
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(created_at)
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn insert_admission(
    client: &tokio_postgres::Client,
    scope: &storyos_application::ProjectScope,
    client_binding: &storyos_application::EditorClientBinding,
    challenge: &storyos_application::ProjectCommandChallengeBinding,
    ids: &storyos_application::AuthorCommandAdmissionIds,
    editor_session_id: &storyos_application::EditorSessionId,
    correlation_id: &str,
    chapter_id: &str,
    expected_head: &str,
    canonical_command_bytes: &[u8],
    command_kind: &str,
) -> Result<(), ProposalGenerationDecisionError> {
    let session_generation = client_binding.session_generation.to_string();
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
                    'explicit_editor_command', $7, $8, $9, $10,
                    $11, $12::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $13::text::uuid, $14::text::uuid, $15::text::uuid, '{}'::uuid[], '{}'::text[],
                    NULL, session.client_contract_revision, NULL, NULL, NULL,
                    convert_from($16::bytea, 'UTF8')::jsonb
               FROM storyos.editor_sessions AS session
               JOIN storyos.project_writer_generations AS writer
                 ON (writer.owner_user_id, writer.project_id, writer.current_editor_session_id) =
                    (session.owner_user_id, session.project_id, session.editor_session_id)
                AND writer.writer_generation = (
                  SELECT max(current_writer.writer_generation)
                    FROM storyos.project_writer_generations AS current_writer
                   WHERE current_writer.owner_user_id = session.owner_user_id
                     AND current_writer.project_id = session.project_id
                )
               JOIN storyos.project_command_challenges AS challenge
                 ON (challenge.owner_user_id, challenge.project_id, challenge.command_kind,
                     challenge.idempotency_key) =
                    (session.owner_user_id, session.project_id, $10, $12::text::uuid)
              WHERE session.owner_user_id = $1::text::uuid
                AND session.project_id = $2::text::uuid
                AND session.editor_session_id = $5::text::uuid
                AND session.client_session_binding_ref = $17
                AND session.client_session_generation = $6::text::numeric
                AND session.client_contract_revision = $18
                AND session.security_policy_revision = $19
                AND challenge.consumed_at IS NOT NULL",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &ids.author_command_admission_id,
                &ids.command_id,
                &editor_session_id.as_ref(),
                &session_generation,
                &challenge.method,
                &challenge.route_template,
                &challenge.command_schema,
                &command_kind,
                &challenge.canonical_command_digest,
                &challenge.idempotency_key,
                &correlation_id,
                &chapter_id,
                &expected_head,
                &canonical_command_bytes,
                &client_binding.binding_ref,
                &client_binding.client_contract_revision,
                &client_binding.security_policy_revision,
            ],
        )
        .await
        .map_err(database_error)?;
    if inserted != 1 {
        return Err(ProposalGenerationDecisionError::InvalidChallenge);
    }
    Ok(())
}
