use storyos_application::{
    ApplyAuthorEditCommand, AuthorCommandAdmissionIds, AuthorEditError, AuthorEditSettlement,
    AuthorEditSettlementEffect,
};

use super::author_edit::{
    author_edit_challenge_error, author_edit_database_error, parse_u64, sha256_hex,
};
use super::*;

impl PostgresProjectReader {
    pub(super) async fn read_author_edit_settlement(
        &self,
        command: &ApplyAuthorEditCommand,
        receipt_id: &str,
    ) -> Result<AuthorEditSettlement, AuthorEditError> {
        let client = self
            .connect_challenge()
            .await
            .map_err(author_edit_challenge_error)?;
        client
            .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .await
            .map_err(author_edit_database_error)?;
        set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(author_edit_challenge_error)?;

        // Read the immutable Receipt from the idempotency result reference before
        // selecting its Authority or Activity relations.
        let receipt = client
            .query_opt(
                "SELECT receipt.command_id::text,
                        receipt.author_command_admission_id::text,
                        receipt.receipt_id::text,
                        admission.completed_intent_record_id::text,
                        admission.local_intent_sequence::text,
                        to_char(receipt.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        receipt.result_kind, receipt.result_reason,
                        receipt.expected_heads[1]::text,
                        receipt.prior_heads[1]::text,
                        receipt.resulting_heads[1]::text,
                        cardinality(receipt.expected_heads),
                        cardinality(receipt.prior_heads),
                        cardinality(receipt.resulting_heads),
                        cardinality(receipt.authoritative_revision_ids),
                        cardinality(receipt.proposal_revision_ids),
                        cardinality(receipt.authoritative_commit_ids),
                        cardinality(receipt.draft_artifact_refs),
                        cardinality(receipt.artifact_lifecycle_event_refs),
                        cardinality(receipt.condition_refs),
                        receipt.authoritative_revision_ids[1]::text,
                        receipt.authoritative_commit_ids[1]::text,
                        receipt.authoritative_commit_id::text,
                        receipt.resulting_revision_id::text,
                        receipt.author_action_sequence::text,
                        receipt.project_activity_position::text
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
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $3::text::uuid
                    AND receipt.command_kind = 'applyAuthorEdit'
                    AND receipt.command_digest = $4
                    AND receipt.idempotency_key = $5::text::uuid
                    AND settlement.settlement_kind = 'receipt_settled'
                    AND idempotency.outcome_kind = 'settled'
                    AND idempotency.result_reference = receipt.receipt_id::text
                    AND admission.chapter_object_id = $6::text::uuid
                    AND admission.expected_authoritative_revision_id = $7::text::uuid
                    AND admission.target_refs = $8::text[]
                    AND admission.expected_proposal_head_revision_ids =
                        receipt.proposal_revision_ids
                    AND receipt.expected_heads =
                        ARRAY[admission.expected_authoritative_revision_id]
                    AND EXISTS (
                      SELECT 1 FROM storyos.authoritative_revisions AS prior_revision
                       WHERE prior_revision.owner_user_id = receipt.owner_user_id
                         AND prior_revision.project_id = receipt.project_id
                         AND prior_revision.manuscript_object_id = admission.chapter_object_id
                         AND prior_revision.revision_id = receipt.prior_heads[1])
                    AND EXISTS (
                      SELECT 1 FROM storyos.authoritative_revisions AS resulting_revision
                       WHERE resulting_revision.owner_user_id = receipt.owner_user_id
                         AND resulting_revision.project_id = receipt.project_id
                         AND resulting_revision.manuscript_object_id = admission.chapter_object_id
                         AND resulting_revision.revision_id = receipt.resulting_heads[1])",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &receipt_id,
                    &command.challenge_binding.canonical_command_digest,
                    &command.challenge_binding.idempotency_key,
                    &command.chapter_id,
                    &command.expected_authoritative_revision_id,
                    &command.target_refs,
                ],
            )
            .await
            .map_err(author_edit_database_error)?
            .ok_or(AuthorEditError::BindingConflict)?;

        let result_kind = receipt.get::<_, String>(6);
        let result_reason = receipt.get::<_, Option<String>>(7);
        let expected_head = receipt.get::<_, String>(8);
        let prior_head = receipt.get::<_, String>(9);
        let resulting_head = receipt.get::<_, String>(10);
        let result_payload = match result_kind.as_str() {
            "authoritative_applied" if result_reason.is_none() => serde_json::json!({}),
            "no_effect" => serde_json::json!({"reason": result_reason}),
            "conflicted" => serde_json::json!({
                "reason": result_reason,
                "current_authoritative_revision_id": resulting_head,
            }),
            "refused" => serde_json::json!({"reason": result_reason}),
            _ => return Err(AuthorEditError::BindingConflict),
        };
        let common_cardinalities =
            [11, 12, 13, 15, 17, 18, 19].map(|index| receipt.get::<_, i32>(index));
        if common_cardinalities != [1, 1, 1, 0, 0, 0, 0] {
            return Err(AuthorEditError::BindingConflict);
        }

        let (effect, revision_id, payload_id, commit_id, activity) = match result_kind.as_str() {
            "authoritative_applied"
                if result_payload == serde_json::json!({})
                    && receipt.get::<_, i32>(14) == 1
                    && receipt.get::<_, i32>(16) == 1 =>
            {
                let revision_id = receipt
                    .get::<_, Option<String>>(20)
                    .ok_or(AuthorEditError::BindingConflict)?;
                let commit_id = receipt
                    .get::<_, Option<String>>(21)
                    .ok_or(AuthorEditError::BindingConflict)?;
                if receipt.get::<_, Option<String>>(22).as_deref() != Some(commit_id.as_str())
                    || receipt.get::<_, Option<String>>(23).as_deref() != Some(revision_id.as_str())
                    || resulting_head != revision_id
                    || expected_head != prior_head
                {
                    return Err(AuthorEditError::BindingConflict);
                }
                let relations = client
                    .query(
                        "SELECT activity.project_activity_event_id::text,
                                activity.project_activity_position::text,
                                activity.event_kind,
                                activity.resulting_revision_id::text,
                                action.author_action_sequence::text,
                                revision.payload_id::text,
                                convert_from(payload.canonical_bytes, 'UTF8'),
                                (SELECT count(*)
                                   FROM storyos.authoritative_revision_envelopes AS candidate
                                  WHERE candidate.owner_user_id = receipt.owner_user_id
                                    AND candidate.project_id = receipt.project_id
                                    AND candidate.creator_ref =
                                        receipt.author_command_admission_id),
                                (SELECT count(*)
                                   FROM storyos.authoritative_commits AS candidate
                                  WHERE candidate.owner_user_id = receipt.owner_user_id
                                    AND candidate.project_id = receipt.project_id
                                    AND candidate.author_command_admission_id =
                                        receipt.author_command_admission_id),
                                commit.authoritative_commit_id::text,
                                commit.manuscript_object_id::text,
                                commit.prior_revision_id::text,
                                commit.resulting_revision_id::text,
                                envelope.parent_revision_id::text,
                                envelope.payload_digest
                           FROM storyos.domain_receipts AS receipt
                           JOIN storyos.project_activity_events AS activity
                             ON (activity.owner_user_id, activity.project_id,
                                 activity.receipt_id) =
                                (receipt.owner_user_id, receipt.project_id,
                                 receipt.receipt_id)
                           JOIN storyos.author_action_entries AS action
                             ON (action.owner_user_id, action.project_id, action.receipt_id,
                                 action.authoritative_commit_id) =
                                (receipt.owner_user_id, receipt.project_id,
                                 receipt.receipt_id, receipt.authoritative_commit_id)
                           JOIN storyos.authoritative_commits AS commit
                             ON (commit.owner_user_id, commit.project_id,
                                 commit.authoritative_commit_id,
                                 commit.resulting_revision_id,
                                 commit.author_command_admission_id) =
                                (receipt.owner_user_id, receipt.project_id,
                                 receipt.authoritative_commit_id,
                                 receipt.resulting_revision_id,
                                 receipt.author_command_admission_id)
                           JOIN storyos.authoritative_revisions AS revision
                             ON (revision.owner_user_id, revision.project_id,
                                 revision.manuscript_object_id, revision.revision_id) =
                                (commit.owner_user_id, commit.project_id,
                                 commit.manuscript_object_id, commit.resulting_revision_id)
                           JOIN storyos.authoritative_revision_envelopes AS envelope
                             ON (envelope.owner_user_id, envelope.project_id,
                                 envelope.manuscript_object_id, envelope.revision_id,
                                 envelope.creator_ref) =
                                (revision.owner_user_id, revision.project_id,
                                 revision.manuscript_object_id, revision.revision_id,
                                 receipt.author_command_admission_id)
                           JOIN storyos.authoritative_payloads AS payload
                             ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                                (revision.owner_user_id, revision.project_id,
                                 revision.payload_id)
                          WHERE receipt.owner_user_id = $1::text::uuid
                            AND receipt.project_id = $2::text::uuid
                            AND receipt.receipt_id = $3::text::uuid",
                        &[
                            &command.project_scope.owner_user_id.as_ref(),
                            &command.project_scope.project_id.as_ref(),
                            &receipt_id,
                        ],
                    )
                    .await
                    .map_err(author_edit_database_error)?;
                if relations.len() != 1 {
                    return Err(AuthorEditError::BindingConflict);
                }
                let relation = &relations[0];
                let body = relation.get::<_, String>(6);
                let receipt_action_sequence = receipt
                    .get::<_, Option<String>>(24)
                    .ok_or(AuthorEditError::BindingConflict)?;
                if relation.get::<_, String>(2) != "author_edit_settled"
                    || relation.get::<_, Option<String>>(3).as_deref() != Some(revision_id.as_str())
                    || relation.get::<_, String>(4) != receipt_action_sequence
                    || relation.get::<_, i64>(7) != 1
                    || relation.get::<_, i64>(8) != 1
                    || relation.get::<_, String>(9) != commit_id
                    || relation.get::<_, String>(10) != command.chapter_id
                    || relation.get::<_, String>(11) != prior_head
                    || relation.get::<_, String>(12) != revision_id
                    || relation.get::<_, String>(13) != prior_head
                    || relation.get::<_, String>(14) != sha256_hex(body.as_bytes())
                    || relation.get::<_, String>(1) != receipt.get::<_, String>(25)
                {
                    return Err(AuthorEditError::BindingConflict);
                }
                let activity = ReplayedActivity {
                    id: relation.get(0),
                    position: parse_u64(relation.get(1))?,
                };
                (
                    AuthorEditSettlementEffect::AuthoritativeApplied {
                        body,
                        author_action_sequence: parse_u64(receipt_action_sequence)?,
                    },
                    revision_id,
                    relation.get(5),
                    commit_id,
                    activity,
                )
            }
            "no_effect"
                if result_payload == serde_json::json!({"reason": "content_unchanged"})
                    && expected_head == prior_head
                    && prior_head == resulting_head =>
            {
                let activity = read_zero_authority_activity(&client, command, receipt_id).await?;
                (
                    AuthorEditSettlementEffect::NoEffect {
                        reason: storyos_core::AuthorEditNoEffect::ContentUnchanged,
                    },
                    String::new(),
                    String::new(),
                    String::new(),
                    activity,
                )
            }
            "conflicted" => {
                let reason = result_payload
                    .get("reason")
                    .and_then(serde_json::Value::as_str);
                let current_revision_id = result_payload
                    .get("current_authoritative_revision_id")
                    .and_then(serde_json::Value::as_str);
                if result_payload.as_object().map(serde_json::Map::len) != Some(2)
                    || current_revision_id != Some(resulting_head.as_str())
                    || prior_head != resulting_head
                {
                    return Err(AuthorEditError::BindingConflict);
                }
                let activity = read_zero_authority_activity(&client, command, receipt_id).await?;
                (
                    AuthorEditSettlementEffect::Conflicted {
                        reason: parse_conflict_reason(reason)?,
                        current_authoritative_revision_id: resulting_head,
                    },
                    String::new(),
                    String::new(),
                    String::new(),
                    activity,
                )
            }
            "refused" => {
                let reason = result_payload
                    .get("reason")
                    .and_then(serde_json::Value::as_str);
                if result_payload.as_object().map(serde_json::Map::len) != Some(1)
                    || prior_head != resulting_head
                    || expected_head != resulting_head
                {
                    return Err(AuthorEditError::BindingConflict);
                }
                let activity = read_zero_authority_activity(&client, command, receipt_id).await?;
                (
                    AuthorEditSettlementEffect::Refused {
                        reason: parse_refusal_reason(reason)?,
                    },
                    String::new(),
                    String::new(),
                    String::new(),
                    activity,
                )
            }
            _ => return Err(AuthorEditError::BindingConflict),
        };

        client
            .batch_execute("COMMIT")
            .await
            .map_err(author_edit_database_error)?;
        Ok(AuthorEditSettlement {
            ids: AuthorCommandAdmissionIds {
                command_id: receipt.get(0),
                author_command_admission_id: receipt.get(1),
                receipt_id: receipt.get(2),
                revision_id,
                payload_id,
                authoritative_commit_id: commit_id,
                project_activity_event_id: activity.id,
            },
            effect,
            project_activity_position: activity.position,
            completed_intent_record_id: receipt.get(3),
            local_intent_sequence: parse_u64(receipt.get(4))?,
            receipt_created_at: receipt.get(5),
        })
    }
}

struct ReplayedActivity {
    id: String,
    position: u64,
}

async fn read_zero_authority_activity(
    client: &tokio_postgres::Client,
    command: &ApplyAuthorEditCommand,
    receipt_id: &str,
) -> Result<ReplayedActivity, AuthorEditError> {
    let relations = client
        .query(
            "SELECT activity.project_activity_event_id::text,
                    activity.project_activity_position::text,
                    activity.event_kind,
                    activity.resulting_revision_id::text,
                    (SELECT count(*) FROM storyos.author_action_entries AS action
                      WHERE action.owner_user_id = receipt.owner_user_id
                        AND action.project_id = receipt.project_id
                        AND action.receipt_id = receipt.receipt_id),
                    (SELECT count(*) FROM storyos.authoritative_commits AS commit
                      WHERE commit.owner_user_id = receipt.owner_user_id
                        AND commit.project_id = receipt.project_id
                        AND commit.author_command_admission_id =
                            receipt.author_command_admission_id),
                    (SELECT count(*)
                       FROM storyos.authoritative_revision_envelopes AS envelope
                      WHERE envelope.owner_user_id = receipt.owner_user_id
                        AND envelope.project_id = receipt.project_id
                        AND envelope.creator_ref = receipt.author_command_admission_id)
               FROM storyos.domain_receipts AS receipt
               JOIN storyos.project_activity_events AS activity
                 ON (activity.owner_user_id, activity.project_id, activity.receipt_id) =
                    (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
              WHERE receipt.owner_user_id = $1::text::uuid
                AND receipt.project_id = $2::text::uuid
                AND receipt.receipt_id = $3::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &receipt_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    if relations.len() != 1 {
        return Err(AuthorEditError::BindingConflict);
    }
    let relation = &relations[0];
    if relation.get::<_, String>(2) != "author_edit_settled"
        || relation.get::<_, Option<String>>(3).is_some()
        || [4, 5, 6].map(|index| relation.get::<_, i64>(index)) != [0, 0, 0]
    {
        return Err(AuthorEditError::BindingConflict);
    }
    Ok(ReplayedActivity {
        id: relation.get(0),
        position: parse_u64(relation.get(1))?,
    })
}

fn parse_conflict_reason(
    reason: Option<&str>,
) -> Result<storyos_core::AuthorEditConflict, AuthorEditError> {
    match reason {
        Some("stale_authoritative_head") => {
            Ok(storyos_core::AuthorEditConflict::StaleAuthoritativeHead)
        }
        Some("proposal_head_present") => Ok(storyos_core::AuthorEditConflict::ProposalHeadPresent),
        Some("ownership_changed") => Ok(storyos_core::AuthorEditConflict::OwnershipChanged),
        _ => Err(AuthorEditError::BindingConflict),
    }
}

fn parse_refusal_reason(
    reason: Option<&str>,
) -> Result<storyos_core::AuthorEditRefusal, AuthorEditError> {
    match reason {
        Some("unsupported_intent_shape") => {
            Ok(storyos_core::AuthorEditRefusal::UnsupportedIntentShape)
        }
        Some("invalid_selection") => Ok(storyos_core::AuthorEditRefusal::InvalidSelection),
        Some("target_mismatch") => Ok(storyos_core::AuthorEditRefusal::TargetMismatch),
        _ => Err(AuthorEditError::BindingConflict),
    }
}
