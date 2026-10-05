//! The input and settlement of the Close Editor Flow Draft command.

use std::convert::Infallible;

use storyos_core::{CloseEditorFlowDraftConflict, CloseEditorFlowDraftRefusal};

use crate::{ActionApplied, EditorSessionId, ProjectCommandSettlement};

/// One author Discard of an open Refused Edit Draft from the writer Editor Session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloseEditorFlowDraftInput {
    pub editor_session_id: EditorSessionId,
    pub writer_generation: u64,
    pub draft_id: String,
    pub source_current_draft_revision_id: String,
    pub source_draft_payload_digest: String,
    pub source_reopen_event_id: Option<String>,
}

/// The current Refused Edit Draft facts that the Domain Receipt of every Discard outcome records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftCloseObservation {
    pub draft_revision_id: String,
    pub payload_digest: String,
    pub observed_closure: String,
}

/// The close event of an applied Discard and the Draft facts that it closed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftClosed {
    pub observation: DraftCloseObservation,
    pub event_id: String,
}

/// The zero-authority effect is the observed Draft of a conflicted or refused Discard.
pub type CloseEditorFlowDraftSettlement = ProjectCommandSettlement<
    ActionApplied<DraftClosed>,
    Infallible,
    CloseEditorFlowDraftConflict,
    CloseEditorFlowDraftRefusal,
    (),
    DraftCloseObservation,
>;
