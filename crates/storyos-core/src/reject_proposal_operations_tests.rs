use super::{
    RejectProposalOperations, RejectProposalOperationsConflict, RejectProposalOperationsRefusal,
    RejectProposalOperationsResult, reject_proposal_operations,
};

fn exact_pending() -> RejectProposalOperations {
    RejectProposalOperations {
        scope_matches: true,
        admission_valid: true,
        proposal_revision_current: true,
        closure_open: true,
        selected_operations_pending: true,
        selection_duplicate_free: true,
        required_dependencies_met: true,
        bundle_closure_complete: true,
        expected_target_matches_head: true,
    }
}

#[test]
fn resolves_one_pending_operation_without_authority() {
    assert_eq!(
        reject_proposal_operations(&exact_pending()),
        RejectProposalOperationsResult::Resolved
    );
}

#[test]
fn refuses_wrong_scope_admission_stale_revision_and_non_pending_work() {
    let mut wrong_scope = exact_pending();
    wrong_scope.scope_matches = false;
    assert_eq!(
        reject_proposal_operations(&wrong_scope),
        RejectProposalOperationsResult::Refused {
            reason: RejectProposalOperationsRefusal::WrongScope,
        }
    );
    let mut wrong_admission = exact_pending();
    wrong_admission.admission_valid = false;
    assert_eq!(
        reject_proposal_operations(&wrong_admission),
        RejectProposalOperationsResult::Refused {
            reason: RejectProposalOperationsRefusal::WrongAdmission,
        }
    );
    let mut stale = exact_pending();
    stale.proposal_revision_current = false;
    assert_eq!(
        reject_proposal_operations(&stale),
        RejectProposalOperationsResult::Refused {
            reason: RejectProposalOperationsRefusal::StaleProposalRevision,
        }
    );
    let mut closed = exact_pending();
    closed.closure_open = false;
    assert_eq!(
        reject_proposal_operations(&closed),
        RejectProposalOperationsResult::Refused {
            reason: RejectProposalOperationsRefusal::NotEligible,
        }
    );
    let mut not_pending = exact_pending();
    not_pending.selected_operations_pending = false;
    assert_eq!(
        reject_proposal_operations(&not_pending),
        RejectProposalOperationsResult::Refused {
            reason: RejectProposalOperationsRefusal::OperationNotPending,
        }
    );
}

#[test]
fn refuses_duplicate_identities_missing_dependencies_and_incomplete_bundle_closure() {
    let mut duplicates = exact_pending();
    duplicates.selection_duplicate_free = false;
    assert_eq!(
        reject_proposal_operations(&duplicates),
        RejectProposalOperationsResult::Refused {
            reason: RejectProposalOperationsRefusal::DuplicateIdentities,
        }
    );
    let mut missing = exact_pending();
    missing.required_dependencies_met = false;
    assert_eq!(
        reject_proposal_operations(&missing),
        RejectProposalOperationsResult::Refused {
            reason: RejectProposalOperationsRefusal::MissingRequiredDependencies,
        }
    );
    let mut incomplete = exact_pending();
    incomplete.bundle_closure_complete = false;
    assert_eq!(
        reject_proposal_operations(&incomplete),
        RejectProposalOperationsResult::Refused {
            reason: RejectProposalOperationsRefusal::IncompleteBundleClosure,
        }
    );
}

#[test]
fn conflicts_a_changed_target_head() {
    let mut changed = exact_pending();
    changed.expected_target_matches_head = false;
    assert_eq!(
        reject_proposal_operations(&changed),
        RejectProposalOperationsResult::Conflicted {
            reason: RejectProposalOperationsConflict::ChangedHead,
        }
    );
}
