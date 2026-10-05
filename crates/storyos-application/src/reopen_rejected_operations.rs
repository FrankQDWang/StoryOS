//! The input and settlement of the Reopen Rejected Operations command.

use std::convert::Infallible;

use storyos_core::{ReopenRejectedOperationsConflict, ReopenRejectedOperationsRefusal};

use crate::{ActionApplied, EditorSessionId, ProjectCommandSettlement};

/// One author reopen of a rejected Proposal Operation from the writer Editor Session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReopenRejectedOperationsInput {
    pub editor_session_id: EditorSessionId,
    pub proposal_id: String,
    pub proposal_revision_id: String,
    pub selected_rejected_operation_id: String,
    pub rejection_event_id: String,
    pub expected_authoritative_revision_id: String,
}

/// The pending Proposal Revision and the reopening event that a reopen creates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectedOperationsReopened {
    pub resulting_proposal_revision_id: String,
    pub preserved_generation: String,
    pub preserved_closure: String,
    pub state_event_id: String,
}

pub type ReopenRejectedOperationsSettlement = ProjectCommandSettlement<
    ActionApplied<RejectedOperationsReopened>,
    Infallible,
    ReopenRejectedOperationsConflict,
    ReopenRejectedOperationsRefusal,
>;
