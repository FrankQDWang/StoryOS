use super::{
    AuthorUndoFrontier, AuthorUndoFrontierKind, UndoLatestAuthorAction,
    UndoLatestAuthorActionApplied, UndoLatestAuthorActionConflict,
    UndoLatestAuthorActionUnavailable, undo_latest_author_action,
};
use crate::TransitionOutcome;

const HEAD: &str = "018f0000-0000-7001-8000-000000000805";
const OTHER: &str = "018f0000-0000-7001-8000-000000000806";

fn acceptance(resulting: &str, prior_evidence_usable: bool) -> UndoLatestAuthorAction {
    let mut command = command();
    command.current_author_undo_frontier = Some(AuthorUndoFrontier {
        sequence: 1,
        kind: AuthorUndoFrontierKind::ReversibleAcceptance {
            resulting_revision_id: resulting.to_owned(),
            prior_evidence_usable,
        },
    });
    command
}

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
        TransitionOutcome::Applied(UndoLatestAuthorActionApplied::Compensated {
            source_sequence: 1
        })
    );
}

#[test]
fn a_frontier_mismatch_classifies_as_conflicted_with_zero_authority_effect() {
    let mut stale = command();
    stale.expected_author_undo_frontier_sequence = 2;
    assert_eq!(
        undo_latest_author_action(&stale),
        TransitionOutcome::Conflicted(UndoLatestAuthorActionConflict::FrontierMismatch)
    );
}

#[test]
fn a_missing_frontier_classifies_as_unavailable() {
    let mut empty = command();
    empty.current_author_undo_frontier = None;
    assert_eq!(
        undo_latest_author_action(&empty),
        TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::NoFrontier)
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
        TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::Barrier)
    );
}

#[test]
fn a_wrong_target_head_classifies_as_conflicted_with_zero_authority_effect() {
    let mut wrong = command();
    wrong.current_head_revision_id = "018f0000-0000-7001-8000-000000000999".to_owned();
    assert_eq!(
        undo_latest_author_action(&wrong),
        TransitionOutcome::Conflicted(UndoLatestAuthorActionConflict::WrongTargetHead)
    );
}

#[test]
fn a_matching_acceptance_frontier_classifies_as_compensated() {
    assert_eq!(
        undo_latest_author_action(&acceptance(HEAD, /*prior_evidence_usable*/ true)),
        TransitionOutcome::Applied(UndoLatestAuthorActionApplied::Compensated {
            source_sequence: 1
        })
    );
}

#[test]
fn a_drifted_acceptance_head_with_usable_evidence_requires_reversal() {
    let mut drifted = acceptance(HEAD, /*prior_evidence_usable*/ true);
    drifted.current_head_revision_id = OTHER.to_owned();
    drifted.expected_head_revision_id = OTHER.to_owned();
    assert_eq!(
        undo_latest_author_action(&drifted),
        TransitionOutcome::Applied(UndoLatestAuthorActionApplied::ReversalRequired {
            source_sequence: 1
        })
    );
}

#[test]
fn unusable_acceptance_evidence_is_unavailable() {
    let mut drifted = acceptance(HEAD, /*prior_evidence_usable*/ false);
    drifted.current_head_revision_id = OTHER.to_owned();
    drifted.expected_head_revision_id = OTHER.to_owned();
    assert_eq!(
        undo_latest_author_action(&drifted),
        TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::SourceUnavailable)
    );
    assert_eq!(
        undo_latest_author_action(&acceptance(HEAD, /*prior_evidence_usable*/ false)),
        TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::SourceUnavailable)
    );
}

#[test]
fn a_stale_expected_acceptance_head_stays_conflicted() {
    let mut stale = acceptance(HEAD, /*prior_evidence_usable*/ true);
    stale.expected_head_revision_id = OTHER.to_owned();
    assert_eq!(
        undo_latest_author_action(&stale),
        TransitionOutcome::Conflicted(UndoLatestAuthorActionConflict::WrongTargetHead)
    );
}

#[test]
fn an_acceptance_frontier_mismatch_stays_conflicted() {
    let mut stale = acceptance(HEAD, /*prior_evidence_usable*/ true);
    stale.expected_author_undo_frontier_sequence = 9;
    assert_eq!(
        undo_latest_author_action(&stale),
        TransitionOutcome::Conflicted(UndoLatestAuthorActionConflict::FrontierMismatch)
    );
}

#[test]
fn a_matching_structure_frontier_classifies_as_compensated_without_head_proof() {
    let mut structure = command();
    structure.current_author_undo_frontier = Some(AuthorUndoFrontier {
        sequence: 1,
        kind: AuthorUndoFrontierKind::ReversibleStructureTransition,
    });
    structure.expected_head_revision_id = String::new();
    structure.current_head_revision_id = "018f0000-0000-7001-8000-000000000999".to_owned();
    assert_eq!(
        undo_latest_author_action(&structure),
        TransitionOutcome::Applied(UndoLatestAuthorActionApplied::Compensated {
            source_sequence: 1
        })
    );
}

#[test]
fn every_zero_authority_undo_outcome_round_trips_through_its_receipt_codes() {
    let outcomes = [
        TransitionOutcome::Conflicted(UndoLatestAuthorActionConflict::FrontierMismatch),
        TransitionOutcome::Conflicted(UndoLatestAuthorActionConflict::WrongTargetHead),
        TransitionOutcome::Conflicted(UndoLatestAuthorActionConflict::SourceBindingChanged),
        TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::NoFrontier),
        TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::Barrier),
        TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::SourceUnavailable),
    ];
    for outcome in outcomes {
        let reason = outcome.reason_code();
        assert_eq!(
            crate::UndoLatestAuthorActionOutcome::from_zero_authority_codes(
                outcome.receipt_result_kind(),
                reason
            ),
            Some(outcome)
        );
    }
}
