use storyos_application::{
    ApplyAuthorEditCommand, AuthorCommandAdmissionIds, AuthorEditError, AuthorEditSettlement,
    AuthorEditSettlementEffect,
};

use super::author_edit::{author_edit_challenge_error, author_edit_database_error, parse_u64};
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
            .batch_execute("BEGIN")
            .await
            .map_err(author_edit_database_error)?;
        set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(author_edit_challenge_error)?;
        let row = client
            .query_opt(
                "SELECT admission.command_id::text, admission.author_command_admission_id::text,
                    receipt.receipt_id::text, COALESCE(receipt.resulting_revision_id::text, ''),
                    COALESCE(revision.payload_id::text, ''),
                    COALESCE(receipt.authoritative_commit_id::text, ''),
                    activity.project_activity_event_id::text,
                    CASE WHEN payload.payload_id IS NULL THEN NULL
                         ELSE convert_from(payload.canonical_bytes, 'UTF8') END,
                    action.author_action_sequence::text, activity.project_activity_position::text,
                    admission.completed_intent_record_id::text,
                    admission.local_intent_sequence::text,
                    to_char(receipt.created_at AT TIME ZONE 'UTC',
                            'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                    receipt.result_kind, receipt.result_reason,
                    receipt.resulting_heads[1]::text
               FROM storyos.domain_receipts AS receipt
               JOIN storyos.author_command_admissions AS admission
                 ON (admission.owner_user_id, admission.project_id,
                     admission.author_command_admission_id, admission.command_id) =
                    (receipt.owner_user_id, receipt.project_id,
                     receipt.author_command_admission_id, receipt.command_id)
               LEFT JOIN storyos.authoritative_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.revision_id) =
                    (receipt.owner_user_id, receipt.project_id, receipt.resulting_revision_id)
               LEFT JOIN storyos.authoritative_payloads AS payload
                 ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                    (revision.owner_user_id, revision.project_id, revision.payload_id)
               LEFT JOIN storyos.author_action_entries AS action
                 ON (action.owner_user_id, action.project_id, action.receipt_id,
                     action.authoritative_commit_id) =
                    (receipt.owner_user_id, receipt.project_id, receipt.receipt_id,
                     receipt.authoritative_commit_id)
               JOIN storyos.project_activity_events AS activity
                 ON (activity.owner_user_id, activity.project_id, activity.receipt_id) =
                    (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
              WHERE receipt.owner_user_id = $1::text::uuid
                AND receipt.project_id = $2::text::uuid AND receipt.receipt_id = $3::text::uuid",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &receipt_id,
                ],
            )
            .await
            .map_err(author_edit_database_error)?;
        client
            .batch_execute("COMMIT")
            .await
            .map_err(author_edit_database_error)?;
        let Some(row) = row else {
            return Err(AuthorEditError::BindingConflict);
        };
        let result_kind = row.get::<_, String>(13);
        let result_reason = row.get::<_, Option<String>>(14);
        let effect = match result_kind.as_str() {
            "authoritative_applied" => AuthorEditSettlementEffect::AuthoritativeApplied {
                body: row
                    .get::<_, Option<String>>(7)
                    .ok_or(AuthorEditError::BindingConflict)?,
                author_action_sequence: parse_u64(
                    row.get::<_, Option<String>>(8)
                        .ok_or(AuthorEditError::BindingConflict)?,
                )?,
            },
            "no_effect" if result_reason.as_deref() == Some("content_unchanged") => {
                AuthorEditSettlementEffect::NoEffect {
                    reason: storyos_core::AuthorEditNoEffect::ContentUnchanged,
                }
            }
            "conflicted" => AuthorEditSettlementEffect::Conflicted {
                reason: parse_conflict_reason(result_reason.as_deref())?,
                current_authoritative_revision_id: row.get(15),
            },
            "refused" => AuthorEditSettlementEffect::Refused {
                reason: parse_refusal_reason(result_reason.as_deref())?,
            },
            _ => return Err(AuthorEditError::BindingConflict),
        };
        Ok(AuthorEditSettlement {
            ids: AuthorCommandAdmissionIds {
                command_id: row.get(0),
                author_command_admission_id: row.get(1),
                receipt_id: row.get(2),
                revision_id: row.get(3),
                payload_id: row.get(4),
                authoritative_commit_id: row.get(5),
                project_activity_event_id: row.get(6),
            },
            effect,
            project_activity_position: parse_u64(row.get(9))?,
            completed_intent_record_id: row.get(10),
            local_intent_sequence: parse_u64(row.get(11))?,
            receipt_created_at: row.get(12),
        })
    }
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
