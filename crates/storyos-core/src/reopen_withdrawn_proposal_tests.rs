use super::{
    ReopenWithdrawnProposal, ReopenWithdrawnProposalConflict, ReopenWithdrawnProposalNoEffect,
    ReopenWithdrawnProposalRefusal, TransitionOutcome, reopen_withdrawn_proposal,
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
        TransitionOutcome::Applied(())
    );
}

#[test]
fn terminal_supersession_and_a_mismatched_event_have_no_effect() {
    let mut superseded = ready();
    superseded.terminal_supersession = true;
    superseded.closure_withdrawn = false;
    assert_eq!(
        reopen_withdrawn_proposal(&superseded),
        TransitionOutcome::NoEffect(ReopenWithdrawnProposalNoEffect::TerminalSupersession)
    );
    let mut unmatched = ready();
    unmatched.withdrawal_event_matches = false;
    assert_eq!(
        reopen_withdrawn_proposal(&unmatched),
        TransitionOutcome::NoEffect(ReopenWithdrawnProposalNoEffect::WithdrawalEventMismatch)
    );
    let mut open = ready();
    open.closure_withdrawn = false;
    assert_eq!(
        reopen_withdrawn_proposal(&open),
        TransitionOutcome::NoEffect(ReopenWithdrawnProposalNoEffect::ClosureNotWithdrawn)
    );
}

#[test]
fn reopen_refuses_scope_admission_and_a_stale_revision_and_conflicts_on_the_head() {
    let mut wrong_scope = ready();
    wrong_scope.scope_matches = false;
    assert_eq!(
        reopen_withdrawn_proposal(&wrong_scope),
        TransitionOutcome::Refused(ReopenWithdrawnProposalRefusal::WrongScope)
    );
    let mut stale = ready();
    stale.proposal_revision_current = false;
    assert_eq!(
        reopen_withdrawn_proposal(&stale),
        TransitionOutcome::Refused(ReopenWithdrawnProposalRefusal::StaleProposalRevision)
    );
    let mut admission = ready();
    admission.admission_valid = false;
    assert_eq!(
        reopen_withdrawn_proposal(&admission),
        TransitionOutcome::Refused(ReopenWithdrawnProposalRefusal::WrongAdmission)
    );
    let mut head = ready();
    head.expected_target_matches_head = false;
    assert_eq!(
        reopen_withdrawn_proposal(&head),
        TransitionOutcome::Conflicted(ReopenWithdrawnProposalConflict::ChangedHead)
    );
}
