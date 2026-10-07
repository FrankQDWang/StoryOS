use super::{
    ReopenRejectedOperations, ReopenRejectedOperationsConflict, ReopenRejectedOperationsRefusal,
    TransitionOutcome, reopen_rejected_operations,
};

fn exact_rejected() -> ReopenRejectedOperations {
    ReopenRejectedOperations {
        scope_matches: true,
        admission_valid: true,
        proposal_revision_current: true,
        closure_open: true,
        selected_operations_rejected: true,
        rejection_event_matches: true,
        reservation_available: true,
        expected_target_matches_head: true,
    }
}

#[test]
fn resolves_one_rejected_operation_without_authority() {
    assert_eq!(
        reopen_rejected_operations(&exact_rejected()),
        TransitionOutcome::Applied(())
    );
}

#[test]
fn refuses_stale_lineage_unavailable_proof_and_non_rejected_work() {
    let mut wrong_scope = exact_rejected();
    wrong_scope.scope_matches = false;
    assert_eq!(
        reopen_rejected_operations(&wrong_scope),
        TransitionOutcome::Refused(ReopenRejectedOperationsRefusal::WrongScope)
    );
    let mut wrong_admission = exact_rejected();
    wrong_admission.admission_valid = false;
    assert_eq!(
        reopen_rejected_operations(&wrong_admission),
        TransitionOutcome::Refused(ReopenRejectedOperationsRefusal::WrongAdmission)
    );
    let mut stale = exact_rejected();
    stale.proposal_revision_current = false;
    assert_eq!(
        reopen_rejected_operations(&stale),
        TransitionOutcome::Refused(ReopenRejectedOperationsRefusal::StaleProposalRevision)
    );
    let mut closed = exact_rejected();
    closed.closure_open = false;
    assert_eq!(
        reopen_rejected_operations(&closed),
        TransitionOutcome::Refused(ReopenRejectedOperationsRefusal::NotEligible)
    );
    let mut reserved = exact_rejected();
    reserved.reservation_available = false;
    assert_eq!(
        reopen_rejected_operations(&reserved),
        TransitionOutcome::Refused(ReopenRejectedOperationsRefusal::NotEligible)
    );
    let mut not_rejected = exact_rejected();
    not_rejected.selected_operations_rejected = false;
    assert_eq!(
        reopen_rejected_operations(&not_rejected),
        TransitionOutcome::Refused(ReopenRejectedOperationsRefusal::OperationNotRejected)
    );
    let mut unavailable = exact_rejected();
    unavailable.rejection_event_matches = false;
    assert_eq!(
        reopen_rejected_operations(&unavailable),
        TransitionOutcome::Refused(ReopenRejectedOperationsRefusal::UnavailableProof)
    );
}

#[test]
fn conflicts_a_changed_target_head() {
    let mut changed = exact_rejected();
    changed.expected_target_matches_head = false;
    assert_eq!(
        reopen_rejected_operations(&changed),
        TransitionOutcome::Conflicted(ReopenRejectedOperationsConflict::ChangedHead)
    );
}
