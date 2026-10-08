//! Pure Core classification for Undo Latest Author Action.

use std::convert::Infallible;

use crate::TransitionOutcome;
use crate::transition_outcome::reason_codes;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UndoLatestAuthorAction {
    pub expected_author_undo_frontier_sequence: u64,
    pub current_author_undo_frontier: Option<AuthorUndoFrontier>,
    pub expected_head_revision_id: String,
    pub current_head_revision_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorUndoFrontier {
    pub sequence: u64,
    pub kind: AuthorUndoFrontierKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorUndoFrontierKind {
    ReversibleDirectAuthorAction {
        resulting_revision_id: String,
    },
    ReversibleAcceptance {
        resulting_revision_id: String,
        prior_evidence_usable: bool,
    },
    ReversibleStructureTransition,
    ReversibleDraftClose,
    DraftSourceUnavailable,
    DraftBindingChanged,
    Barrier,
}

/// The applied variant of one Author Undo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UndoLatestAuthorActionApplied {
    Compensated { source_sequence: u64 },
    ReversalRequired { source_sequence: u64 },
}

/// The Core Transition Outcome of one Author Undo. An unavailable Undo records `refused`.
pub type UndoLatestAuthorActionOutcome = TransitionOutcome<
    UndoLatestAuthorActionApplied,
    Infallible,
    UndoLatestAuthorActionConflict,
    UndoLatestAuthorActionUnavailable,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UndoLatestAuthorActionConflict {
    FrontierMismatch,
    WrongTargetHead,
    SourceBindingChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UndoLatestAuthorActionUnavailable {
    NoFrontier,
    Barrier,
    SourceUnavailable,
}

reason_codes!(UndoLatestAuthorActionConflict {
    FrontierMismatch => "frontier_mismatch",
    WrongTargetHead => "wrong_target_head",
    SourceBindingChanged => "source_binding_changed",
});
reason_codes!(UndoLatestAuthorActionUnavailable {
    NoFrontier => "no_frontier",
    Barrier => "barrier",
    SourceUnavailable => "source_unavailable",
});

/// Classify one Undo Latest Author Action against the derived Author Undo Frontier.
pub fn undo_latest_author_action(
    command: &UndoLatestAuthorAction,
) -> UndoLatestAuthorActionOutcome {
    let Some(frontier) = command.current_author_undo_frontier.as_ref() else {
        return TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::NoFrontier);
    };
    if frontier.sequence != command.expected_author_undo_frontier_sequence {
        return TransitionOutcome::Conflicted(UndoLatestAuthorActionConflict::FrontierMismatch);
    }
    match &frontier.kind {
        AuthorUndoFrontierKind::Barrier => {
            TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::Barrier)
        }
        AuthorUndoFrontierKind::DraftSourceUnavailable => {
            TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::SourceUnavailable)
        }
        AuthorUndoFrontierKind::DraftBindingChanged => {
            TransitionOutcome::Conflicted(UndoLatestAuthorActionConflict::SourceBindingChanged)
        }
        AuthorUndoFrontierKind::ReversibleDraftClose => {
            if command.current_head_revision_id != command.expected_head_revision_id {
                return TransitionOutcome::Conflicted(
                    UndoLatestAuthorActionConflict::WrongTargetHead,
                );
            }
            TransitionOutcome::Applied(UndoLatestAuthorActionApplied::Compensated {
                source_sequence: frontier.sequence,
            })
        }
        AuthorUndoFrontierKind::ReversibleStructureTransition => {
            TransitionOutcome::Applied(UndoLatestAuthorActionApplied::Compensated {
                source_sequence: frontier.sequence,
            })
        }
        AuthorUndoFrontierKind::ReversibleAcceptance {
            resulting_revision_id,
            prior_evidence_usable,
        } => {
            if command.current_head_revision_id != command.expected_head_revision_id {
                return TransitionOutcome::Conflicted(
                    UndoLatestAuthorActionConflict::WrongTargetHead,
                );
            }
            if command.current_head_revision_id == *resulting_revision_id && *prior_evidence_usable
            {
                TransitionOutcome::Applied(UndoLatestAuthorActionApplied::Compensated {
                    source_sequence: frontier.sequence,
                })
            } else if command.current_head_revision_id != *resulting_revision_id
                && *prior_evidence_usable
            {
                TransitionOutcome::Applied(UndoLatestAuthorActionApplied::ReversalRequired {
                    source_sequence: frontier.sequence,
                })
            } else {
                TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::SourceUnavailable)
            }
        }
        AuthorUndoFrontierKind::ReversibleDirectAuthorAction {
            resulting_revision_id,
        } => {
            if &command.expected_head_revision_id != resulting_revision_id
                || command.current_head_revision_id != command.expected_head_revision_id
            {
                return TransitionOutcome::Conflicted(
                    UndoLatestAuthorActionConflict::WrongTargetHead,
                );
            }
            TransitionOutcome::Applied(UndoLatestAuthorActionApplied::Compensated {
                source_sequence: frontier.sequence,
            })
        }
    }
}
