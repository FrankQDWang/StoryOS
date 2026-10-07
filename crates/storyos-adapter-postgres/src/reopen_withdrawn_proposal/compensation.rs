//! The withdrawn Proposal reopen Compensation of Author Undo (ADR 0044).

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

/// Withdraws a reopened Proposal again with the Proposal Revision before the reopen.
pub(crate) struct ReopenWithdrawnCompensation;

impl CompensationAdapter for ReopenWithdrawnCompensation {
    type Forward = ();
    type Evidence = ObservedProposalDecision;

    /// A Proposal head that moved after the reopen makes it a Barrier.
    async fn load(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        _forward: (),
        sequence: u64,
    ) -> Result<Option<ObservedProposalDecision>, UndoLatestAuthorActionError> {
        let history = DecisionHistory {
            table: "proposal_withdrawal_reopenings",
            receipt_column: "reopen_receipt_id",
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
