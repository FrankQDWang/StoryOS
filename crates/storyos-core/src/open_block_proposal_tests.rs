use super::{
    OpenBlockProposal, OpenBlockProposalConflict, OpenBlockProposalRefusal,
    OpenBlockProposalResult, open_block_proposal,
};

fn exact_current_block() -> OpenBlockProposal {
    OpenBlockProposal {
        scope_matches: true,
        target_block_present: true,
        expected_base_revision_id: "rev-1".to_owned(),
        current_base_revision_id: Some("rev-1".to_owned()),
        conflicting_reservation: false,
    }
}

#[test]
fn opens_one_block_proposal_for_an_exact_current_base() {
    let opened = open_block_proposal(&exact_current_block());
    assert_eq!(opened, OpenBlockProposalResult::Applied);
    assert_eq!(opened.validation_receipt_result(), Some("valid"));
}

#[test]
fn refuses_wrong_scope_and_unavailable_targets() {
    let mut wrong_scope = exact_current_block();
    wrong_scope.scope_matches = false;
    assert_eq!(
        open_block_proposal(&wrong_scope),
        OpenBlockProposalResult::Refused {
            reason: OpenBlockProposalRefusal::WrongScope,
        }
    );
    let mut missing = exact_current_block();
    missing.target_block_present = false;
    assert_eq!(
        open_block_proposal(&missing),
        OpenBlockProposalResult::Refused {
            reason: OpenBlockProposalRefusal::UnavailableTarget,
        }
    );
    let mut no_head = exact_current_block();
    no_head.current_base_revision_id = None;
    assert_eq!(
        open_block_proposal(&no_head),
        OpenBlockProposalResult::Refused {
            reason: OpenBlockProposalRefusal::UnavailableTarget,
        }
    );
}

#[test]
fn conflicts_changed_heads_and_overlapping_reservations() {
    let mut changed = exact_current_block();
    changed.current_base_revision_id = Some("rev-2".to_owned());
    assert_eq!(
        open_block_proposal(&changed),
        OpenBlockProposalResult::Conflicted {
            reason: OpenBlockProposalConflict::ChangedHead,
        }
    );
    let mut reserved = exact_current_block();
    reserved.conflicting_reservation = true;
    assert_eq!(
        open_block_proposal(&reserved),
        OpenBlockProposalResult::Conflicted {
            reason: OpenBlockProposalConflict::ConflictingReservation,
        }
    );
}
