#[path = "withdraw_proposal_read.rs"]
mod read;
#[path = "withdraw_proposal_write.rs"]
mod write;

use read::read_withdraw_settlement;
use write::{insert_withdraw_admission, persist_resolved, persist_zero};

use super::*;
use storyos_application::{
    ProjectCommandChallengeError, ProjectCommandChallengeUse, ResolvedWithdrawal,
    WithdrawProposalCommand, WithdrawProposalError, WithdrawProposalSettlement,
    WithdrawProposalSettlementEffect, WithdrawProposalStore, WithdrawalActor,
};
use storyos_core::{
    WithdrawProposal as CoreWithdraw, WithdrawProposalResult, WithdrawalCause, withdraw_proposal,
};

impl WithdrawProposalStore for PostgresProjectReader {
    async fn withdraw_proposal(
        &self,
        command: &WithdrawProposalCommand,
    ) -> Result<WithdrawProposalSettlement, WithdrawProposalError> {
        if matches!(command.actor, WithdrawalActor::CurrentProducer { .. }) {
            let transaction = self
                .begin_serializable_project_command_transaction(&command.project_scope)
                .await
                .map_err(withdraw_challenge_error)?;
            let outcome = async {
                if let Some(existing) = read_producer_receipt(&transaction.client, command).await? {
                    return Ok(existing);
                }
                persist_withdraw(&transaction.client, command).await
            }
            .await;
            return match outcome {
                Ok(settlement) => {
                    transaction
                        .commit()
                        .await
                        .map_err(withdraw_challenge_error)?;
                    Ok(settlement)
                }
                Err(WithdrawProposalError::BindingConflict) => {
                    transaction
                        .rollback()
                        .await
                        .map_err(withdraw_challenge_error)?;
                    let replay = self
                        .begin_serializable_project_command_transaction(&command.project_scope)
                        .await
                        .map_err(withdraw_challenge_error)?;
                    let replayed = read_producer_receipt(&replay.client, command).await;
                    replay.rollback().await.map_err(withdraw_challenge_error)?;
                    match replayed {
                        Ok(Some(settlement)) => Ok(settlement),
                        Ok(None) => Err(WithdrawProposalError::BindingConflict),
                        Err(error) => Err(error),
                    }
                }
                Err(error) => {
                    transaction
                        .rollback()
                        .await
                        .map_err(withdraw_challenge_error)?;
                    Err(error)
                }
            };
        }
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(withdraw_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(withdraw_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(withdraw_challenge_error)?;
                read_withdraw_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(withdraw_challenge_error)?;
                Err(WithdrawProposalError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_withdraw(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction
                            .commit()
                            .await
                            .map_err(withdraw_challenge_error)?;
                        Ok(settlement)
                    }
                    Err(error) => {
                        transaction
                            .rollback()
                            .await
                            .map_err(withdraw_challenge_error)?;
                        Err(error)
                    }
                }
            }
        }
    }
}

struct LoadedProposal {
    current_revision_id: String,
    generation: String,
    validation: String,
    closure: String,
    chapter_id: String,
    current_head_revision_id: Option<String>,
    source_run_id: Option<String>,
    source_decision_id: Option<String>,
}

async fn persist_withdraw(
    client: &tokio_postgres::Client,
    command: &WithdrawProposalCommand,
) -> Result<WithdrawProposalSettlement, WithdrawProposalError> {
    let Some(loaded) = load_proposal(client, command).await? else {
        return Err(WithdrawProposalError::MissingProject);
    };
    let (cause, admission_valid, producer_matches) = match &command.actor {
        WithdrawalActor::Author => (WithdrawalCause::Author, true, false),
        WithdrawalActor::CurrentProducer {
            run_id,
            decision_id,
        } => (
            WithdrawalCause::CurrentProducer,
            false,
            loaded.source_run_id.as_deref() == Some(run_id.as_str())
                && loaded.source_decision_id.as_deref() == Some(decision_id.as_str()),
        ),
    };
    if matches!(command.actor, WithdrawalActor::Author) {
        insert_withdraw_admission(client, command, &loaded.chapter_id).await?;
    }
    let classified = withdraw_proposal(&CoreWithdraw {
        scope_matches: true,
        cause,
        admission_valid,
        producer_matches,
        proposal_revision_current: loaded.current_revision_id == command.proposal_revision_id,
        closure_open: loaded.closure == "open",
        terminal_supersession: loaded.closure == "superseded",
        expected_target_matches_head: loaded.current_head_revision_id.as_deref()
            == Some(command.expected_authoritative_revision_id.as_str()),
    });
    match classified {
        WithdrawProposalResult::Resolved { allocation } => {
            persist_resolved(client, command, &loaded, allocation).await
        }
        WithdrawProposalResult::Conflicted { reason } => {
            persist_zero(
                client,
                command,
                "conflicted",
                "changed_head",
                WithdrawProposalSettlementEffect::Conflicted { reason },
            )
            .await
        }
        WithdrawProposalResult::Refused { reason } => {
            let reason_code = match &reason {
                storyos_core::WithdrawProposalRefusal::WrongScope => "wrong_scope",
                storyos_core::WithdrawProposalRefusal::WrongAdmission => "wrong_admission",
                storyos_core::WithdrawProposalRefusal::StaleProposalRevision => {
                    "stale_proposal_revision"
                }
            };
            persist_zero(
                client,
                command,
                "refused",
                reason_code,
                WithdrawProposalSettlementEffect::Refused { reason },
            )
            .await
        }
        WithdrawProposalResult::NoEffect { reason } => {
            let reason_code = match &reason {
                storyos_core::WithdrawProposalNoEffect::UnsupportedCause => "unsupported_cause",
                storyos_core::WithdrawProposalNoEffect::TerminalSupersession => {
                    "terminal_supersession"
                }
                storyos_core::WithdrawProposalNoEffect::ClosureNotOpen => "closure_not_open",
            };
            persist_zero(
                client,
                command,
                "no_effect",
                reason_code,
                WithdrawProposalSettlementEffect::NoEffect { reason },
            )
            .await
        }
    }
}

async fn load_proposal(
    client: &tokio_postgres::Client,
    command: &WithdrawProposalCommand,
) -> Result<Option<LoadedProposal>, WithdrawProposalError> {
    let row = client
        .query_opt(
            "SELECT head.current_revision_id::text, revision.generation, revision.validation,
                    revision.closure, proposal.chapter_id::text,
                    chapter_head.current_revision_id::text,
                    proposal.source_run_id::text, proposal.source_decision_id::text
               FROM storyos.proposals AS proposal
               JOIN storyos.proposal_heads AS head
                 ON (head.owner_user_id, head.project_id, head.proposal_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.revision_id) =
                    (head.owner_user_id, head.project_id, head.proposal_id,
                     head.current_revision_id)
               LEFT JOIN storyos.authoritative_heads AS chapter_head
                 ON (chapter_head.owner_user_id, chapter_head.project_id,
                     chapter_head.manuscript_object_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.chapter_id)
              WHERE proposal.owner_user_id = $1::text::uuid
                AND proposal.project_id = $2::text::uuid
                AND proposal.proposal_id = $3::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.proposal_id,
            ],
        )
        .await
        .map_err(withdraw_database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let chapter_id: Option<String> = row.get(4);
    let Some(chapter_id) = chapter_id else {
        return Err(WithdrawProposalError::MissingProject);
    };
    Ok(Some(LoadedProposal {
        current_revision_id: row.get(0),
        generation: row.get(1),
        validation: row.get(2),
        closure: row.get(3),
        chapter_id,
        current_head_revision_id: row.get(5),
        source_run_id: row.get(6),
        source_decision_id: row.get(7),
    }))
}

async fn read_producer_receipt(
    client: &tokio_postgres::Client,
    command: &WithdrawProposalCommand,
) -> Result<Option<WithdrawProposalSettlement>, WithdrawProposalError> {
    let row = client
        .query_opt(
            "SELECT receipt.command_id::text, receipt.receipt_id::text, receipt.command_digest,
                    receipt.result_kind, receipt.result_payload->>'reason',
                    to_char(receipt.created_at AT TIME ZONE 'UTC',
                            'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                    withdrawal.withdrawal_event_id::text, revision.generation, revision.validation,
                    project.title, project.current_chapter_id::text
               FROM storyos.domain_receipts AS receipt
               JOIN storyos.projects AS project
                 ON (project.owner_user_id, project.project_id) =
                    (receipt.owner_user_id, receipt.project_id)
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
                AND receipt.command_kind = 'withdrawProposal'
                AND receipt.producer_cause = 'agent_run_decision'
                AND receipt.idempotency_key = $3::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(withdraw_database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let stored_digest: String = row.get(2);
    if stored_digest != command.challenge_binding.canonical_command_digest {
        return Err(WithdrawProposalError::BindingConflict);
    }
    let result_kind: String = row.get(3);
    let reason: Option<String> = row.get(4);
    let effect = match (result_kind.as_str(), reason.as_deref()) {
        ("proposal_closure_changed", _) => {
            let withdrawal_event_id: Option<String> = row.get(6);
            let generation: Option<String> = row.get(7);
            let validation: Option<String> = row.get(8);
            match (withdrawal_event_id, generation, validation) {
                (
                    Some(withdrawal_event_id),
                    Some(preserved_generation),
                    Some(preserved_validation),
                ) => WithdrawProposalSettlementEffect::Resolved {
                    ownership: ResolvedWithdrawal::CurrentProducer,
                    preserved_generation,
                    preserved_validation,
                    withdrawal_event_id,
                },
                _ => return Err(WithdrawProposalError::HistoricalAcknowledgementUnavailable),
            }
        }
        ("conflicted", Some("changed_head")) => WithdrawProposalSettlementEffect::Conflicted {
            reason: storyos_core::WithdrawProposalConflict::ChangedHead,
        },
        ("refused", Some("wrong_scope")) => WithdrawProposalSettlementEffect::Refused {
            reason: storyos_core::WithdrawProposalRefusal::WrongScope,
        },
        ("refused", Some("wrong_admission")) => WithdrawProposalSettlementEffect::Refused {
            reason: storyos_core::WithdrawProposalRefusal::WrongAdmission,
        },
        ("refused", Some("stale_proposal_revision")) => WithdrawProposalSettlementEffect::Refused {
            reason: storyos_core::WithdrawProposalRefusal::StaleProposalRevision,
        },
        ("no_effect", Some("unsupported_cause")) => WithdrawProposalSettlementEffect::NoEffect {
            reason: storyos_core::WithdrawProposalNoEffect::UnsupportedCause,
        },
        ("no_effect", Some("terminal_supersession")) => {
            WithdrawProposalSettlementEffect::NoEffect {
                reason: storyos_core::WithdrawProposalNoEffect::TerminalSupersession,
            }
        }
        ("no_effect", Some("closure_not_open")) => WithdrawProposalSettlementEffect::NoEffect {
            reason: storyos_core::WithdrawProposalNoEffect::ClosureNotOpen,
        },
        _ => return Err(WithdrawProposalError::HistoricalAcknowledgementUnavailable),
    };
    let current_chapter_id: Option<String> = row.get(10);
    Ok(Some(WithdrawProposalSettlement {
        ids: storyos_application::AuthorCommandAdmissionIds {
            command_id: row.get(0),
            author_command_admission_id: String::new(),
            receipt_id: row.get(1),
        },
        effect,
        receipt_created_at: row.get(5),
        response_project: storyos_application::Project {
            project_id: command.project_scope.project_id.clone(),
            title: row.get(9),
            current_chapter_id: current_chapter_id.map(storyos_application::ChapterId::new),
        },
    }))
}

fn withdraw_challenge_error(error: ProjectCommandChallengeError) -> WithdrawProposalError {
    match error {
        ProjectCommandChallengeError::BindingConflict => WithdrawProposalError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired
        | ProjectCommandChallengeError::RateLimited { .. } => {
            WithdrawProposalError::InvalidChallenge
        }
        ProjectCommandChallengeError::Unavailable(source) => {
            WithdrawProposalError::Unavailable(source)
        }
    }
}

pub(super) fn withdraw_database_error(error: tokio_postgres::Error) -> WithdrawProposalError {
    WithdrawProposalError::Unavailable(Box::new(error))
}

pub(super) fn withdraw_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> WithdrawProposalError {
    WithdrawProposalError::Unavailable(Box::new(error))
}
