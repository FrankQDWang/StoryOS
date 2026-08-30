//! Pure Core classification for Undo Latest Author Action.

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
    ReversibleDirectAuthorAction { resulting_revision_id: String },
    Barrier,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UndoLatestAuthorActionResult {
    Compensated {
        source_sequence: u64,
    },
    Conflicted {
        reason: UndoLatestAuthorActionConflict,
    },
    Unavailable {
        reason: UndoLatestAuthorActionUnavailable,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UndoLatestAuthorActionConflict {
    FrontierMismatch {
        current_author_undo_frontier_sequence: Option<u64>,
    },
    WrongTargetHead,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UndoLatestAuthorActionUnavailable {
    NoFrontier,
    Barrier,
}

/// Classify one Undo Latest Author Action against the derived Author Undo Frontier.
pub fn undo_latest_author_action(command: &UndoLatestAuthorAction) -> UndoLatestAuthorActionResult {
    let Some(frontier) = command.current_author_undo_frontier.as_ref() else {
        return UndoLatestAuthorActionResult::Unavailable {
            reason: UndoLatestAuthorActionUnavailable::NoFrontier,
        };
    };
    if frontier.sequence != command.expected_author_undo_frontier_sequence {
        return UndoLatestAuthorActionResult::Conflicted {
            reason: UndoLatestAuthorActionConflict::FrontierMismatch {
                current_author_undo_frontier_sequence: Some(frontier.sequence),
            },
        };
    }
    match &frontier.kind {
        AuthorUndoFrontierKind::Barrier => UndoLatestAuthorActionResult::Unavailable {
            reason: UndoLatestAuthorActionUnavailable::Barrier,
        },
        AuthorUndoFrontierKind::ReversibleDirectAuthorAction {
            resulting_revision_id,
        } => {
            if &command.expected_head_revision_id != resulting_revision_id
                || command.current_head_revision_id != command.expected_head_revision_id
            {
                return UndoLatestAuthorActionResult::Conflicted {
                    reason: UndoLatestAuthorActionConflict::WrongTargetHead,
                };
            }
            UndoLatestAuthorActionResult::Compensated {
                source_sequence: frontier.sequence,
            }
        }
    }
}
