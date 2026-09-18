#[path = "accept_proposal_read.rs"]
mod read;
#[path = "accept_proposal_write.rs"]
mod write;

use read::read_accept_settlement;
use write::{insert_accept_admission, persist_applied, persist_zero};

use super::*;
use storyos_application::{
    AcceptProposalCommand, AcceptProposalError, AcceptProposalSettlement,
    AcceptProposalSettlementEffect, AcceptProposalStore, AcceptanceRefusalBoundary,
    AcceptanceRefusalReason, ProjectCommandChallengeError, ProjectCommandChallengeUse,
};
use storyos_core::{
    AcceptProposal as CoreAccept, AcceptProposalInvalid, AcceptProposalRefusal,
    AcceptProposalResult, accept_proposal as classify,
};

impl AcceptProposalStore for PostgresProjectReader {
    async fn accept_proposal(
        &self,
        command: &AcceptProposalCommand,
    ) -> Result<AcceptProposalSettlement, AcceptProposalError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(accept_challenge_error)?;
        let challenge_use = match transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
        {
            Ok(challenge_use) => challenge_use,
            Err(ProjectCommandChallengeError::InvalidOrExpired) => {
                transaction
                    .rollback()
                    .await
                    .map_err(accept_challenge_error)?;
                let reason = self
                    .retain_acceptance_refusal(
                        command,
                        AcceptanceRefusalReason::InvalidChallenge,
                        AcceptanceRefusalBoundary::Challenge,
                    )
                    .await?;
                return Err(AcceptProposalError::PreAdmissionRefused { reason });
            }
            Err(error) => return Err(accept_challenge_error(error)),
        };
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(accept_challenge_error)?;
                read_accept_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(accept_challenge_error)?;
                Err(AcceptProposalError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_accept(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction.commit().await.map_err(accept_challenge_error)?;
                        Ok(settlement)
                    }
                    Err(error) => {
                        transaction
                            .rollback()
                            .await
                            .map_err(accept_challenge_error)?;
                        let reason = match error {
                            AcceptProposalError::PreAdmissionRefused { reason } => reason,
                            AcceptProposalError::InvalidChallenge => {
                                AcceptanceRefusalReason::InvalidChallenge
                            }
                            other => return Err(other),
                        };
                        let reason = self
                            .retain_acceptance_refusal(
                                command,
                                reason,
                                AcceptanceRefusalBoundary::WriterSession,
                            )
                            .await?;
                        Err(AcceptProposalError::PreAdmissionRefused { reason })
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
    validation_current: bool,
    candidate_text: String,
    chapter_id: String,
    operation_id: String,
    operation_resolution: String,
    receipt_id: Option<String>,
    receipt_result: Option<String>,
    receipt_revision_id: Option<String>,
    receipt_candidate_text: Option<String>,
    current_head_revision_id: Option<String>,
    validated_target_matches_head: bool,
}

async fn persist_accept(
    client: &tokio_postgres::Client,
    command: &AcceptProposalCommand,
) -> Result<AcceptProposalSettlement, AcceptProposalError> {
    let Some(loaded) = load_proposal(client, command).await? else {
        return Err(AcceptProposalError::MissingProject);
    };
    if let Some(row) = client
        .query_opt(
            "SELECT reason FROM storyos.acceptance_refusals WHERE owner_user_id = $1::text::uuid
         AND project_id = $2::text::uuid AND idempotency_key = $3::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(accept_database_error)?
    {
        let reason = crate::acceptance_refusal::parse_reason(row.get::<_, &str>(0))
            .map_err(accept_parse_error)?;
        return Err(AcceptProposalError::PreAdmissionRefused { reason });
    }
    insert_accept_admission(client, command, &loaded.chapter_id).await?;
    let classified = classify(&CoreAccept {
        scope_matches: true,
        admission_valid: true,
        proposal_revision_current: loaded.current_revision_id == command.proposal_revision_id,
        retention_retained: true,
        generation_ready: loaded.generation == "ready",
        closure_open: loaded.closure == "open",
        validation_current: loaded.validation_current,
        validation_receipt_valid: loaded.receipt_result.as_deref() == Some("valid")
            && loaded.receipt_id.as_deref() == Some(command.validation_receipt_id.as_str()),
        validation_receipt_matches_revision: loaded.receipt_revision_id.as_deref()
            == Some(command.proposal_revision_id.as_str()),
        selected_operation_pending: loaded.operation_id == command.selected_operation_id
            && loaded.operation_resolution == "pending",
        expected_target_matches_head: loaded.validated_target_matches_head
            && loaded.current_head_revision_id.as_deref()
                == Some(command.expected_authoritative_revision_id.as_str()),
        candidate_unaltered: loaded.receipt_candidate_text.as_deref()
            == Some(loaded.candidate_text.as_str()),
    });
    match classified {
        AcceptProposalResult::Applied => persist_applied(client, command, &loaded).await,
        AcceptProposalResult::Invalid { reason } => {
            persist_zero(
                client,
                command,
                "invalid",
                invalid_reason(&reason),
                AcceptProposalSettlementEffect::Invalid { reason },
            )
            .await
        }
        AcceptProposalResult::Conflicted { reason } => {
            persist_zero(
                client,
                command,
                "conflicted",
                "changed_head",
                AcceptProposalSettlementEffect::Conflicted { reason },
            )
            .await
        }
        AcceptProposalResult::Refused { reason } => {
            persist_zero(
                client,
                command,
                "refused",
                refuse_reason(&reason),
                AcceptProposalSettlementEffect::Refused { reason },
            )
            .await
        }
    }
}

async fn load_proposal(
    client: &tokio_postgres::Client,
    command: &AcceptProposalCommand,
) -> Result<Option<LoadedProposal>, AcceptProposalError> {
    let row = client
        .query_opt(
            "SELECT head.current_revision_id::text, revision.generation, revision.closure,
                    revision.candidate_text, proposal.chapter_id::text,
                    operation.operation_id::text,
                    operation.resolution, receipt.validation_receipt_id::text, receipt.result,
                    receipt.proposal_revision_id::text, receipt.candidate_text,
                    chapter_head.current_revision_id::text,
                    revision.validation = 'valid' AND NOT EXISTS (SELECT 1 FROM storyos.proposal_validation_conditions AS condition
                      WHERE (condition.owner_user_id, condition.project_id, condition.proposal_id,
                             condition.proposal_revision_id) =
                            (revision.owner_user_id, revision.project_id, revision.proposal_id, revision.revision_id)),
                    COALESCE(receipt.base_authoritative_revision_id = chapter_head.current_revision_id
                      AND revision.base_authoritative_revision_id = chapter_head.current_revision_id, false)
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
               LEFT JOIN storyos.validation_receipts AS receipt
                 ON (receipt.owner_user_id, receipt.project_id, receipt.validation_receipt_id) =
                    (proposal.owner_user_id, proposal.project_id, $4::text::uuid)
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
                &command.validation_receipt_id,
            ],
        )
        .await
        .map_err(accept_database_error)?;
    Ok(row.map(|row| LoadedProposal {
        current_revision_id: row.get(0),
        generation: row.get(1),
        closure: row.get(2),
        candidate_text: row.get(3),
        chapter_id: row.get(4),
        operation_id: row.get(5),
        operation_resolution: row.get(6),
        receipt_id: row.get(7),
        receipt_result: row.get(8),
        receipt_revision_id: row.get(9),
        receipt_candidate_text: row.get(10),
        current_head_revision_id: row.get(11),
        validation_current: row.get(12),
        validated_target_matches_head: row.get(13),
    }))
}

pub(super) fn invalid_reason(reason: &AcceptProposalInvalid) -> &'static str {
    match reason {
        AcceptProposalInvalid::InvalidValidation => "invalid_validation",
        AcceptProposalInvalid::AlteredCandidate => "altered_candidate",
    }
}

pub(super) fn refuse_reason(reason: &AcceptProposalRefusal) -> &'static str {
    match reason {
        AcceptProposalRefusal::WrongScope => "wrong_scope",
        AcceptProposalRefusal::WrongAdmission => "wrong_admission",
        AcceptProposalRefusal::StaleProposalRevision => "stale_proposal_revision",
        AcceptProposalRefusal::NotEligible => "not_eligible",
        AcceptProposalRefusal::OperationNotPending => "operation_not_pending",
    }
}

pub(super) fn accept_challenge_error(error: ProjectCommandChallengeError) -> AcceptProposalError {
    match error {
        ProjectCommandChallengeError::BindingConflict => AcceptProposalError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired
        | ProjectCommandChallengeError::RateLimited { .. } => AcceptProposalError::InvalidChallenge,
        ProjectCommandChallengeError::Unavailable(source) => {
            AcceptProposalError::Unavailable(source)
        }
    }
}

pub(super) fn accept_database_error(error: tokio_postgres::Error) -> AcceptProposalError {
    AcceptProposalError::Unavailable(Box::new(error))
}

pub(super) fn accept_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> AcceptProposalError {
    AcceptProposalError::Unavailable(Box::new(error))
}
