use storyos_application::{
    RejectProposalOperationsCommand, RejectProposalOperationsError,
    RejectProposalOperationsSettlement, RejectProposalOperationsSettlementEffect, RejectionNote,
};
use storyos_core::{RejectProposalOperationsConflict, RejectProposalOperationsRefusal};

use super::{reject_challenge_error, reject_database_error, reject_parse_error};
use crate::PostgresProjectReader;
use crate::author_edit::parse_u64;
use crate::command_response_project::{
    CommandResponseProjectEvidence, read_command_response_project,
};

pub(super) async fn read_reject_settlement(
    store: &PostgresProjectReader,
    command: &RejectProposalOperationsCommand,
    receipt_id: &str,
) -> Result<RejectProposalOperationsSettlement, RejectProposalOperationsError> {
    let client = store
        .connect_challenge()
        .await
        .map_err(reject_challenge_error)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(reject_database_error)?;
    let result = async {
        crate::set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(reject_challenge_error)?;
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
                        resolution.resolution_event_id::text,
                        resolution_head.generation,
                        resolution_head.validation,
                        resolution_head.closure,
                        rejected.rejection_note,
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
              LEFT JOIN LATERAL (
                    SELECT resolution.resolution_event_id, resolution.proposal_id,
                           resolution.proposal_revision_id
                      FROM storyos.proposal_operation_resolutions AS resolution
                     WHERE (resolution.owner_user_id, resolution.project_id,
                            resolution.rejection_receipt_id) =
                           (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                     ORDER BY resolution.operation_id
                     LIMIT 1
                   ) AS resolution ON true
              LEFT JOIN storyos.proposal_revisions AS resolution_head
                     ON (resolution_head.owner_user_id, resolution_head.project_id,
                         resolution_head.proposal_id, resolution_head.revision_id) =
                        (resolution.owner_user_id, resolution.project_id,
                         resolution.proposal_id, resolution.proposal_revision_id)
              LEFT JOIN storyos.proposal_rejection_receipts AS rejected
                     ON (rejected.owner_user_id, rejected.project_id,
                         rejected.rejection_receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $3::text::uuid
                    AND receipt.command_kind = 'rejectProposalOperations'
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
            .map_err(reject_database_error)?;
        let Some(row) = row else {
            return Err(RejectProposalOperationsError::HistoricalAcknowledgementUnavailable);
        };
        let response_project = match read_command_response_project(
            row.get::<_, Option<String>>(12).as_deref(),
            row.get::<_, Option<String>>(13).as_deref(),
        ) {
            Ok(CommandResponseProjectEvidence::Captured(project)) => project,
            Ok(CommandResponseProjectEvidence::HistoricalUnavailable) => {
                return Err(RejectProposalOperationsError::HistoricalAcknowledgementUnavailable);
            }
            Err(()) => {
                return Err(RejectProposalOperationsError::Unavailable(Box::new(
                    std::io::Error::other("Rejection acknowledgement evidence is damaged"),
                )));
            }
        };
        let result_kind: String = row.get(3);
        let reason: Option<String> = row.get(4);
        let effect = match (result_kind.as_str(), reason.as_deref()) {
            ("proposal_operations_resolved", _) => {
                RejectProposalOperationsSettlementEffect::Resolved {
                    author_action_sequence: parse_u64(row.get::<_, String>(6))
                        .map_err(reject_parse_error)?,
                    operation_ids: command.selected_pending_operation_ids.clone(),
                    rejection_note: match row.get::<_, Option<String>>(11) {
                        Some(text) => RejectionNote::Present { text },
                        None => RejectionNote::Omitted,
                    },
                    preserved_generation: row.get(8),
                    preserved_validation: row.get(9),
                    preserved_closure: row.get(10),
                    resolution_event_id: row.get(7),
                }
            }
            ("conflicted", Some("changed_head")) => {
                RejectProposalOperationsSettlementEffect::Conflicted {
                    reason: RejectProposalOperationsConflict::ChangedHead,
                }
            }
            ("refused", Some("wrong_scope")) => RejectProposalOperationsSettlementEffect::Refused {
                reason: RejectProposalOperationsRefusal::WrongScope,
            },
            ("refused", Some("wrong_admission")) => {
                RejectProposalOperationsSettlementEffect::Refused {
                    reason: RejectProposalOperationsRefusal::WrongAdmission,
                }
            }
            ("refused", Some("stale_proposal_revision")) => {
                RejectProposalOperationsSettlementEffect::Refused {
                    reason: RejectProposalOperationsRefusal::StaleProposalRevision,
                }
            }
            ("refused", Some("not_eligible")) => {
                RejectProposalOperationsSettlementEffect::Refused {
                    reason: RejectProposalOperationsRefusal::NotEligible,
                }
            }
            ("refused", Some("operation_not_pending")) => {
                RejectProposalOperationsSettlementEffect::Refused {
                    reason: RejectProposalOperationsRefusal::OperationNotPending,
                }
            }
            ("refused", Some("duplicate_identities")) => {
                RejectProposalOperationsSettlementEffect::Refused {
                    reason: RejectProposalOperationsRefusal::DuplicateIdentities,
                }
            }
            ("refused", Some("missing_required_dependencies")) => {
                RejectProposalOperationsSettlementEffect::Refused {
                    reason: RejectProposalOperationsRefusal::MissingRequiredDependencies,
                }
            }
            ("refused", Some("incomplete_bundle_closure")) => {
                RejectProposalOperationsSettlementEffect::Refused {
                    reason: RejectProposalOperationsRefusal::IncompleteBundleClosure,
                }
            }
            _ => return Err(RejectProposalOperationsError::HistoricalAcknowledgementUnavailable),
        };
        Ok(RejectProposalOperationsSettlement {
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
