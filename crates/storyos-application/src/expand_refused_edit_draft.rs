//! The input and settlement of the Expand Refused Edit Draft to Proposal command.

use std::convert::Infallible;

use storyos_core::{
    ExpandRefusedEditDraftConflict, ExpandRefusedEditDraftRefusal, OpenInlineProposalAnchor,
    ReplacementBlock,
};

use crate::{ActionApplied, DraftCloseObservation, EditorSessionId, ProjectCommandSettlement};

/// One author expansion of a whole retained Refused Edit Draft to one inline edit Proposal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpandRefusedEditDraftToProposalInput {
    pub editor_session_id: EditorSessionId,
    pub writer_generation: u64,
    pub draft_id: String,
    pub source_current_draft_revision_id: String,
    pub source_draft_payload_digest: String,
    pub source_reopen_event_id: Option<String>,
    pub chapter_id: String,
    pub target_ref: String,
    pub expected_target_revision_id: String,
    pub anchor: OpenInlineProposalAnchor,
}

/// The Draft and target head facts that the Domain Receipt of every expansion outcome records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftExpansionObservation {
    pub draft: DraftCloseObservation,
    pub current_target_revision_id: Option<String>,
}

/// The Proposal that an applied expansion created and the close event of its source Draft.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftExpanded {
    pub observation: DraftExpansionObservation,
    pub event_id: String,
    pub proposal_id: String,
    pub proposal_revision_id: String,
    pub candidate_blocks: Vec<ReplacementBlock>,
}

/// The zero-authority effect is the observation of a conflicted or refused expansion.
pub type ExpandRefusedEditDraftSettlement = ProjectCommandSettlement<
    ActionApplied<DraftExpanded>,
    Infallible,
    ExpandRefusedEditDraftConflict,
    ExpandRefusedEditDraftRefusal,
    (),
    DraftExpansionObservation,
>;
