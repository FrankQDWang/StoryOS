use super::{
    AuthorUndoFrontier, AuthorUndoFrontierKind, UndoLatestAuthorAction,
    UndoLatestAuthorActionConflict, UndoLatestAuthorActionResult,
    UndoLatestAuthorActionUnavailable, undo_latest_author_action,
};

const HEAD: &str = "018f0000-0000-7001-8000-000000000805";

fn command() -> UndoLatestAuthorAction {
    UndoLatestAuthorAction {
        expected_author_undo_frontier_sequence: 1,
        current_author_undo_frontier: Some(AuthorUndoFrontier {
            sequence: 1,
            kind: AuthorUndoFrontierKind::ReversibleDirectAuthorAction {
                resulting_revision_id: HEAD.to_owned(),
            },
        }),
        expected_head_revision_id: HEAD.to_owned(),
        current_head_revision_id: HEAD.to_owned(),
    }
}

#[test]
fn a_matching_reversible_frontier_classifies_as_compensated() {
    assert_eq!(
        undo_latest_author_action(&command()),
        UndoLatestAuthorActionResult::Compensated { source_sequence: 1 }
    );
}

#[test]
fn a_frontier_mismatch_classifies_as_conflicted_with_zero_authority_effect() {
    let mut stale = command();
    stale.expected_author_undo_frontier_sequence = 2;
    assert_eq!(
        undo_latest_author_action(&stale),
        UndoLatestAuthorActionResult::Conflicted {
            reason: UndoLatestAuthorActionConflict::FrontierMismatch {
                current_author_undo_frontier_sequence: Some(1),
            },
        }
    );
}

#[test]
fn a_missing_frontier_classifies_as_unavailable() {
    let mut empty = command();
    empty.current_author_undo_frontier = None;
    assert_eq!(
        undo_latest_author_action(&empty),
        UndoLatestAuthorActionResult::Unavailable {
            reason: UndoLatestAuthorActionUnavailable::NoFrontier,
        }
    );
}

#[test]
fn a_barrier_frontier_classifies_as_unavailable_and_cannot_be_skipped() {
    let mut barrier = command();
    barrier.current_author_undo_frontier = Some(AuthorUndoFrontier {
        sequence: 1,
        kind: AuthorUndoFrontierKind::Barrier,
    });
    assert_eq!(
        undo_latest_author_action(&barrier),
        UndoLatestAuthorActionResult::Unavailable {
            reason: UndoLatestAuthorActionUnavailable::Barrier,
        }
    );
}

#[test]
fn a_wrong_target_head_classifies_as_conflicted_with_zero_authority_effect() {
    let mut wrong = command();
    wrong.current_head_revision_id = "018f0000-0000-7001-8000-000000000999".to_owned();
    assert_eq!(
        undo_latest_author_action(&wrong),
        UndoLatestAuthorActionResult::Conflicted {
            reason: UndoLatestAuthorActionConflict::WrongTargetHead,
        }
    );
}
