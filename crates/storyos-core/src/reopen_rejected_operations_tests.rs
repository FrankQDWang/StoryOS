use super::{
    ReopenRejectedOperations, ReopenRejectedOperationsConflict, ReopenRejectedOperationsRefusal,
    ReopenRejectedOperationsResult, reopen_rejected_operations,
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
        ReopenRejectedOperationsResult::Resolved
    );
}

#[test]
fn refuses_stale_lineage_unavailable_proof_and_non_rejected_work() {
    let mut wrong_scope = exact_rejected();
    wrong_scope.scope_matches = false;
    assert_eq!(
        reopen_rejected_operations(&wrong_scope),
        ReopenRejectedOperationsResult::Refused {
            reason: ReopenRejectedOperationsRefusal::WrongScope,
        }
    );
    let mut wrong_admission = exact_rejected();
    wrong_admission.admission_valid = false;
    assert_eq!(
        reopen_rejected_operations(&wrong_admission),
        ReopenRejectedOperationsResult::Refused {
            reason: ReopenRejectedOperationsRefusal::WrongAdmission,
        }
    );
    let mut stale = exact_rejected();
    stale.proposal_revision_current = false;
    assert_eq!(
        reopen_rejected_operations(&stale),
        ReopenRejectedOperationsResult::Refused {
            reason: ReopenRejectedOperationsRefusal::StaleProposalRevision,
        }
    );
    let mut closed = exact_rejected();
    closed.closure_open = false;
    assert_eq!(
        reopen_rejected_operations(&closed),
        ReopenRejectedOperationsResult::Refused {
            reason: ReopenRejectedOperationsRefusal::NotEligible,
        }
    );
    let mut reserved = exact_rejected();
    reserved.reservation_available = false;
    assert_eq!(
        reopen_rejected_operations(&reserved),
        ReopenRejectedOperationsResult::Refused {
            reason: ReopenRejectedOperationsRefusal::NotEligible,
        }
    );
    let mut not_rejected = exact_rejected();
    not_rejected.selected_operations_rejected = false;
    assert_eq!(
        reopen_rejected_operations(&not_rejected),
        ReopenRejectedOperationsResult::Refused {
            reason: ReopenRejectedOperationsRefusal::OperationNotRejected,
        }
    );
    let mut unavailable = exact_rejected();
    unavailable.rejection_event_matches = false;
    assert_eq!(
        reopen_rejected_operations(&unavailable),
        ReopenRejectedOperationsResult::Refused {
            reason: ReopenRejectedOperationsRefusal::UnavailableProof,
        }
    );
}

#[test]
fn conflicts_a_changed_target_head() {
    let mut changed = exact_rejected();
    changed.expected_target_matches_head = false;
    assert_eq!(
        reopen_rejected_operations(&changed),
        ReopenRejectedOperationsResult::Conflicted {
            reason: ReopenRejectedOperationsConflict::ChangedHead,
        }
    );
}
