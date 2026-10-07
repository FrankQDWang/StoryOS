//! The input and settlement of the Reopen Withdrawn Proposal command.

use storyos_core::{
    ReopenWithdrawnProposalConflict, ReopenWithdrawnProposalNoEffect,
    ReopenWithdrawnProposalRefusal,
};

use crate::{ActionApplied, EditorSessionId, ProjectCommandSettlement};

/// One author reopen of a withdrawn Proposal from the writer Editor Session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReopenWithdrawnProposalInput {
    pub editor_session_id: EditorSessionId,
    pub proposal_id: String,
    pub proposal_revision_id: String,
    pub withdrawal_event_id: String,
    pub expected_authoritative_revision_id: String,
}

/// The pending open Proposal Revision that a reopen creates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalReopened {
    pub resulting_proposal_revision_id: String,
    pub preserved_generation: String,
    pub preserved_operation_resolution: String,
}

pub type ReopenWithdrawnProposalSettlement = ProjectCommandSettlement<
    ActionApplied<ProposalReopened>,
    ReopenWithdrawnProposalNoEffect,
    ReopenWithdrawnProposalConflict,
    ReopenWithdrawnProposalRefusal,
>;
