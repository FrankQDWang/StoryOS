use storyos_application::{
    AcceptProposalCommand, AcceptProposalError, AcceptProposalSettlement,
    AcceptProposalSettlementEffect,
};
use storyos_core::{AcceptProposalConflict, AcceptProposalInvalid, AcceptProposalRefusal};

use super::{accept_challenge_error, accept_database_error, accept_parse_error};
use crate::PostgresProjectReader;
use crate::author_edit::parse_u64;
use crate::command_response_project::{
    CommandResponseProjectEvidence, read_command_response_project,
};

pub(super) async fn read_accept_settlement(
    store: &PostgresProjectReader,
    command: &AcceptProposalCommand,
    receipt_id: &str,
) -> Result<AcceptProposalSettlement, AcceptProposalError> {
    let client = store
        .connect_challenge()
        .await
        .map_err(accept_challenge_error)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(accept_database_error)?;
    let result = async {
        crate::set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(accept_challenge_error)?;
        let row = client
            .query_opt(
                "SELECT receipt.command_id::text,
                        receipt.author_command_admission_id::text,
                        receipt.receipt_id::text,
                        receipt.result_kind,
                        receipt.result_payload->>'reason',
                        to_char(receipt.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        action.author_action_sequence::text,
                        commit.authoritative_commit_id::text,
                        commit.resulting_revision_id::text,
                        convert_from(payload.canonical_bytes, 'UTF8'),
                        activity.project_activity_position::text,
                        commit.manuscript_object_id::text,
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
              LEFT JOIN storyos.author_action_entries AS action
                     ON (action.owner_user_id, action.project_id, action.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
              LEFT JOIN storyos.authoritative_commits AS commit
                     ON (commit.owner_user_id, commit.project_id, commit.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
              LEFT JOIN storyos.authoritative_revisions AS revision
                     ON (revision.owner_user_id, revision.project_id,
                         revision.manuscript_object_id, revision.revision_id) =
                        (commit.owner_user_id, commit.project_id,
                         commit.manuscript_object_id, commit.resulting_revision_id)
              LEFT JOIN storyos.authoritative_payloads AS payload
                     ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                        (revision.owner_user_id, revision.project_id, revision.payload_id)
              LEFT JOIN storyos.project_activity_events AS activity
                     ON (activity.owner_user_id, activity.project_id, activity.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $3::text::uuid
                    AND receipt.command_kind = 'acceptProposal'
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
            .map_err(accept_database_error)?;
        let Some(row) = row else {
            return Err(AcceptProposalError::HistoricalAcknowledgementUnavailable);
        };
        let response_project = match read_command_response_project(
            row.get::<_, Option<String>>(12).as_deref(),
            row.get::<_, Option<String>>(13).as_deref(),
        ) {
            Ok(CommandResponseProjectEvidence::Captured(project)) => project,
            Ok(CommandResponseProjectEvidence::HistoricalUnavailable) => {
                return Err(AcceptProposalError::HistoricalAcknowledgementUnavailable);
            }
            Err(()) => {
                return Err(AcceptProposalError::Unavailable(Box::new(
                    std::io::Error::other("Acceptance acknowledgement evidence is damaged"),
                )));
            }
        };
        let result_kind: String = row.get(3);
        let reason: Option<String> = row.get(4);
        let effect = match (result_kind.as_str(), reason.as_deref()) {
            ("authoritative_applied", _) => {
                let chapter_id: String = row.get(11);
                let revision_id: String = row.get(8);
                let stored: String = row.get(9);
                let blocks = crate::manuscript_block::load_revision_blocks(
                    &*client,
                    command.project_scope.owner_user_id.as_ref(),
                    command.project_scope.project_id.as_ref(),
                    &chapter_id,
                    &revision_id,
                    &stored,
                )
                .await
                .map_err(accept_database_error)?;
                AcceptProposalSettlementEffect::Applied {
                    author_action_sequence: parse_u64(row.get::<_, String>(6))
                        .map_err(accept_parse_error)?,
                    authoritative_commit_id: row.get(7),
                    revision_id,
                    body: crate::manuscript_block::display_body_from_stored(&stored, &blocks),
                    blocks,
                    project_activity_position: parse_u64(row.get::<_, String>(10))
                        .map_err(accept_parse_error)?,
                }
            }
            ("invalid", Some("invalid_validation")) => AcceptProposalSettlementEffect::Invalid {
                reason: AcceptProposalInvalid::InvalidValidation,
            },
            ("invalid", Some("altered_candidate")) => AcceptProposalSettlementEffect::Invalid {
                reason: AcceptProposalInvalid::AlteredCandidate,
            },
            ("conflicted", Some("changed_head")) => AcceptProposalSettlementEffect::Conflicted {
                reason: AcceptProposalConflict::ChangedHead,
            },
            ("refused", Some("wrong_scope")) => AcceptProposalSettlementEffect::Refused {
                reason: AcceptProposalRefusal::WrongScope,
            },
            ("refused", Some("wrong_admission")) => AcceptProposalSettlementEffect::Refused {
                reason: AcceptProposalRefusal::WrongAdmission,
            },
            ("refused", Some("stale_proposal_revision")) => {
                AcceptProposalSettlementEffect::Refused {
                    reason: AcceptProposalRefusal::StaleProposalRevision,
                }
            }
            ("refused", Some("not_eligible")) => AcceptProposalSettlementEffect::Refused {
                reason: AcceptProposalRefusal::NotEligible,
            },
            ("refused", Some("operation_not_pending")) => AcceptProposalSettlementEffect::Refused {
                reason: AcceptProposalRefusal::OperationNotPending,
            },
            _ => return Err(AcceptProposalError::HistoricalAcknowledgementUnavailable),
        };
        Ok(AcceptProposalSettlement {
            ids: storyos_application::AuthorCommandAdmissionIds {
                command_id: row.get(0),
                author_command_admission_id: row.get(1),
                receipt_id: row.get(2),
            },
            effect,
            receipt_created_at: row.get(5),
            response_project,
        })
    }
    .await;
    let _rollback = client.batch_execute("ROLLBACK").await;
    result
}
