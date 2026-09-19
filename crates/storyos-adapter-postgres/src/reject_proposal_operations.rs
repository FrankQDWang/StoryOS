#[path = "reject_proposal_operations_read.rs"]
mod read;
#[path = "reject_proposal_operations_write.rs"]
mod write;

use read::read_reject_settlement;
use write::{insert_reject_admission, persist_resolved, persist_zero};

use super::*;
use storyos_application::{
    ProjectCommandChallengeError, ProjectCommandChallengeUse, RejectProposalOperationsCommand,
    RejectProposalOperationsError, RejectProposalOperationsSettlement,
    RejectProposalOperationsSettlementEffect, RejectProposalOperationsStore,
};
use storyos_core::{
    ProposalBundlePolicy, ProposalOperationSelection, ProposalSelectionIntent,
    RejectProposalOperations as CoreReject, RejectProposalOperationsResult,
    classify_proposal_selection, reject_proposal_operations as classify,
};

impl RejectProposalOperationsStore for PostgresProjectReader {
    async fn reject_proposal_operations(
        &self,
        command: &RejectProposalOperationsCommand,
    ) -> Result<RejectProposalOperationsSettlement, RejectProposalOperationsError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(reject_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(reject_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(reject_challenge_error)?;
                read_reject_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(reject_challenge_error)?;
                Err(RejectProposalOperationsError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_reject(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction.commit().await.map_err(reject_challenge_error)?;
                        Ok(settlement)
                    }
                    Err(error) => {
                        transaction
                            .rollback()
                            .await
                            .map_err(reject_challenge_error)?;
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
    validation: String,
    closure: String,
    chapter_id: String,
    bundle_policy: String,
    current_head_revision_id: Option<String>,
}

async fn persist_reject(
    client: &tokio_postgres::Client,
    command: &RejectProposalOperationsCommand,
) -> Result<RejectProposalOperationsSettlement, RejectProposalOperationsError> {
    let Some(loaded) = load_proposal(client, command).await? else {
        return Err(RejectProposalOperationsError::MissingProject);
    };
    insert_reject_admission(client, command, &loaded.chapter_id).await?;
    let operations = load_operations(client, command).await?;
    let selection = classify_proposal_selection(
        &command.selected_pending_operation_ids,
        &operations,
        ProposalBundlePolicy::from_stored(&loaded.bundle_policy),
        ProposalSelectionIntent::Reject,
    );
    let classified = classify(&CoreReject {
        scope_matches: true,
        admission_valid: true,
        proposal_revision_current: loaded.current_revision_id == command.proposal_revision_id,
        closure_open: loaded.closure == "open",
        selected_operations_pending: selection.all_selected_pending,
        selection_duplicate_free: selection.duplicate_free,
        required_dependencies_met: selection.required_dependencies_met,
        bundle_closure_complete: selection.bundle_closure_complete,
        expected_target_matches_head: loaded.current_head_revision_id.as_deref()
            == Some(command.expected_authoritative_revision_id.as_str()),
    });
    match classified {
        RejectProposalOperationsResult::Resolved => {
            persist_resolved(client, command, &loaded).await
        }
        RejectProposalOperationsResult::Conflicted { reason } => {
            persist_zero(
                client,
                command,
                "conflicted",
                "changed_head",
                RejectProposalOperationsSettlementEffect::Conflicted { reason },
            )
            .await
        }
        RejectProposalOperationsResult::Refused { reason } => {
            persist_zero(
                client,
                command,
                "refused",
                refuse_reason(&reason),
                RejectProposalOperationsSettlementEffect::Refused { reason },
            )
            .await
        }
    }
}

async fn load_proposal(
    client: &tokio_postgres::Client,
    command: &RejectProposalOperationsCommand,
) -> Result<Option<LoadedProposal>, RejectProposalOperationsError> {
    let row = client
        .query_opt(
            "SELECT head.current_revision_id::text, revision.generation, revision.validation,
                    revision.closure, proposal.chapter_id::text,
                    chapter_head.current_revision_id::text, proposal.bundle_policy
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
        .map_err(reject_database_error)?;
    Ok(row.map(|row| LoadedProposal {
        current_revision_id: row.get(0),
        generation: row.get(1),
        validation: row.get(2),
        closure: row.get(3),
        chapter_id: row.get(4),
        current_head_revision_id: row.get(5),
        bundle_policy: row.get(6),
    }))
}

async fn load_operations(
    client: &tokio_postgres::Client,
    command: &RejectProposalOperationsCommand,
) -> Result<Vec<ProposalOperationSelection>, RejectProposalOperationsError> {
    let rows = client
        .query(
            "SELECT operation_id::text, resolution,
                    COALESCE(predecessor_operation_ids::text[], '{}')
               FROM storyos.proposal_operations
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid
              ORDER BY operation_id",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.proposal_id,
            ],
        )
        .await
        .map_err(reject_database_error)?;
    Ok(rows
        .into_iter()
        .map(|row| ProposalOperationSelection {
            operation_id: row.get(0),
            resolution: row.get(1),
            predecessor_operation_ids: row.get(2),
        })
        .collect())
}

pub(super) fn refuse_reason(
    reason: &storyos_core::RejectProposalOperationsRefusal,
) -> &'static str {
    match reason {
        storyos_core::RejectProposalOperationsRefusal::WrongScope => "wrong_scope",
        storyos_core::RejectProposalOperationsRefusal::WrongAdmission => "wrong_admission",
        storyos_core::RejectProposalOperationsRefusal::StaleProposalRevision => {
            "stale_proposal_revision"
        }
        storyos_core::RejectProposalOperationsRefusal::NotEligible => "not_eligible",
        storyos_core::RejectProposalOperationsRefusal::OperationNotPending => {
            "operation_not_pending"
        }
        storyos_core::RejectProposalOperationsRefusal::DuplicateIdentities => {
            "duplicate_identities"
        }
        storyos_core::RejectProposalOperationsRefusal::MissingRequiredDependencies => {
            "missing_required_dependencies"
        }
        storyos_core::RejectProposalOperationsRefusal::IncompleteBundleClosure => {
            "incomplete_bundle_closure"
        }
    }
}

pub(super) fn reject_challenge_error(
    error: ProjectCommandChallengeError,
) -> RejectProposalOperationsError {
    match error {
        ProjectCommandChallengeError::BindingConflict => {
            RejectProposalOperationsError::BindingConflict
        }
        ProjectCommandChallengeError::InvalidOrExpired
        | ProjectCommandChallengeError::RateLimited { .. } => {
            RejectProposalOperationsError::InvalidChallenge
        }
        ProjectCommandChallengeError::Unavailable(source) => {
            RejectProposalOperationsError::Unavailable(source)
        }
    }
}

pub(super) fn reject_database_error(error: tokio_postgres::Error) -> RejectProposalOperationsError {
    RejectProposalOperationsError::Unavailable(Box::new(error))
}

pub(super) fn reject_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> RejectProposalOperationsError {
    RejectProposalOperationsError::Unavailable(Box::new(error))
}
