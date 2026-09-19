use super::{
    AcceptProposal, AcceptProposalConflict, AcceptProposalInvalid, AcceptProposalRefusal,
    AcceptProposalResult, ProposalBundlePolicy, ProposalOperationSelection,
    ProposalSelectionIntent, accept_proposal, classify_proposal_selection,
};

fn exact_eligible() -> AcceptProposal {
    AcceptProposal {
        scope_matches: true,
        admission_valid: true,
        proposal_revision_current: true,
        retention_retained: true,
        generation_ready: true,
        closure_open: true,
        validation_current: true,
        validation_receipt_valid: true,
        validation_receipt_matches_revision: true,
        selected_operation_pending: true,
        selection_duplicate_free: true,
        required_dependencies_met: true,
        bundle_closure_complete: true,
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
fn refuses_duplicate_identities_missing_dependencies_and_incomplete_bundle_closure() {
    let mut duplicates = exact_eligible();
    duplicates.selection_duplicate_free = false;
    assert_eq!(
        accept_proposal(&duplicates),
        AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::DuplicateIdentities,
        }
    );
    let mut missing = exact_eligible();
    missing.required_dependencies_met = false;
    assert_eq!(
        accept_proposal(&missing),
        AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::MissingRequiredDependencies,
        }
    );
    let mut incomplete = exact_eligible();
    incomplete.bundle_closure_complete = false;
    assert_eq!(
        accept_proposal(&incomplete),
        AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::IncompleteBundleClosure,
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
fn classifies_duplicate_dependency_and_bundle_selection_sets() {
    let first = ProposalOperationSelection {
        operation_id: "op-1".to_owned(),
        resolution: "pending".to_owned(),
        predecessor_operation_ids: Vec::new(),
    };
    let second = ProposalOperationSelection {
        operation_id: "op-2".to_owned(),
        resolution: "pending".to_owned(),
        predecessor_operation_ids: vec!["op-1".to_owned()],
    };
    let operations = [first.clone(), second.clone()];
    let duplicates = classify_proposal_selection(
        &["op-1".to_owned(), "op-1".to_owned()],
        &operations,
        ProposalBundlePolicy::None,
        ProposalSelectionIntent::Accept,
    );
    assert!(!duplicates.duplicate_free);
    let missing = classify_proposal_selection(
        &["op-2".to_owned()],
        &operations,
        ProposalBundlePolicy::None,
        ProposalSelectionIntent::Accept,
    );
    assert!(missing.duplicate_free);
    assert!(missing.all_selected_pending);
    assert!(!missing.required_dependencies_met);
    let incomplete = classify_proposal_selection(
        &["op-1".to_owned()],
        &operations,
        ProposalBundlePolicy::Atomic,
        ProposalSelectionIntent::Accept,
    );
    assert!(incomplete.required_dependencies_met);
    assert!(!incomplete.bundle_closure_complete);
    let closed = classify_proposal_selection(
        &["op-2".to_owned(), "op-1".to_owned()],
        &operations,
        ProposalBundlePolicy::Atomic,
        ProposalSelectionIntent::Accept,
    );
    assert!(closed.required_dependencies_met);
    assert!(closed.bundle_closure_complete);
    let applied_first = ProposalOperationSelection {
        resolution: "applied".to_owned(),
        ..first.clone()
    };
    let after_apply = classify_proposal_selection(
        &["op-2".to_owned()],
        &[applied_first, second.clone()],
        ProposalBundlePolicy::None,
        ProposalSelectionIntent::Accept,
    );
    assert!(after_apply.required_dependencies_met);
    assert!(after_apply.bundle_closure_complete);
    let rejected_first = ProposalOperationSelection {
        resolution: "rejected".to_owned(),
        ..first
    };
    let after_reject = classify_proposal_selection(
        &["op-2".to_owned()],
        &[rejected_first.clone(), second.clone()],
        ProposalBundlePolicy::None,
        ProposalSelectionIntent::Accept,
    );
    assert!(!after_reject.required_dependencies_met);
    let reject_remaining = classify_proposal_selection(
        &["op-2".to_owned()],
        &[rejected_first, second],
        ProposalBundlePolicy::None,
        ProposalSelectionIntent::Reject,
    );
    assert!(reject_remaining.required_dependencies_met);
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
