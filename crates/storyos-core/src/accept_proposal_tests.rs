use super::{
    AcceptProposal, AcceptProposalConflict, AcceptProposalInvalid, AcceptProposalRefusal,
    AcceptProposalResult, accept_proposal,
};

fn exact_eligible() -> AcceptProposal {
    AcceptProposal {
        scope_matches: true,
        admission_valid: true,
        proposal_revision_current: true,
        retention_retained: true,
        generation_ready: true,
        closure_open: true,
        validation_receipt_valid: true,
        validation_receipt_matches_revision: true,
        selected_operation_pending: true,
        expected_target_matches_head: true,
        candidate_unaltered: true,
    }
}

#[test]
fn applies_one_pending_operation_for_an_exact_eligible_revision() {
    assert_eq!(
        accept_proposal(&exact_eligible()),
        AcceptProposalResult::Applied
    );
}

#[test]
fn refuses_wrong_scope_admission_stale_revision_and_ineligible_state() {
    let mut wrong_scope = exact_eligible();
    wrong_scope.scope_matches = false;
    assert_eq!(
        accept_proposal(&wrong_scope),
        AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::WrongScope,
        }
    );
    let mut wrong_admission = exact_eligible();
    wrong_admission.admission_valid = false;
    assert_eq!(
        accept_proposal(&wrong_admission),
        AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::WrongAdmission,
        }
    );
    let mut stale = exact_eligible();
    stale.proposal_revision_current = false;
    assert_eq!(
        accept_proposal(&stale),
        AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::StaleProposalRevision,
        }
    );
    let mut not_ready = exact_eligible();
    not_ready.generation_ready = false;
    assert_eq!(
        accept_proposal(&not_ready),
        AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::NotEligible,
        }
    );
    let mut not_pending = exact_eligible();
    not_pending.selected_operation_pending = false;
    assert_eq!(
        accept_proposal(&not_pending),
        AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::OperationNotPending,
        }
    );
}

#[test]
fn invalidates_bad_validation_and_altered_candidates() {
    let mut invalid = exact_eligible();
    invalid.validation_receipt_valid = false;
    assert_eq!(
        accept_proposal(&invalid),
        AcceptProposalResult::Invalid {
            reason: AcceptProposalInvalid::InvalidValidation,
        }
    );
    let mut mismatched = exact_eligible();
    mismatched.validation_receipt_matches_revision = false;
    assert_eq!(
        accept_proposal(&mismatched),
        AcceptProposalResult::Invalid {
            reason: AcceptProposalInvalid::InvalidValidation,
        }
    );
    let mut altered = exact_eligible();
    altered.candidate_unaltered = false;
    assert_eq!(
        accept_proposal(&altered),
        AcceptProposalResult::Invalid {
            reason: AcceptProposalInvalid::AlteredCandidate,
        }
    );
}

#[test]
fn conflicts_a_changed_target_head() {
    let mut changed = exact_eligible();
    changed.expected_target_matches_head = false;
    assert_eq!(
        accept_proposal(&changed),
        AcceptProposalResult::Conflicted {
            reason: AcceptProposalConflict::ChangedHead,
        }
    );
}
