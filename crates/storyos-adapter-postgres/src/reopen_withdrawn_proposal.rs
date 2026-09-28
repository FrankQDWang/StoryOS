#[path = "reopen_withdrawn_proposal_read.rs"]
mod read;
#[path = "reopen_withdrawn_proposal_write.rs"]
mod write;

use read::read_reopen_settlement;
use write::{insert_reopen_admission, persist_resolved, persist_zero};

use super::*;
use storyos_application::{
    ProjectCommandChallengeError, ProjectCommandChallengeUse, ReopenWithdrawnProposalCommand,
    ReopenWithdrawnProposalError, ReopenWithdrawnProposalSettlement,
    ReopenWithdrawnProposalSettlementEffect, ReopenWithdrawnProposalStore,
};
use storyos_core::{
    ReopenWithdrawnProposal as CoreReopen, ReopenWithdrawnProposalResult, reopen_withdrawn_proposal,
};

impl ReopenWithdrawnProposalStore for PostgresProjectReader {
    async fn reopen_withdrawn_proposal(
        &self,
        command: &ReopenWithdrawnProposalCommand,
    ) -> Result<ReopenWithdrawnProposalSettlement, ReopenWithdrawnProposalError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(reopen_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(reopen_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(reopen_challenge_error)?;
                read_reopen_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(reopen_challenge_error)?;
                Err(ReopenWithdrawnProposalError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_reopen(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction.commit().await.map_err(reopen_challenge_error)?;
                        Ok(settlement)
                    }
                    Err(error) => {
                        transaction
                            .rollback()
                            .await
                            .map_err(reopen_challenge_error)?;
                        Err(error)
                    }
                }
            }
        }
    }
}

pub(super) struct LoadedProposal {
    current_revision_id: String,
    generation: String,
    closure: String,
    chapter_id: String,
    candidate_text: String,
    base_authoritative_revision_id: String,
    operation_resolution: String,
    current_head_revision_id: Option<String>,
    withdrawal_event_matches: bool,
}

async fn persist_reopen(
    client: &tokio_postgres::Client,
    command: &ReopenWithdrawnProposalCommand,
) -> Result<ReopenWithdrawnProposalSettlement, ReopenWithdrawnProposalError> {
    let Some(loaded) = load_proposal(client, command).await? else {
        return Err(ReopenWithdrawnProposalError::MissingProject);
    };
    insert_reopen_admission(client, command, &loaded.chapter_id).await?;
    let classified = reopen_withdrawn_proposal(&CoreReopen {
        scope_matches: true,
        admission_valid: true,
        proposal_revision_current: loaded.current_revision_id == command.proposal_revision_id,
        closure_withdrawn: loaded.closure == "withdrawn",
        terminal_supersession: loaded.closure == "superseded",
        withdrawal_event_matches: loaded.withdrawal_event_matches,
        expected_target_matches_head: loaded.current_head_revision_id.as_deref()
            == Some(command.expected_authoritative_revision_id.as_str()),
    });
    match classified {
        ReopenWithdrawnProposalResult::Resolved => persist_resolved(client, command, &loaded).await,
        ReopenWithdrawnProposalResult::Conflicted { reason } => {
            persist_zero(
                client,
                command,
                "conflicted",
                "changed_head",
                ReopenWithdrawnProposalSettlementEffect::Conflicted { reason },
            )
            .await
        }
        ReopenWithdrawnProposalResult::Refused { reason } => {
            let reason_code = match &reason {
                storyos_core::ReopenWithdrawnProposalRefusal::WrongScope => "wrong_scope",
                storyos_core::ReopenWithdrawnProposalRefusal::WrongAdmission => "wrong_admission",
                storyos_core::ReopenWithdrawnProposalRefusal::StaleProposalRevision => {
                    "stale_proposal_revision"
                }
            };
            persist_zero(
                client,
                command,
                "refused",
                reason_code,
                ReopenWithdrawnProposalSettlementEffect::Refused { reason },
            )
            .await
        }
        ReopenWithdrawnProposalResult::NoEffect { reason } => {
            let reason_code = match &reason {
                storyos_core::ReopenWithdrawnProposalNoEffect::TerminalSupersession => {
                    "terminal_supersession"
                }
                storyos_core::ReopenWithdrawnProposalNoEffect::ClosureNotWithdrawn => {
                    "closure_not_withdrawn"
                }
                storyos_core::ReopenWithdrawnProposalNoEffect::WithdrawalEventMismatch => {
                    "withdrawal_event_mismatch"
                }
            };
            persist_zero(
                client,
                command,
                "no_effect",
                reason_code,
                ReopenWithdrawnProposalSettlementEffect::NoEffect { reason },
            )
            .await
        }
    }
}

async fn load_proposal(
    client: &tokio_postgres::Client,
    command: &ReopenWithdrawnProposalCommand,
) -> Result<Option<LoadedProposal>, ReopenWithdrawnProposalError> {
    let row = client
        .query_opt(
            "SELECT head.current_revision_id::text, revision.generation, revision.closure,
                    proposal.chapter_id::text, revision.candidate_text,
                    revision.base_authoritative_revision_id::text,
                    chapter_head.current_revision_id::text,
                    (
                      SELECT operation.resolution
                        FROM storyos.proposal_operations AS operation
                       WHERE operation.owner_user_id = proposal.owner_user_id
                         AND operation.project_id = proposal.project_id
                         AND operation.proposal_id = proposal.proposal_id
                       ORDER BY operation.operation_id
                       LIMIT 1
                    ),
                    EXISTS (
                      SELECT 1
                        FROM storyos.proposal_withdrawals AS withdrawal
                       WHERE withdrawal.owner_user_id = proposal.owner_user_id
                         AND withdrawal.project_id = proposal.project_id
                         AND withdrawal.proposal_id = proposal.proposal_id
                         AND withdrawal.proposal_revision_id = head.current_revision_id
                         AND withdrawal.withdrawal_event_id = $4::text::uuid
                    )
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
                &command.withdrawal_event_id,
            ],
        )
        .await
        .map_err(reopen_database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let chapter_id: Option<String> = row.get(3);
    let Some(chapter_id) = chapter_id else {
        return Err(ReopenWithdrawnProposalError::MissingProject);
    };
    let operation_resolution: Option<String> = row.get(7);
    let Some(operation_resolution) = operation_resolution else {
        return Err(ReopenWithdrawnProposalError::MissingProject);
    };
    Ok(Some(LoadedProposal {
        current_revision_id: row.get(0),
        generation: row.get(1),
        closure: row.get(2),
        chapter_id,
        candidate_text: row.get(4),
        base_authoritative_revision_id: row.get(5),
        current_head_revision_id: row.get(6),
        operation_resolution,
        withdrawal_event_matches: row.get(8),
    }))
}

fn reopen_challenge_error(error: ProjectCommandChallengeError) -> ReopenWithdrawnProposalError {
    match error {
        ProjectCommandChallengeError::BindingConflict => {
            ReopenWithdrawnProposalError::BindingConflict
        }
        ProjectCommandChallengeError::InvalidOrExpired
        | ProjectCommandChallengeError::RateLimited { .. } => {
            ReopenWithdrawnProposalError::InvalidChallenge
        }
        ProjectCommandChallengeError::Unavailable(source) => {
            ReopenWithdrawnProposalError::Unavailable(source)
        }
    }
}

pub(super) fn reopen_database_error(error: tokio_postgres::Error) -> ReopenWithdrawnProposalError {
    ReopenWithdrawnProposalError::Unavailable(Box::new(error))
}

pub(super) fn reopen_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> ReopenWithdrawnProposalError {
    ReopenWithdrawnProposalError::Unavailable(Box::new(error))
}
