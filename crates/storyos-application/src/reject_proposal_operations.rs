//! The input and settlement of the Reject Proposal Operations command.

use std::convert::Infallible;

use storyos_core::{RejectProposalOperationsConflict, RejectProposalOperationsRefusal};

use crate::{ActionApplied, EditorSessionId, ProjectCommandSettlement};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RejectionNote {
    Omitted,
    Present { text: String },
}

/// One author rejection of selected pending Proposal Operations from the writer Editor Session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectProposalOperationsInput {
    pub editor_session_id: EditorSessionId,
    pub proposal_id: String,
    pub proposal_revision_id: String,
    pub selected_pending_operation_ids: Vec<String>,
    pub expected_authoritative_revision_id: String,
    pub rejection_note: RejectionNote,
}

/// The first resolution event of a rejection and the Proposal State Axes of its Proposal Revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalOperationsRejected {
    pub preserved_generation: String,
    pub preserved_validation: String,
    pub preserved_closure: String,
    pub resolution_event_id: String,
}

pub type RejectProposalOperationsSettlement = ProjectCommandSettlement<
    ActionApplied<ProposalOperationsRejected>,
    Infallible,
    RejectProposalOperationsConflict,
    RejectProposalOperationsRefusal,
>;
