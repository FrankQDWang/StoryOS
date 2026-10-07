//! The inputs and settlements of the author and current-producer forms of Withdraw Proposal.

use storyos_core::{WithdrawProposalConflict, WithdrawProposalNoEffect, WithdrawProposalRefusal};

use crate::{ActionApplied, EditorSessionId, ProjectCommandSettlement};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WithdrawalNote {
    Omitted,
    Present { text: String },
}

/// One author Withdrawal of an open Proposal from the writer Editor Session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawProposalInput {
    pub editor_session_id: EditorSessionId,
    pub proposal_id: String,
    pub proposal_revision_id: String,
    pub expected_authoritative_revision_id: String,
    pub withdrawal_note: WithdrawalNote,
}

/// One Withdrawal by the AgentRun decision that produced the Proposal. It consumes no Command
/// Challenge and writes no Admission (ADR 0043).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentProducerWithdrawal {
    pub run_id: String,
    pub decision_id: String,
    pub proposal_id: String,
    pub proposal_revision_id: String,
    pub expected_authoritative_revision_id: String,
}

/// The closure change of one resolved Withdrawal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalWithdrawn {
    pub preserved_generation: String,
    pub preserved_validation: String,
    pub withdrawal_event_id: String,
}

pub type WithdrawProposalSettlement = ProjectCommandSettlement<
    ActionApplied<ProposalWithdrawn>,
    WithdrawProposalNoEffect,
    WithdrawProposalConflict,
    WithdrawProposalRefusal,
>;

pub type CurrentProducerWithdrawalSettlement = ProjectCommandSettlement<
    ProposalWithdrawn,
    WithdrawProposalNoEffect,
    WithdrawProposalConflict,
    WithdrawProposalRefusal,
>;
