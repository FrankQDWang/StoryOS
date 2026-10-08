//! The withdrawn Proposal reopen Compensation of Author Undo (ADR 0044).

use storyos_application::{ProjectCommandError, ProjectScope, UndoRecords};
use storyos_core::AuthorUndoFrontierKind;
use tokio_postgres::Client;

use crate::author_edit_proposal::decode_proposal_compensation;
use crate::command_replay::ReplayFault;
use crate::proposal_decision_compensation::{
    DecisionHistory, ObservedProposalDecision, compensate_proposal_decision,
    decision_receipt_payload, load_proposal_decision,
};
use crate::undo_compensation::{
    CompensationAction, CompensationAdapter, CompensationReplay, UndoRequest,
    allocate_compensation_action,
};

/// Withdraws a reopened Proposal again with the Proposal Revision before the reopen.
pub(crate) struct ReopenWithdrawnCompensation;

impl CompensationAdapter for ReopenWithdrawnCompensation {
    type Forward = ();
    type Evidence = ObservedProposalDecision;
    type Sequences = CompensationAction;

    /// A Proposal head that moved after the reopen makes it a Barrier.
    async fn load(
        client: &Client,
        command: &UndoRequest,
        _forward: (),
        sequence: u64,
    ) -> Result<Option<ObservedProposalDecision>, ProjectCommandError> {
        let history = DecisionHistory {
            table: "proposal_withdrawal_reopenings",
            receipt_column: "reopen_receipt_id",
        };
        load_proposal_decision(client, command, sequence, history).await
    }

    fn frontier_kind(_evidence: &ObservedProposalDecision) -> AuthorUndoFrontierKind {
        AuthorUndoFrontierKind::ReversibleStructureTransition
    }

    async fn allocate(
        client: &Client,
        scope: &ProjectScope,
    ) -> Result<CompensationAction, ProjectCommandError> {
        allocate_compensation_action(client, scope).await
    }

    fn receipt_payload(
        evidence: &ObservedProposalDecision,
        project_activity_position: u64,
    ) -> serde_json::Map<String, serde_json::Value> {
        decision_receipt_payload(evidence, project_activity_position)
    }

    async fn compensate(
        client: &Client,
        command: &UndoRequest,
        evidence: &ObservedProposalDecision,
        sequences: CompensationAction,
        source_sequence: u64,
    ) -> Result<UndoRecords, ProjectCommandError> {
        compensate_proposal_decision(client, command, evidence, sequences, source_sequence).await
    }

    fn decode(replay: &CompensationReplay) -> Result<UndoRecords, ReplayFault> {
        decode_proposal_compensation(replay, replay.restored_proposal_revision_id.clone())
    }
}
