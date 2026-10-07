use super::{
    ReplanProposal, ReplanProposalConflict, ReplanProposalRefusal, TransitionOutcome,
    replan_proposal,
};

fn exact_conflict() -> ReplanProposal {
    ReplanProposal {
        scope_matches: true,
        admission_valid: true,
        proposal_revision_current: true,
        expected_head_current: true,
        closure_open: true,
        source_condition_matches: true,
        replacement_operations_preserve_identity: true,
        expected_target_matches_head: true,
    }
}

#[test]
fn resolves_one_current_conflict_without_authority() {
    assert_eq!(
        replan_proposal(&exact_conflict()),
        TransitionOutcome::Applied(())
    );
}

#[test]
fn refuses_stale_ineligible_and_unproven_sources() {
    let mut wrong_scope = exact_conflict();
    wrong_scope.scope_matches = false;
    assert_eq!(
        replan_proposal(&wrong_scope),
        TransitionOutcome::Refused(ReplanProposalRefusal::WrongScope)
    );
    let mut wrong_admission = exact_conflict();
    wrong_admission.admission_valid = false;
    assert_eq!(
        replan_proposal(&wrong_admission),
        TransitionOutcome::Refused(ReplanProposalRefusal::WrongAdmission)
    );
    let mut stale = exact_conflict();
    stale.proposal_revision_current = false;
    assert_eq!(
        replan_proposal(&stale),
        TransitionOutcome::Refused(ReplanProposalRefusal::StaleProposalRevision)
    );
    let mut stale_head = exact_conflict();
    stale_head.expected_head_current = false;
    assert_eq!(
        replan_proposal(&stale_head),
        TransitionOutcome::Refused(ReplanProposalRefusal::StaleProposalRevision)
    );
    let mut closed = exact_conflict();
    closed.closure_open = false;
    assert_eq!(
        replan_proposal(&closed),
        TransitionOutcome::Refused(ReplanProposalRefusal::NotEligible)
    );
    let mut missing_condition = exact_conflict();
    missing_condition.source_condition_matches = false;
    assert_eq!(
        replan_proposal(&missing_condition),
        TransitionOutcome::Refused(ReplanProposalRefusal::UnavailableProof)
    );
    let mut replaced = exact_conflict();
    replaced.replacement_operations_preserve_identity = false;
    assert_eq!(
        replan_proposal(&replaced),
        TransitionOutcome::Refused(ReplanProposalRefusal::UnavailableProof)
    );
}

#[test]
fn conflicts_a_changed_target_head() {
    let mut changed = exact_conflict();
    changed.expected_target_matches_head = false;
    assert_eq!(
        replan_proposal(&changed),
        TransitionOutcome::Conflicted(ReplanProposalConflict::ChangedHead)
    );
}
