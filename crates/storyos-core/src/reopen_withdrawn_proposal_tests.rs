use super::{
    ReopenWithdrawnProposal, ReopenWithdrawnProposalConflict, ReopenWithdrawnProposalNoEffect,
    ReopenWithdrawnProposalRefusal, ReopenWithdrawnProposalResult, reopen_withdrawn_proposal,
};

fn ready() -> ReopenWithdrawnProposal {
    ReopenWithdrawnProposal {
        scope_matches: true,
        admission_valid: true,
        proposal_revision_current: true,
        closure_withdrawn: true,
        terminal_supersession: false,
        withdrawal_event_matches: true,
        expected_target_matches_head: true,
    }
}

#[test]
fn a_matching_withdrawn_proposal_reopens() {
    assert_eq!(
        reopen_withdrawn_proposal(&ready()),
        ReopenWithdrawnProposalResult::Resolved
    );
}

#[test]
fn terminal_supersession_and_a_mismatched_event_have_no_effect() {
    let mut superseded = ready();
    superseded.terminal_supersession = true;
    superseded.closure_withdrawn = false;
    assert_eq!(
        reopen_withdrawn_proposal(&superseded),
        ReopenWithdrawnProposalResult::NoEffect {
            reason: ReopenWithdrawnProposalNoEffect::TerminalSupersession,
        }
    );
    let mut unmatched = ready();
    unmatched.withdrawal_event_matches = false;
    assert_eq!(
        reopen_withdrawn_proposal(&unmatched),
        ReopenWithdrawnProposalResult::NoEffect {
            reason: ReopenWithdrawnProposalNoEffect::WithdrawalEventMismatch,
        }
    );
    let mut open = ready();
    open.closure_withdrawn = false;
    assert_eq!(
        reopen_withdrawn_proposal(&open),
        ReopenWithdrawnProposalResult::NoEffect {
            reason: ReopenWithdrawnProposalNoEffect::ClosureNotWithdrawn,
        }
    );
}

#[test]
fn reopen_refuses_scope_admission_and_a_stale_revision_and_conflicts_on_the_head() {
    let mut wrong_scope = ready();
    wrong_scope.scope_matches = false;
    assert_eq!(
        reopen_withdrawn_proposal(&wrong_scope),
        ReopenWithdrawnProposalResult::Refused {
            reason: ReopenWithdrawnProposalRefusal::WrongScope,
        }
    );
    let mut stale = ready();
    stale.proposal_revision_current = false;
    assert_eq!(
        reopen_withdrawn_proposal(&stale),
        ReopenWithdrawnProposalResult::Refused {
            reason: ReopenWithdrawnProposalRefusal::StaleProposalRevision,
        }
    );
    let mut admission = ready();
    admission.admission_valid = false;
    assert_eq!(
        reopen_withdrawn_proposal(&admission),
        ReopenWithdrawnProposalResult::Refused {
            reason: ReopenWithdrawnProposalRefusal::WrongAdmission,
        }
    );
    let mut head = ready();
    head.expected_target_matches_head = false;
    assert_eq!(
        reopen_withdrawn_proposal(&head),
        ReopenWithdrawnProposalResult::Conflicted {
            reason: ReopenWithdrawnProposalConflict::ChangedHead,
        }
    );
}
