//! The input and settlement of the Replan Proposal command.

use std::convert::Infallible;

use storyos_core::{ReplanProposalConflict, ReplanProposalRefusal};

use crate::{ActionApplied, EditorSessionId, ProjectCommandSettlement};

/// One author Replan of a conflicted Proposal from the writer Editor Session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplanProposalInput {
    pub editor_session_id: EditorSessionId,
    pub proposal_id: String,
    pub conflicted_proposal_revision_id: String,
    pub expected_current_proposal_head: String,
    pub expected_authoritative_revision_id: String,
    pub replacement_operation_id: String,
    pub source_condition: storyos_contracts::ReplanSourceCondition,
}

/// The pending Proposal Revision and the replan event that a Replan creates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalReplanned {
    pub resulting_proposal_revision_id: String,
    pub preserved_generation: String,
    pub preserved_closure: String,
    pub state_event_id: String,
}

pub type ReplanProposalSettlement = ProjectCommandSettlement<
    ActionApplied<ProposalReplanned>,
    Infallible,
    ReplanProposalConflict,
    ReplanProposalRefusal,
>;
