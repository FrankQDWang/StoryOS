//! The replan Compensation of Author Undo (ADR 0044).

use storyos_application::{
    UndoLatestAuthorActionCommand, UndoLatestAuthorActionError, UndoLatestAuthorActionSettlement,
    UndoLatestAuthorActionSettlementEffect,
};
use storyos_core::AuthorUndoFrontierKind;
use tokio_postgres::Client;

use crate::author_edit_proposal::ProposalEditCompensation;
use crate::proposal_decision_compensation::{
    DecisionHistory, ObservedProposalDecision, compensate_proposal_decision, load_proposal_decision,
};
use crate::undo_compensation::{CompensationAdapter, CompensationReplay};

/// Restores the Proposal Revision before a Replan: base, candidate, generation, closure, and validation.
pub(crate) struct ReplanCompensation;

impl CompensationAdapter for ReplanCompensation {
    type Forward = ();
    type Evidence = ObservedProposalDecision;

    /// A Proposal head that moved after the Replan makes it a Barrier.
    async fn load(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        _forward: (),
        sequence: u64,
    ) -> Result<Option<ObservedProposalDecision>, UndoLatestAuthorActionError> {
        let history = DecisionHistory {
            table: "proposal_replans",
            receipt_column: "replan_receipt_id",
        };
        load_proposal_decision(client, command, sequence, history).await
    }

    fn frontier_kind(_evidence: &ObservedProposalDecision) -> AuthorUndoFrontierKind {
        AuthorUndoFrontierKind::ReversibleStructureTransition
    }

    async fn compensate(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        evidence: &ObservedProposalDecision,
        source_sequence: u64,
    ) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
        compensate_proposal_decision(client, command, evidence, source_sequence).await
    }

    async fn decode(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        replay: &CompensationReplay,
    ) -> Result<UndoLatestAuthorActionSettlementEffect, UndoLatestAuthorActionError> {
        ProposalEditCompensation::decode(client, command, replay).await
    }
}
