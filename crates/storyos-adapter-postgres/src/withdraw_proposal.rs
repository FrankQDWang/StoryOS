#[path = "withdraw_proposal_read.rs"]
mod read;
#[path = "withdraw_proposal_write.rs"]
mod write;

use read::read_withdraw_settlement;
use write::{insert_withdraw_admission, persist_resolved, persist_zero};

use super::*;
use storyos_application::{
    ProjectCommandChallengeError, ProjectCommandChallengeUse, WithdrawProposalCommand,
    WithdrawProposalError, WithdrawProposalSettlement, WithdrawProposalSettlementEffect,
    WithdrawProposalStore,
};
use storyos_core::{
    WithdrawProposal as CoreWithdraw, WithdrawProposalResult, WithdrawalCause, withdraw_proposal,
};

impl WithdrawProposalStore for PostgresProjectReader {
    async fn withdraw_proposal(
        &self,
        command: &WithdrawProposalCommand,
    ) -> Result<WithdrawProposalSettlement, WithdrawProposalError> {
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
}

async fn persist_withdraw(
    client: &tokio_postgres::Client,
    command: &WithdrawProposalCommand,
) -> Result<WithdrawProposalSettlement, WithdrawProposalError> {
    let Some(loaded) = load_proposal(client, command).await? else {
        return Err(WithdrawProposalError::MissingProject);
    };
    insert_withdraw_admission(client, command, &loaded.chapter_id).await?;
    let classified = withdraw_proposal(&CoreWithdraw {
        scope_matches: true,
        cause: WithdrawalCause::Author,
        admission_valid: true,
        producer_matches: false,
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
                    chapter_head.current_revision_id::text
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
