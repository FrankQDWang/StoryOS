use storyos_application::{
    ApplyAuthorEditCommand, AuthorEditError, AuthorEditSettlement, AuthorEditSettlementEffect,
};
use storyos_core::{
    ApplyAuthorEditResult, AuthorEditConflict, AuthorEditNoEffect, AuthorEditRefusal,
};
use tokio_postgres::Client;

use super::author_edit::{author_edit_database_error, parse_u64, sha256_hex};

pub(super) async fn persist_author_edit_settlement(
    client: &Client,
    command: &ApplyAuthorEditCommand,
    current_revision_id: &str,
    core_result: ApplyAuthorEditResult,
) -> Result<AuthorEditSettlement, AuthorEditError> {
    client
        .execute(
            "INSERT INTO storyos.scope_counters (owner_user_id, project_id)
             VALUES ($1::text::uuid, $2::text::uuid) ON CONFLICT DO NOTHING",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(author_edit_database_error)?;

    let changes_authority = matches!(
        core_result,
        ApplyAuthorEditResult::AuthoritativeApplied { .. }
    );
    let counter_row = client
        .query_one(
            "UPDATE storyos.scope_counters
                SET author_action_sequence = author_action_sequence
                      + CASE WHEN $3 THEN 1 ELSE 0 END,
                    authoritative_commit_sequence = authoritative_commit_sequence
                      + CASE WHEN $3 THEN 1 ELSE 0 END,
                    project_activity_position = project_activity_position + 1
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
          RETURNING author_action_sequence::text, authoritative_commit_sequence::text,
                    project_activity_position::text",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &changes_authority,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    let author_action_sequence = changes_authority
        .then(|| parse_u64(counter_row.get(0)))
        .transpose()?;
    let authoritative_commit_sequence = changes_authority
        .then(|| parse_u64(counter_row.get(1)))
        .transpose()?;
    let project_activity_position = parse_u64(counter_row.get(2))?;

    let (result_kind, result_reason, resulting_head, effect) = match core_result {
        ApplyAuthorEditResult::AuthoritativeApplied { body } => {
            persist_authority_change(
                client,
                command,
                current_revision_id,
                &body,
                authoritative_commit_sequence
                    .expect("authority change allocates a Commit sequence"),
            )
            .await?;
            (
                "authoritative_applied",
                None,
                command.ids.revision_id.clone(),
                AuthorEditSettlementEffect::AuthoritativeApplied {
                    body,
                    author_action_sequence: author_action_sequence
                        .expect("authority change allocates an Author Action sequence"),
                },
            )
        }
        ApplyAuthorEditResult::NoEffect { reason } => (
            "no_effect",
            Some(no_effect_reason(&reason)),
            current_revision_id.to_owned(),
            AuthorEditSettlementEffect::NoEffect { reason },
        ),
        ApplyAuthorEditResult::Conflicted { reason } => (
            "conflicted",
            Some(conflict_reason(&reason)),
            current_revision_id.to_owned(),
            AuthorEditSettlementEffect::Conflicted {
                reason,
                current_authoritative_revision_id: current_revision_id.to_owned(),
            },
        ),
        ApplyAuthorEditResult::Refused { reason } => (
            "refused",
            Some(refusal_reason(&reason)),
            current_revision_id.to_owned(),
            AuthorEditSettlementEffect::Refused { reason },
        ),
    };

    let allocated_revision_id = changes_authority.then_some(command.ids.revision_id.as_str());
    let allocated_commit_id =
        changes_authority.then_some(command.ids.authoritative_commit_id.as_str());
    let author_action_sequence_text = author_action_sequence.map(|value| value.to_string());
    let project_activity_position_text = project_activity_position.to_string();
    let receipt_row = client
        .query_one(
            "INSERT INTO storyos.domain_receipts
               (owner_user_id, project_id, receipt_id, author_command_admission_id,
                command_id, command_kind, command_digest, idempotency_key, producer_cause,
                expected_heads, prior_heads, resulting_heads, authoritative_revision_ids,
                proposal_revision_ids, authoritative_commit_ids, author_action_sequence,
                draft_artifact_refs, artifact_lifecycle_event_refs, condition_refs,
                result_kind, result_reason, project_activity_position,
                authoritative_commit_id, resulting_revision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'applyAuthorEdit', $6, $7::text::uuid,
                     'author_command_admission', ARRAY[$8::text::uuid], ARRAY[$9::text::uuid],
                     ARRAY[$10::text::uuid],
                     CASE WHEN $11::text IS NULL THEN ARRAY[]::uuid[]
                          ELSE ARRAY[$11::text::uuid] END,
                     ARRAY[]::uuid[],
                     CASE WHEN $12::text IS NULL THEN ARRAY[]::uuid[]
                          ELSE ARRAY[$12::text::uuid] END,
                     $13::text::numeric, ARRAY[]::text[], ARRAY[]::text[], ARRAY[]::text[],
                     $14, $15, $16::text::numeric, $12::text::uuid, $11::text::uuid)
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
                &current_revision_id,
                &resulting_head,
                &allocated_revision_id,
                &allocated_commit_id,
                &author_action_sequence_text,
                &result_kind,
                &result_reason,
                &project_activity_position_text,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    let receipt_created_at = receipt_row.get::<_, String>(0);

    if let Some(author_action_sequence) = author_action_sequence {
        client
            .execute(
                "INSERT INTO storyos.author_action_entries
                   (owner_user_id, project_id, author_action_sequence, disposition,
                    authoritative_commit_id, receipt_id)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, 'forward',
                         $4::text::uuid, $5::text::uuid)",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &author_action_sequence.to_string(),
                    &command.ids.authoritative_commit_id,
                    &command.ids.receipt_id,
                ],
            )
            .await
            .map_err(author_edit_database_error)?;
    }
    client
        .execute(
            "INSERT INTO storyos.project_activity_events
               (owner_user_id, project_id, project_activity_position, project_activity_event_id,
                event_kind, receipt_id, resulting_revision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                     'author_edit_settled', $5::text::uuid, $6::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &project_activity_position_text,
                &command.ids.project_activity_event_id,
                &command.ids.receipt_id,
                &allocated_revision_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
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
        .map_err(author_edit_database_error)?;
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'applyAuthorEdit' AND idempotency_key = $4::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.receipt_id,
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;

    Ok(AuthorEditSettlement {
        ids: command.ids.clone(),
        effect,
        project_activity_position,
        receipt_created_at,
        completed_intent_record_id: command.completed_intent_record_id.clone(),
        local_intent_sequence: command.local_intent_sequence,
    })
}

async fn persist_authority_change(
    client: &Client,
    command: &ApplyAuthorEditCommand,
    current_revision_id: &str,
    body: &str,
    authoritative_commit_sequence: u64,
) -> Result<(), AuthorEditError> {
    client
        .execute(
            "INSERT INTO storyos.authoritative_payloads
           (owner_user_id, project_id, payload_id, canonical_bytes)
         VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, convert_to($4, 'UTF8'))",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.payload_id,
                &body,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.authoritative_revisions
           (owner_user_id, project_id, manuscript_object_id, revision_id, payload_id)
         VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.chapter_id,
                &command.ids.revision_id,
                &command.ids.payload_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    client
        .execute(
            "INSERT INTO storyos.authoritative_revision_envelopes
           (owner_user_id, project_id, manuscript_object_id, revision_id, parent_revision_id,
            schema_revision, creator_kind, creator_ref, cause_kind, payload_digest)
         VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                 $5::text::uuid, 'storyos.authoritative-revision-envelope.v1',
                 'author_command_admission', $6::text::uuid, 'direct_author_action', $7)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.chapter_id,
                &command.ids.revision_id,
                &current_revision_id,
                &command.ids.author_command_admission_id,
                &sha256_hex(body.as_bytes()),
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    let head_updates = client
        .execute(
            "UPDATE storyos.authoritative_heads SET current_revision_id = $4::text::uuid
          WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
            AND manuscript_object_id = $3::text::uuid AND current_revision_id = $5::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.chapter_id,
                &command.ids.revision_id,
                &current_revision_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    if head_updates != 1 {
        return Err(AuthorEditError::BindingConflict);
    }
    client
        .execute(
            "INSERT INTO storyos.authoritative_commits
           (owner_user_id, project_id, authoritative_commit_id, authoritative_commit_sequence,
            manuscript_object_id, prior_revision_id, resulting_revision_id,
            author_command_admission_id)
         VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::numeric,
                 $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.ids.authoritative_commit_id,
                &authoritative_commit_sequence.to_string(),
                &command.chapter_id,
                &current_revision_id,
                &command.ids.revision_id,
                &command.ids.author_command_admission_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    Ok(())
}

fn no_effect_reason(reason: &AuthorEditNoEffect) -> &'static str {
    match reason {
        AuthorEditNoEffect::ContentUnchanged => "content_unchanged",
    }
}

fn conflict_reason(reason: &AuthorEditConflict) -> &'static str {
    match reason {
        AuthorEditConflict::StaleAuthoritativeHead => "stale_authoritative_head",
        AuthorEditConflict::ProposalHeadPresent => "proposal_head_present",
        AuthorEditConflict::OwnershipChanged => "ownership_changed",
    }
}

fn refusal_reason(reason: &AuthorEditRefusal) -> &'static str {
    match reason {
        AuthorEditRefusal::UnsupportedIntentShape => "unsupported_intent_shape",
        AuthorEditRefusal::InvalidSelection => "invalid_selection",
        AuthorEditRefusal::TargetMismatch => "target_mismatch",
    }
}
