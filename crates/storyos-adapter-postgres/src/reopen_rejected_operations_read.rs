use storyos_application::{
    ReopenRejectedOperationsCommand, ReopenRejectedOperationsError,
    ReopenRejectedOperationsSettlement, ReopenRejectedOperationsSettlementEffect,
};
use storyos_core::{ReopenRejectedOperationsConflict, ReopenRejectedOperationsRefusal};

use super::{reopen_challenge_error, reopen_database_error, reopen_parse_error};
use crate::PostgresProjectReader;
use crate::author_edit::parse_u64;
use crate::command_response_project::{
    CommandResponseProjectEvidence, read_command_response_project,
};

pub(super) async fn read_reopen_settlement(
    store: &PostgresProjectReader,
    command: &ReopenRejectedOperationsCommand,
    receipt_id: &str,
) -> Result<ReopenRejectedOperationsSettlement, ReopenRejectedOperationsError> {
    let client = store
        .connect_challenge()
        .await
        .map_err(reopen_challenge_error)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(reopen_database_error)?;
    let result = async {
        crate::set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(reopen_challenge_error)?;
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
                        reopening.reopen_event_id::text,
                        reopening.resulting_proposal_revision_id::text,
                        resolution_head.generation,
                        resolution_head.closure,
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
              LEFT JOIN storyos.proposal_operation_reopenings AS reopening
                     ON (reopening.owner_user_id, reopening.project_id,
                         reopening.reopen_receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
              LEFT JOIN storyos.proposal_revisions AS resolution_head
                     ON (resolution_head.owner_user_id, resolution_head.project_id,
                         resolution_head.proposal_id, resolution_head.revision_id) =
                        (reopening.owner_user_id, reopening.project_id,
                         reopening.proposal_id, reopening.resulting_proposal_revision_id)
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $3::text::uuid
                    AND receipt.command_kind = 'reopenRejectedOperations'
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
            .map_err(reopen_database_error)?;
        let Some(row) = row else {
            return Err(ReopenRejectedOperationsError::HistoricalAcknowledgementUnavailable);
        };
        let response_project = match read_command_response_project(
            row.get::<_, Option<String>>(11).as_deref(),
            row.get::<_, Option<String>>(12).as_deref(),
        ) {
            Ok(CommandResponseProjectEvidence::Captured(project)) => project,
            Ok(CommandResponseProjectEvidence::HistoricalUnavailable) => {
                return Err(ReopenRejectedOperationsError::HistoricalAcknowledgementUnavailable);
            }
            Err(()) => {
                return Err(ReopenRejectedOperationsError::Unavailable(Box::new(
                    std::io::Error::other("Reopen acknowledgement evidence is damaged"),
                )));
            }
        };
        let result_kind: String = row.get(3);
        let reason: Option<String> = row.get(4);
        let effect = match (result_kind.as_str(), reason.as_deref()) {
            ("proposal_revised", _) => ReopenRejectedOperationsSettlementEffect::Resolved {
                author_action_sequence: parse_u64(row.get::<_, String>(6))
                    .map_err(reopen_parse_error)?,
                operation_id: command.selected_rejected_operation_id.clone(),
                rejection_event_id: command.rejection_event_id.clone(),
                resulting_proposal_revision_id: row.get(8),
                preserved_generation: row.get(9),
                preserved_closure: row.get(10),
                state_event_id: row.get(7),
            },
            ("conflicted", Some("changed_head")) => {
                ReopenRejectedOperationsSettlementEffect::Conflicted {
                    reason: ReopenRejectedOperationsConflict::ChangedHead,
                }
            }
            ("refused", Some("wrong_scope")) => ReopenRejectedOperationsSettlementEffect::Refused {
                reason: ReopenRejectedOperationsRefusal::WrongScope,
            },
            ("refused", Some("wrong_admission")) => {
                ReopenRejectedOperationsSettlementEffect::Refused {
                    reason: ReopenRejectedOperationsRefusal::WrongAdmission,
                }
            }
            ("refused", Some("stale_proposal_revision")) => {
                ReopenRejectedOperationsSettlementEffect::Refused {
                    reason: ReopenRejectedOperationsRefusal::StaleProposalRevision,
                }
            }
            ("refused", Some("not_eligible")) => {
                ReopenRejectedOperationsSettlementEffect::Refused {
                    reason: ReopenRejectedOperationsRefusal::NotEligible,
                }
            }
            ("refused", Some("operation_not_rejected")) => {
                ReopenRejectedOperationsSettlementEffect::Refused {
                    reason: ReopenRejectedOperationsRefusal::OperationNotRejected,
                }
            }
            ("refused", Some("unavailable_proof")) => {
                ReopenRejectedOperationsSettlementEffect::Refused {
                    reason: ReopenRejectedOperationsRefusal::UnavailableProof,
                }
            }
            _ => return Err(ReopenRejectedOperationsError::HistoricalAcknowledgementUnavailable),
        };
        Ok(ReopenRejectedOperationsSettlement {
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
