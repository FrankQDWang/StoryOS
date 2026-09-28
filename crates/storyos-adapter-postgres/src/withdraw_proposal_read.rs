use storyos_application::{
    ResolvedWithdrawal, WithdrawProposalCommand, WithdrawProposalError, WithdrawProposalSettlement,
    WithdrawProposalSettlementEffect,
};
use storyos_core::{WithdrawProposalConflict, WithdrawProposalNoEffect, WithdrawProposalRefusal};

use super::{withdraw_challenge_error, withdraw_database_error, withdraw_parse_error};
use crate::PostgresProjectReader;
use crate::author_edit::parse_u64;
use crate::command_response_project::{
    CommandResponseProjectEvidence, read_command_response_project,
};

pub(super) async fn read_withdraw_settlement(
    store: &PostgresProjectReader,
    command: &WithdrawProposalCommand,
    receipt_id: &str,
) -> Result<WithdrawProposalSettlement, WithdrawProposalError> {
    let client = store
        .connect_challenge()
        .await
        .map_err(withdraw_challenge_error)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(withdraw_database_error)?;
    let result = async {
        crate::set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(withdraw_challenge_error)?;
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
                        revision.generation,
                        revision.validation,
                        withdrawal.withdrawal_event_id::text,
                        withdrawal.author_note,
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
              LEFT JOIN storyos.proposal_withdrawals AS withdrawal
                     ON (withdrawal.owner_user_id, withdrawal.project_id,
                         withdrawal.withdrawal_receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
              LEFT JOIN storyos.proposal_revisions AS revision
                     ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                         revision.revision_id) =
                        (withdrawal.owner_user_id, withdrawal.project_id,
                         withdrawal.proposal_id, withdrawal.proposal_revision_id)
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $3::text::uuid
                    AND receipt.command_kind = 'withdrawProposal'
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
            .map_err(withdraw_database_error)?;
        let Some(row) = row else {
            return Err(WithdrawProposalError::HistoricalAcknowledgementUnavailable);
        };
        let response_project = match read_command_response_project(
            row.get::<_, Option<String>>(11).as_deref(),
            row.get::<_, Option<String>>(12).as_deref(),
        ) {
            Ok(CommandResponseProjectEvidence::Captured(project)) => project,
            Ok(CommandResponseProjectEvidence::HistoricalUnavailable) => {
                return Err(WithdrawProposalError::HistoricalAcknowledgementUnavailable);
            }
            Err(()) => {
                return Err(WithdrawProposalError::Unavailable(Box::new(
                    std::io::Error::other("Withdrawal acknowledgement evidence is damaged"),
                )));
            }
        };
        let result_kind: String = row.get(3);
        let reason: Option<String> = row.get(4);
        let effect = match (result_kind.as_str(), reason.as_deref()) {
            ("proposal_closure_changed", _) => WithdrawProposalSettlementEffect::Resolved {
                ownership: ResolvedWithdrawal::Author {
                    author_action_sequence: parse_u64(row.get::<_, String>(6))
                        .map_err(withdraw_parse_error)?,
                    withdrawal_note: match row.get::<_, Option<String>>(10) {
                        Some(text) => storyos_application::WithdrawalNote::Present { text },
                        None => storyos_application::WithdrawalNote::Omitted,
                    },
                },
                preserved_generation: row.get(7),
                preserved_validation: row.get(8),
                withdrawal_event_id: row.get(9),
            },
            ("conflicted", Some("changed_head")) => WithdrawProposalSettlementEffect::Conflicted {
                reason: WithdrawProposalConflict::ChangedHead,
            },
            ("refused", Some("wrong_scope")) => WithdrawProposalSettlementEffect::Refused {
                reason: WithdrawProposalRefusal::WrongScope,
            },
            ("refused", Some("wrong_admission")) => WithdrawProposalSettlementEffect::Refused {
                reason: WithdrawProposalRefusal::WrongAdmission,
            },
            ("refused", Some("stale_proposal_revision")) => {
                WithdrawProposalSettlementEffect::Refused {
                    reason: WithdrawProposalRefusal::StaleProposalRevision,
                }
            }
            ("no_effect", Some("unsupported_cause")) => {
                WithdrawProposalSettlementEffect::NoEffect {
                    reason: WithdrawProposalNoEffect::UnsupportedCause,
                }
            }
            ("no_effect", Some("terminal_supersession")) => {
                WithdrawProposalSettlementEffect::NoEffect {
                    reason: WithdrawProposalNoEffect::TerminalSupersession,
                }
            }
            ("no_effect", Some("closure_not_open")) => WithdrawProposalSettlementEffect::NoEffect {
                reason: WithdrawProposalNoEffect::ClosureNotOpen,
            },
            _ => return Err(WithdrawProposalError::HistoricalAcknowledgementUnavailable),
        };
        Ok(WithdrawProposalSettlement {
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
    client
        .batch_execute("ROLLBACK")
        .await
        .map_err(withdraw_database_error)?;
    result
}
