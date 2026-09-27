use super::{
    ReplanProposal, ReplanProposalConflict, ReplanProposalRefusal, ReplanProposalResult,
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
        ReplanProposalResult::Resolved
    );
}

#[test]
fn refuses_stale_ineligible_and_unproven_sources() {
    let mut wrong_scope = exact_conflict();
    wrong_scope.scope_matches = false;
    assert_eq!(
        replan_proposal(&wrong_scope),
        ReplanProposalResult::Refused {
            reason: ReplanProposalRefusal::WrongScope,
        }
    );
    let mut wrong_admission = exact_conflict();
    wrong_admission.admission_valid = false;
    assert_eq!(
        replan_proposal(&wrong_admission),
        ReplanProposalResult::Refused {
            reason: ReplanProposalRefusal::WrongAdmission,
        }
    );
    let mut stale = exact_conflict();
    stale.proposal_revision_current = false;
    assert_eq!(
        replan_proposal(&stale),
        ReplanProposalResult::Refused {
            reason: ReplanProposalRefusal::StaleProposalRevision,
        }
    );
    let mut stale_head = exact_conflict();
    stale_head.expected_head_current = false;
    assert_eq!(
        replan_proposal(&stale_head),
        ReplanProposalResult::Refused {
            reason: ReplanProposalRefusal::StaleProposalRevision,
        }
    );
    let mut closed = exact_conflict();
    closed.closure_open = false;
    assert_eq!(
        replan_proposal(&closed),
        ReplanProposalResult::Refused {
            reason: ReplanProposalRefusal::NotEligible,
        }
    );
    let mut missing_condition = exact_conflict();
    missing_condition.source_condition_matches = false;
    assert_eq!(
        replan_proposal(&missing_condition),
        ReplanProposalResult::Refused {
            reason: ReplanProposalRefusal::UnavailableProof,
        }
    );
    let mut replaced = exact_conflict();
    replaced.replacement_operations_preserve_identity = false;
    assert_eq!(
        replan_proposal(&replaced),
        ReplanProposalResult::Refused {
            reason: ReplanProposalRefusal::UnavailableProof,
        }
    );
}

#[test]
fn conflicts_a_changed_target_head() {
    let mut changed = exact_conflict();
    changed.expected_target_matches_head = false;
    assert_eq!(
        replan_proposal(&changed),
        ReplanProposalResult::Conflicted {
            reason: ReplanProposalConflict::ChangedHead,
        }
    );
}
