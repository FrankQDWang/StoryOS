use storyos_application::{
    ReopenWithdrawnProposalCommand, ReopenWithdrawnProposalError,
    ReopenWithdrawnProposalSettlement, ReopenWithdrawnProposalSettlementEffect,
};
use storyos_core::{
    ReopenWithdrawnProposalConflict, ReopenWithdrawnProposalNoEffect,
    ReopenWithdrawnProposalRefusal,
};

use super::{reopen_challenge_error, reopen_database_error, reopen_parse_error};
use crate::PostgresProjectReader;
use crate::author_edit::parse_u64;
use crate::command_response_project::{
    CommandResponseProjectEvidence, read_command_response_project,
};

pub(super) async fn read_reopen_settlement(
    store: &PostgresProjectReader,
    command: &ReopenWithdrawnProposalCommand,
    receipt_id: &str,
) -> Result<ReopenWithdrawnProposalSettlement, ReopenWithdrawnProposalError> {
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
                        receipt.proposal_revision_ids[1]::text,
                        to_char(receipt.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        action.author_action_sequence::text,
                        revision.generation,
                        (
                          SELECT operation.resolution
                            FROM storyos.proposal_operations AS operation
                           WHERE operation.owner_user_id = revision.owner_user_id
                             AND operation.project_id = revision.project_id
                             AND operation.proposal_id = revision.proposal_id
                           ORDER BY operation.operation_id
                           LIMIT 1
                        ),
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
              LEFT JOIN storyos.proposal_revisions AS revision
                     ON revision.owner_user_id = receipt.owner_user_id
                    AND revision.project_id = receipt.project_id
                    AND revision.revision_id = receipt.proposal_revision_ids[1]
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $3::text::uuid
                    AND receipt.command_kind = 'reopenWithdrawnProposal'
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
            return Err(ReopenWithdrawnProposalError::HistoricalAcknowledgementUnavailable);
        };
        let result_kind: String = row.get(3);
        let reason: Option<String> = row.get(4);
        let effect = match (result_kind.as_str(), reason.as_deref()) {
            ("proposal_revised", _) => {
                let revision_id: Option<String> = row.get(5);
                let sequence: Option<String> = row.get(7);
                let generation: Option<String> = row.get(8);
                let resolution: Option<String> = row.get(9);
                match (revision_id, sequence, generation, resolution) {
                    (
                        Some(resulting_proposal_revision_id),
                        Some(sequence),
                        Some(preserved_generation),
                        Some(preserved_operation_resolution),
                    ) => ReopenWithdrawnProposalSettlementEffect::Resolved {
                        author_action_sequence: parse_u64(sequence).map_err(reopen_parse_error)?,
                        resulting_proposal_revision_id,
                        preserved_generation,
                        preserved_operation_resolution,
                    },
                    _ => {
                        return Err(
                            ReopenWithdrawnProposalError::HistoricalAcknowledgementUnavailable,
                        );
                    }
                }
            }
            ("conflicted", Some("changed_head")) => {
                ReopenWithdrawnProposalSettlementEffect::Conflicted {
                    reason: ReopenWithdrawnProposalConflict::ChangedHead,
                }
            }
            ("refused", Some("wrong_scope")) => ReopenWithdrawnProposalSettlementEffect::Refused {
                reason: ReopenWithdrawnProposalRefusal::WrongScope,
            },
            ("refused", Some("wrong_admission")) => {
                ReopenWithdrawnProposalSettlementEffect::Refused {
                    reason: ReopenWithdrawnProposalRefusal::WrongAdmission,
                }
            }
            ("refused", Some("stale_proposal_revision")) => {
                ReopenWithdrawnProposalSettlementEffect::Refused {
                    reason: ReopenWithdrawnProposalRefusal::StaleProposalRevision,
                }
            }
            ("no_effect", Some("terminal_supersession")) => {
                ReopenWithdrawnProposalSettlementEffect::NoEffect {
                    reason: ReopenWithdrawnProposalNoEffect::TerminalSupersession,
                }
            }
            ("no_effect", Some("closure_not_withdrawn")) => {
                ReopenWithdrawnProposalSettlementEffect::NoEffect {
                    reason: ReopenWithdrawnProposalNoEffect::ClosureNotWithdrawn,
                }
            }
            ("no_effect", Some("withdrawal_event_mismatch")) => {
                ReopenWithdrawnProposalSettlementEffect::NoEffect {
                    reason: ReopenWithdrawnProposalNoEffect::WithdrawalEventMismatch,
                }
            }
            _ => {
                return Err(ReopenWithdrawnProposalError::HistoricalAcknowledgementUnavailable);
            }
        };
        let response_project = match read_command_response_project(
            row.get::<_, Option<String>>(10).as_deref(),
            row.get::<_, Option<String>>(11).as_deref(),
        ) {
            Ok(CommandResponseProjectEvidence::Captured(project)) => project,
            Ok(CommandResponseProjectEvidence::HistoricalUnavailable) => {
                return Err(ReopenWithdrawnProposalError::HistoricalAcknowledgementUnavailable);
            }
            Err(()) => {
                return Err(ReopenWithdrawnProposalError::Unavailable(Box::new(
                    std::io::Error::other("Reopen acknowledgement evidence is damaged"),
                )));
            }
        };
        Ok(ReopenWithdrawnProposalSettlement {
            ids: storyos_application::AuthorCommandAdmissionIds {
                command_id: row.get(0),
                author_command_admission_id: row.get(1),
                receipt_id: row.get(2),
            },
            effect,
            receipt_created_at: row.get(6),
            response_project,
        })
    }
    .await;
    let _ = client.batch_execute("ROLLBACK").await;
    result
}
