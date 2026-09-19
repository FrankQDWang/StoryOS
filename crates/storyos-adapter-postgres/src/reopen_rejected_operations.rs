#[path = "reopen_rejected_operations_read.rs"]
mod read;
#[path = "reopen_rejected_operations_write.rs"]
mod write;

use read::read_reopen_settlement;
use write::{insert_reopen_admission, persist_resolved, persist_zero};

use super::*;
use storyos_application::{
    ProjectCommandChallengeError, ProjectCommandChallengeUse, ReopenRejectedOperationsCommand,
    ReopenRejectedOperationsError, ReopenRejectedOperationsSettlement,
    ReopenRejectedOperationsSettlementEffect, ReopenRejectedOperationsStore,
};
use storyos_core::{
    ReopenRejectedOperations as CoreReopen, ReopenRejectedOperationsResult,
    reopen_rejected_operations as classify,
};

impl ReopenRejectedOperationsStore for PostgresProjectReader {
    async fn reopen_rejected_operations(
        &self,
        command: &ReopenRejectedOperationsCommand,
    ) -> Result<ReopenRejectedOperationsSettlement, ReopenRejectedOperationsError> {
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
                Err(ReopenRejectedOperationsError::BindingConflict)
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
    operation_id: String,
    operation_resolution: String,
    candidate_text: String,
    base_authoritative_revision_id: String,
    current_head_revision_id: Option<String>,
    rejection_event_matches: bool,
    reservation_available: bool,
}

async fn persist_reopen(
    client: &tokio_postgres::Client,
    command: &ReopenRejectedOperationsCommand,
) -> Result<ReopenRejectedOperationsSettlement, ReopenRejectedOperationsError> {
    let Some(loaded) = load_proposal(client, command).await? else {
        return Err(ReopenRejectedOperationsError::MissingProject);
    };
    insert_reopen_admission(client, command, &loaded.chapter_id).await?;
    let classified = classify(&CoreReopen {
        scope_matches: true,
        admission_valid: true,
        proposal_revision_current: loaded.current_revision_id == command.proposal_revision_id,
        closure_open: loaded.closure == "open",
        selected_operations_rejected: loaded.operation_id == command.selected_rejected_operation_id
            && loaded.operation_resolution == "rejected",
        rejection_event_matches: loaded.rejection_event_matches,
        reservation_available: loaded.reservation_available,
        expected_target_matches_head: loaded.current_head_revision_id.as_deref()
            == Some(command.expected_authoritative_revision_id.as_str()),
    });
    match classified {
        ReopenRejectedOperationsResult::Resolved => {
            persist_resolved(client, command, &loaded).await
        }
        ReopenRejectedOperationsResult::Conflicted { reason } => {
            persist_zero(
                client,
                command,
                "conflicted",
                "changed_head",
                ReopenRejectedOperationsSettlementEffect::Conflicted { reason },
            )
            .await
        }
        ReopenRejectedOperationsResult::Refused { reason } => {
            persist_zero(
                client,
                command,
                "refused",
                refuse_reason(&reason),
                ReopenRejectedOperationsSettlementEffect::Refused { reason },
            )
            .await
        }
    }
}

async fn load_proposal(
    client: &tokio_postgres::Client,
    command: &ReopenRejectedOperationsCommand,
) -> Result<Option<LoadedProposal>, ReopenRejectedOperationsError> {
    let row = client
        .query_opt(
            "SELECT head.current_revision_id::text, revision.generation, revision.closure,
                    proposal.chapter_id::text, operation.operation_id::text,
                    operation.resolution, revision.candidate_text,
                    revision.base_authoritative_revision_id::text,
                    chapter_head.current_revision_id::text,
                    EXISTS (
                      SELECT 1
                        FROM storyos.proposal_operation_resolutions AS resolution
                       WHERE resolution.owner_user_id = proposal.owner_user_id
                         AND resolution.project_id = proposal.project_id
                         AND resolution.resolution_event_id = $4::text::uuid
                         AND resolution.proposal_id = proposal.proposal_id
                         AND resolution.operation_id = $5::text::uuid
                         AND resolution.resulting_resolution = 'rejected'
                         AND NOT EXISTS (
                           SELECT 1
                             FROM storyos.proposal_operation_reopenings AS reopening
                            WHERE reopening.owner_user_id = resolution.owner_user_id
                              AND reopening.project_id = resolution.project_id
                              AND reopening.rejection_event_id = resolution.resolution_event_id
                         )
                    ),
                    NOT EXISTS (
                      SELECT 1
                        FROM storyos.proposal_operations AS reserved
                       WHERE reserved.owner_user_id = proposal.owner_user_id
                         AND reserved.project_id = proposal.project_id
                         AND reserved.manuscript_block_id = operation.manuscript_block_id
                         AND reserved.reservation_state = 'unresolved'
                         AND reserved.proposal_id <> proposal.proposal_id
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
               JOIN storyos.proposal_operations AS operation
                 ON (operation.owner_user_id, operation.project_id, operation.proposal_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
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
                &command.rejection_event_id,
                &command.selected_rejected_operation_id,
            ],
        )
        .await
        .map_err(reopen_database_error)?;
    Ok(row.map(|row| LoadedProposal {
        current_revision_id: row.get(0),
        generation: row.get(1),
        closure: row.get(2),
        chapter_id: row.get(3),
        operation_id: row.get(4),
        operation_resolution: row.get(5),
        candidate_text: row.get(6),
        base_authoritative_revision_id: row.get(7),
        current_head_revision_id: row.get(8),
        rejection_event_matches: row.get(9),
        reservation_available: row.get(10),
    }))
}

pub(super) fn refuse_reason(
    reason: &storyos_core::ReopenRejectedOperationsRefusal,
) -> &'static str {
    match reason {
        storyos_core::ReopenRejectedOperationsRefusal::WrongScope => "wrong_scope",
        storyos_core::ReopenRejectedOperationsRefusal::WrongAdmission => "wrong_admission",
        storyos_core::ReopenRejectedOperationsRefusal::StaleProposalRevision => {
            "stale_proposal_revision"
        }
        storyos_core::ReopenRejectedOperationsRefusal::NotEligible => "not_eligible",
        storyos_core::ReopenRejectedOperationsRefusal::OperationNotRejected => {
            "operation_not_rejected"
        }
        storyos_core::ReopenRejectedOperationsRefusal::UnavailableProof => "unavailable_proof",
    }
}

pub(super) fn reopen_challenge_error(
    error: ProjectCommandChallengeError,
) -> ReopenRejectedOperationsError {
    match error {
        ProjectCommandChallengeError::BindingConflict => {
            ReopenRejectedOperationsError::BindingConflict
        }
        ProjectCommandChallengeError::InvalidOrExpired
        | ProjectCommandChallengeError::RateLimited { .. } => {
            ReopenRejectedOperationsError::InvalidChallenge
        }
        ProjectCommandChallengeError::Unavailable(source) => {
            ReopenRejectedOperationsError::Unavailable(source)
        }
    }
}

pub(super) fn reopen_database_error(error: tokio_postgres::Error) -> ReopenRejectedOperationsError {
    ReopenRejectedOperationsError::Unavailable(Box::new(error))
}

pub(super) fn reopen_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> ReopenRejectedOperationsError {
    ReopenRejectedOperationsError::Unavailable(Box::new(error))
}
