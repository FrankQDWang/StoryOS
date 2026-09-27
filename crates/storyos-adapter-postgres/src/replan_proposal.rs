#[path = "replan_proposal_read.rs"]
mod read;
#[path = "replan_proposal_write.rs"]
mod write;

use read::read_replan_settlement;
use write::{insert_replan_admission, persist_resolved, persist_zero};

use super::*;
use storyos_application::{
    ProjectCommandChallengeError, ProjectCommandChallengeUse, ReplanProposalCommand,
    ReplanProposalError, ReplanProposalSettlement, ReplanProposalSettlementEffect,
    ReplanProposalStore,
};
use storyos_core::{
    ReplanProposal as CoreReplan, ReplanProposalResult, replan_proposal as classify,
};

impl ReplanProposalStore for PostgresProjectReader {
    async fn replan_proposal(
        &self,
        command: &ReplanProposalCommand,
    ) -> Result<ReplanProposalSettlement, ReplanProposalError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(replan_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(replan_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(replan_challenge_error)?;
                read_replan_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(replan_challenge_error)?;
                Err(ReplanProposalError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_replan(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction.commit().await.map_err(replan_challenge_error)?;
                        Ok(settlement)
                    }
                    Err(error) => {
                        transaction
                            .rollback()
                            .await
                            .map_err(replan_challenge_error)?;
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
    candidate_blocks: Option<String>,
    current_head_revision_id: Option<String>,
    source_condition_matches: bool,
    operation_identity_matches: bool,
}

async fn persist_replan(
    client: &tokio_postgres::Client,
    command: &ReplanProposalCommand,
) -> Result<ReplanProposalSettlement, ReplanProposalError> {
    let Some(loaded) = load_proposal(client, command).await? else {
        return Err(ReplanProposalError::MissingProject);
    };
    insert_replan_admission(client, command, &loaded.chapter_id).await?;
    let classified = classify(&CoreReplan {
        scope_matches: true,
        admission_valid: true,
        proposal_revision_current: loaded.current_revision_id
            == command.conflicted_proposal_revision_id,
        expected_head_current: loaded.current_revision_id == command.expected_current_proposal_head,
        closure_open: loaded.closure == "open",
        source_condition_matches: loaded.source_condition_matches,
        replacement_operations_preserve_identity: loaded.operation_identity_matches,
        expected_target_matches_head: loaded.current_head_revision_id.as_deref()
            == Some(command.expected_authoritative_revision_id.as_str()),
    });
    match classified {
        ReplanProposalResult::Resolved => persist_resolved(client, command, &loaded).await,
        ReplanProposalResult::Conflicted { reason } => {
            persist_zero(
                client,
                command,
                "conflicted",
                "changed_head",
                ReplanProposalSettlementEffect::Conflicted { reason },
            )
            .await
        }
        ReplanProposalResult::Refused { reason } => {
            persist_zero(
                client,
                command,
                "refused",
                refuse_reason(&reason),
                ReplanProposalSettlementEffect::Refused { reason },
            )
            .await
        }
    }
}

async fn load_proposal(
    client: &tokio_postgres::Client,
    command: &ReplanProposalCommand,
) -> Result<Option<LoadedProposal>, ReplanProposalError> {
    let (condition_kind, condition_ref) = source_condition_parts(&command.source_condition);
    let row = client
        .query_opt(
            "SELECT head.current_revision_id::text, revision.generation, revision.closure,
                    proposal.chapter_id::text, revision.candidate_text,
                    revision.candidate_blocks::text,
                    chapter_head.current_revision_id::text,
                    EXISTS (
                      SELECT 1
                        FROM storyos.proposal_validation_conditions AS condition
                       WHERE (condition.owner_user_id, condition.project_id,
                              condition.proposal_id, condition.proposal_revision_id) =
                             (revision.owner_user_id, revision.project_id,
                              revision.proposal_id, revision.revision_id)
                         AND condition.validation = 'conflicted'
                         AND condition.condition_kind = $4
                         AND condition.conflict_id = $5::text::uuid
                    ),
                    EXISTS (
                      SELECT 1
                        FROM storyos.proposal_operations AS operation
                       WHERE (operation.owner_user_id, operation.project_id,
                              operation.proposal_id) =
                             (proposal.owner_user_id, proposal.project_id,
                              proposal.proposal_id)
                         AND operation.operation_id = $6::text::uuid
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
                &condition_kind,
                &condition_ref,
                &command.replacement_operation_id,
            ],
        )
        .await
        .map_err(replan_database_error)?;
    Ok(row.map(|row| LoadedProposal {
        current_revision_id: row.get(0),
        generation: row.get(1),
        closure: row.get(2),
        chapter_id: row.get(3),
        candidate_text: row.get(4),
        candidate_blocks: row.get(5),
        current_head_revision_id: row.get(6),
        source_condition_matches: row.get(7),
        operation_identity_matches: row.get(8),
    }))
}

pub(super) fn source_condition_parts(
    condition: &storyos_contracts::ReplanSourceCondition,
) -> (&'static str, &str) {
    match condition {
        storyos_contracts::ReplanSourceCondition::ProposalConflict {
            proposal_conflict_ref,
        } => ("proposal_conflict", proposal_conflict_ref.as_str()),
        storyos_contracts::ReplanSourceCondition::ProposalRecoveryConflict {
            proposal_recovery_conflict_ref,
        } => (
            "proposal_recovery_conflict",
            proposal_recovery_conflict_ref.as_str(),
        ),
    }
}

pub(super) fn refuse_reason(reason: &storyos_core::ReplanProposalRefusal) -> &'static str {
    match reason {
        storyos_core::ReplanProposalRefusal::WrongScope => "wrong_scope",
        storyos_core::ReplanProposalRefusal::WrongAdmission => "wrong_admission",
        storyos_core::ReplanProposalRefusal::StaleProposalRevision => "stale_proposal_revision",
        storyos_core::ReplanProposalRefusal::NotEligible => "not_eligible",
        storyos_core::ReplanProposalRefusal::UnavailableProof => "unavailable_proof",
    }
}

pub(super) fn replan_challenge_error(error: ProjectCommandChallengeError) -> ReplanProposalError {
    match error {
        ProjectCommandChallengeError::BindingConflict => ReplanProposalError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired
        | ProjectCommandChallengeError::RateLimited { .. } => ReplanProposalError::InvalidChallenge,
        ProjectCommandChallengeError::Unavailable(source) => {
            ReplanProposalError::Unavailable(source)
        }
    }
}

pub(super) fn replan_database_error(error: tokio_postgres::Error) -> ReplanProposalError {
    ReplanProposalError::Unavailable(Box::new(error))
}

pub(super) fn replan_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> ReplanProposalError {
    ReplanProposalError::Unavailable(Box::new(error))
}
