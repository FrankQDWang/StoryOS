use super::{
    TransitionOutcome, WithdrawProposal, WithdrawProposalConflict, WithdrawProposalNoEffect,
    WithdrawProposalRefusal, WithdrawalCause, withdraw_proposal,
};

fn author_withdraw() -> WithdrawProposal {
    WithdrawProposal {
        scope_matches: true,
        cause: WithdrawalCause::Author,
        admission_valid: true,
        producer_matches: false,
        proposal_revision_current: true,
        closure_open: true,
        terminal_supersession: false,
        expected_target_matches_head: true,
    }
}

fn producer_withdraw() -> WithdrawProposal {
    WithdrawProposal {
        cause: WithdrawalCause::CurrentProducer,
        admission_valid: false,
        producer_matches: true,
        ..author_withdraw()
    }
}

#[test]
fn an_author_withdrawal_of_an_open_current_revision_applies() {
    assert_eq!(
        withdraw_proposal(&author_withdraw()),
        TransitionOutcome::Applied(())
    );
}

#[test]
fn a_matching_current_producer_withdrawal_applies() {
    assert_eq!(
        withdraw_proposal(&producer_withdraw()),
        TransitionOutcome::Applied(())
    );
}

#[test]
fn unsupported_cause_and_terminal_supersession_have_no_effect() {
    let mut unmatched = producer_withdraw();
    unmatched.producer_matches = false;
    assert_eq!(
        withdraw_proposal(&unmatched),
        TransitionOutcome::NoEffect(WithdrawProposalNoEffect::UnsupportedCause)
    );
    let mut superseded = author_withdraw();
    superseded.terminal_supersession = true;
    superseded.closure_open = false;
    assert_eq!(
        withdraw_proposal(&superseded),
        TransitionOutcome::NoEffect(WithdrawProposalNoEffect::TerminalSupersession)
    );
    let mut closed = author_withdraw();
    closed.closure_open = false;
    assert_eq!(
        withdraw_proposal(&closed),
        TransitionOutcome::NoEffect(WithdrawProposalNoEffect::ClosureNotOpen)
    );
}

#[test]
fn withdraw_refuses_scope_admission_and_stale_revision_and_conflicts_on_head() {
    let mut wrong_scope = author_withdraw();
    wrong_scope.scope_matches = false;
    assert_eq!(
        withdraw_proposal(&wrong_scope),
        TransitionOutcome::Refused(WithdrawProposalRefusal::WrongScope)
    );
    let mut wrong_admission = author_withdraw();
    wrong_admission.admission_valid = false;
    assert_eq!(
        withdraw_proposal(&wrong_admission),
        TransitionOutcome::Refused(WithdrawProposalRefusal::WrongAdmission)
    );
    let mut stale = author_withdraw();
    stale.proposal_revision_current = false;
    assert_eq!(
        withdraw_proposal(&stale),
        TransitionOutcome::Refused(WithdrawProposalRefusal::StaleProposalRevision)
    );
    let mut changed = author_withdraw();
    changed.expected_target_matches_head = false;
    assert_eq!(
        withdraw_proposal(&changed),
        TransitionOutcome::Conflicted(WithdrawProposalConflict::ChangedHead)
    );
}
