use super::{
    RejectProposalOperations, RejectProposalOperationsConflict, RejectProposalOperationsRefusal,
    reject_proposal_operations,
};
use crate::TransitionOutcome;

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
        TransitionOutcome::Applied(())
    );
}

#[test]
fn refuses_wrong_scope_admission_stale_revision_and_non_pending_work() {
    let mut wrong_scope = exact_pending();
    wrong_scope.scope_matches = false;
    assert_eq!(
        reject_proposal_operations(&wrong_scope),
        TransitionOutcome::Refused(RejectProposalOperationsRefusal::WrongScope)
    );
    let mut wrong_admission = exact_pending();
    wrong_admission.admission_valid = false;
    assert_eq!(
        reject_proposal_operations(&wrong_admission),
        TransitionOutcome::Refused(RejectProposalOperationsRefusal::WrongAdmission)
    );
    let mut stale = exact_pending();
    stale.proposal_revision_current = false;
    assert_eq!(
        reject_proposal_operations(&stale),
        TransitionOutcome::Refused(RejectProposalOperationsRefusal::StaleProposalRevision)
    );
    let mut closed = exact_pending();
    closed.closure_open = false;
    assert_eq!(
        reject_proposal_operations(&closed),
        TransitionOutcome::Refused(RejectProposalOperationsRefusal::NotEligible)
    );
    let mut not_pending = exact_pending();
    not_pending.selected_operations_pending = false;
    assert_eq!(
        reject_proposal_operations(&not_pending),
        TransitionOutcome::Refused(RejectProposalOperationsRefusal::OperationNotPending)
    );
}

#[test]
fn refuses_duplicate_identities_missing_dependencies_and_incomplete_bundle_closure() {
    let mut duplicates = exact_pending();
    duplicates.selection_duplicate_free = false;
    assert_eq!(
        reject_proposal_operations(&duplicates),
        TransitionOutcome::Refused(RejectProposalOperationsRefusal::DuplicateIdentities)
    );
    let mut missing = exact_pending();
    missing.required_dependencies_met = false;
    assert_eq!(
        reject_proposal_operations(&missing),
        TransitionOutcome::Refused(RejectProposalOperationsRefusal::MissingRequiredDependencies)
    );
    let mut incomplete = exact_pending();
    incomplete.bundle_closure_complete = false;
    assert_eq!(
        reject_proposal_operations(&incomplete),
        TransitionOutcome::Refused(RejectProposalOperationsRefusal::IncompleteBundleClosure)
    );
}

#[test]
fn conflicts_a_changed_target_head() {
    let mut changed = exact_pending();
    changed.expected_target_matches_head = false;
    assert_eq!(
        reject_proposal_operations(&changed),
        TransitionOutcome::Conflicted(RejectProposalOperationsConflict::ChangedHead)
    );
}
