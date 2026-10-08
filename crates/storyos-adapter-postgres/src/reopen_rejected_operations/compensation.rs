//! The rejected operations reopen Compensation of Author Undo (ADR 0044).

use storyos_application::{ProjectCommandError, ProjectScope, UndoRecords};
use storyos_core::AuthorUndoFrontierKind;
use tokio_postgres::Client;

use crate::author_edit_proposal::decode_proposal_compensation;
use crate::command_replay::ReplayFault;
use crate::proposal_decision_compensation::{
    ObservedProposalDecision, compensate_proposal_decision, decision_receipt_payload,
};
use crate::undo_compensation::{
    CompensationAction, CompensationAdapter, CompensationReplay, UndoRequest,
    allocate_compensation_action,
};
use crate::undo_latest_author_action::undo_database_error;

/// Puts reopened operations back to `rejected` with the Proposal Revision before the reopen.
pub(crate) struct ReopenRejectedCompensation;

pub(crate) struct ObservedOperationReopening {
    pub decision: ObservedProposalDecision,
    pub operation_ids: Vec<String>,
}

impl CompensationAdapter for ReopenRejectedCompensation {
    type Forward = ();
    type Evidence = ObservedOperationReopening;
    type Sequences = CompensationAction;

    /// A Proposal head or a reopened operation that changed after the reopen makes it a Barrier.
    async fn load(
        client: &Client,
        command: &UndoRequest,
        _forward: (),
        sequence: u64,
    ) -> Result<Option<ObservedOperationReopening>, ProjectCommandError> {
        let rows = client
            .query(
                "SELECT reopening.proposal_id::text, proposal.chapter_id::text,
                        reopening.source_proposal_revision_id::text,
                        reopening.resulting_proposal_revision_id::text,
                        reopening.operation_id::text,
                        operation.resolution = 'pending'
                          AND operation.reservation_state = 'unresolved'
                   FROM storyos.author_action_entries AS action
                   JOIN storyos.proposal_operation_reopenings AS reopening
                     ON (reopening.owner_user_id, reopening.project_id,
                         reopening.reopen_receipt_id, reopening.author_action_sequence) =
                        (action.owner_user_id, action.project_id, action.receipt_id,
                         action.author_action_sequence)
                   JOIN storyos.proposals AS proposal
                     ON (proposal.owner_user_id, proposal.project_id, proposal.proposal_id) =
                        (reopening.owner_user_id, reopening.project_id, reopening.proposal_id)
                   JOIN storyos.proposal_heads AS head
                     ON (head.owner_user_id, head.project_id, head.proposal_id,
                         head.current_revision_id) =
                        (reopening.owner_user_id, reopening.project_id, reopening.proposal_id,
                         reopening.resulting_proposal_revision_id)
                   JOIN storyos.proposal_operations AS operation
                     ON (operation.owner_user_id, operation.project_id, operation.proposal_id,
                         operation.operation_id) =
                        (reopening.owner_user_id, reopening.project_id, reopening.proposal_id,
                         reopening.operation_id)
                  WHERE action.owner_user_id = $1::text::uuid
                    AND action.project_id = $2::text::uuid
                    AND action.author_action_sequence = $3::text::numeric
                    AND action.disposition = 'forward'
                    AND proposal.chapter_id IS NOT NULL
                  ORDER BY reopening.operation_id
                    FOR UPDATE OF head, operation",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &sequence.to_string(),
                ],
            )
            .await
            .map_err(undo_database_error)?;
        let Some(first) = rows.first() else {
            return Ok(None);
        };
        if !rows.iter().all(|row| row.get::<_, bool>(/*idx*/ 5)) {
            return Ok(None);
        }
        Ok(Some(ObservedOperationReopening {
            decision: ObservedProposalDecision::from_row(first, sequence),
            operation_ids: rows.iter().map(|row| row.get(/*idx*/ 4)).collect(),
        }))
    }

    fn frontier_kind(_evidence: &ObservedOperationReopening) -> AuthorUndoFrontierKind {
        AuthorUndoFrontierKind::ReversibleStructureTransition
    }

    async fn allocate(
        client: &Client,
        scope: &ProjectScope,
    ) -> Result<CompensationAction, ProjectCommandError> {
        allocate_compensation_action(client, scope).await
    }

    fn receipt_payload(
        evidence: &ObservedOperationReopening,
        project_activity_position: u64,
    ) -> serde_json::Map<String, serde_json::Value> {
        decision_receipt_payload(&evidence.decision, project_activity_position)
    }

    async fn compensate(
        client: &Client,
        command: &UndoRequest,
        evidence: &ObservedOperationReopening,
        sequences: CompensationAction,
        source_sequence: u64,
    ) -> Result<UndoRecords, ProjectCommandError> {
        let rejected = client
            .execute(
                "UPDATE storyos.proposal_operations
                    SET resolution = 'rejected', reservation_state = 'resolved'
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid
                    AND operation_id = ANY($4::text[]::uuid[])
                    AND resolution = 'pending' AND reservation_state = 'unresolved'",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &evidence.decision.proposal_id,
                    &evidence.operation_ids,
                ],
            )
            .await
            .map_err(undo_database_error)?;
        if rejected != evidence.operation_ids.len() as u64 {
            return Err(ProjectCommandError::BindingConflict);
        }
        compensate_proposal_decision(
            client,
            command,
            &evidence.decision,
            sequences,
            source_sequence,
        )
        .await
    }

    fn decode(replay: &CompensationReplay) -> Result<UndoRecords, ReplayFault> {
        decode_proposal_compensation(replay, replay.restored_proposal_revision_id.clone())
    }
}
