//! The inputs and settlements of the Complete Ready Partial Proposal and Continue Proposal
//! Generation commands.

use std::convert::Infallible;

use storyos_core::{
    CompleteReadyPartialProposalRefusal, ContinueProposalGenerationRefusal,
    ProposalGenerationConflict,
};

use crate::{ActionApplied, EditorSessionId, ProjectCommandSettlement};

/// One author completion of a ready-partial Proposal Generation from the writer Editor Session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteReadyPartialProposalInput {
    pub editor_session_id: EditorSessionId,
    pub proposal_id: String,
    pub proposal_revision_id: String,
    pub generation_id: String,
    pub expected_candidate_digest: String,
    pub last_applied_stream_seq: u64,
    pub expected_authoritative_revision_id: String,
}

/// One author continuation of a Proposal Generation from the writer Editor Session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinueProposalGenerationInput {
    pub editor_session_id: EditorSessionId,
    pub proposal_id: String,
    pub proposal_revision_id: String,
    pub prior_generation_id: String,
    pub expected_generation_state: String,
    pub expected_candidate_digest: String,
    pub selected_pending_operation_ids: Vec<String>,
    pub expected_authoritative_revision_id: String,
}

/// The Proposal Generation that a completion makes ready.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalGenerationCompleted {
    pub generation_id: String,
    pub preserved_validation: String,
    pub preserved_closure: String,
    pub preserved_operation_resolution: String,
    pub generation_event_id: String,
}

/// The new Proposal Generation that a continuation starts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalGenerationStarted {
    pub prior_generation_id: String,
    pub new_generation_id: String,
    pub prior_generation_state: String,
    pub prior_run_id: String,
    pub resulting_run_id: String,
    pub preserved_validation: String,
    pub preserved_closure: String,
    pub preserved_operation_resolution: String,
    pub generation_event_id: String,
}

pub type CompleteReadyPartialProposalSettlement = ProjectCommandSettlement<
    ActionApplied<ProposalGenerationCompleted>,
    Infallible,
    ProposalGenerationConflict,
    CompleteReadyPartialProposalRefusal,
>;

pub type ContinueProposalGenerationSettlement = ProjectCommandSettlement<
    ActionApplied<ProposalGenerationStarted>,
    Infallible,
    ProposalGenerationConflict,
    ContinueProposalGenerationRefusal,
>;
