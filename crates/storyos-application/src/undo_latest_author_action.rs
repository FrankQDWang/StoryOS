use std::convert::Infallible;

use storyos_core::{UndoLatestAuthorActionConflict, UndoLatestAuthorActionUnavailable};

use crate::{EditorSessionId, ManuscriptBlock, Project, ProjectCommandSettlement};

/// The typed input of one Undo Latest Author Action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UndoLatestAuthorActionInput {
    pub editor_session_id: EditorSessionId,
    pub expected_author_undo_frontier_sequence: u64,
    pub expected_authoritative_revision_id: String,
}

/// The settled Author Undo. The zero-authority effect gives the Author Undo Frontier.
pub type UndoLatestAuthorActionSettlement = ProjectCommandSettlement<
    UndoApplied<()>,
    Infallible,
    UndoLatestAuthorActionConflict,
    UndoLatestAuthorActionUnavailable,
    Project,
    AuthorUndoFrontierPosition,
>;

/// The applied value of an Author Undo: the source Forward action, the new Author Action, and
/// the records of its compensation family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UndoApplied<E> {
    pub effect: E,
    pub source_sequence: u64,
    pub author_action_sequence: u64,
    /// The Author Undo Frontier after the settlement. A Reversal Proposal gives none.
    pub author_undo_frontier_sequence: Option<u64>,
    /// The Refused Edit Draft that the Undo reopens because the source superseded it.
    pub source_reopen_event: Option<storyos_contracts::EditorFlowDraftReopened>,
    pub records: UndoRecords,
}

/// The records of one applied Author Undo, for each compensation family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UndoRecords {
    /// A prose or Acceptance Compensation. An Acceptance Compensation can link its Proposal.
    Revision {
        authoritative_commit_id: String,
        revision_id: String,
        body: String,
        blocks: Vec<ManuscriptBlock>,
        project_activity_position: u64,
        proposal_id: Option<String>,
        proposal_revision_id: Option<String>,
    },
    Structure {
        authoritative_commit_id: String,
        snapshot_id: String,
        project_activity_position: u64,
    },
    CurrentChapter {
        snapshot_id: String,
        project_activity_position: u64,
    },
    /// A Compensation that appends a Proposal Revision.
    Proposal {
        proposal_revision_id: Option<String>,
        project_activity_position: u64,
    },
    Draft {
        event: Box<storyos_contracts::EditorFlowDraftReopened>,
    },
    /// The Forward Author Action of an Undo Acceptance that requires a Reversal Proposal.
    ReversalRequired {
        proposal_id: String,
        proposal_revision_id: String,
    },
}

/// The Author Undo Frontier that a zero-authority Author Undo observed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorUndoFrontierPosition {
    pub current_author_undo_frontier_sequence: Option<u64>,
}
