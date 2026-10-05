//! Classify one explicit Withdrawal of an open Proposal.

use super::TransitionOutcome;
use crate::transition_outcome::reason_codes;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WithdrawalCause {
    Author,
    CurrentProducer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawProposal {
    pub scope_matches: bool,
    pub cause: WithdrawalCause,
    pub admission_valid: bool,
    pub producer_matches: bool,
    pub proposal_revision_current: bool,
    pub closure_open: bool,
    pub terminal_supersession: bool,
    pub expected_target_matches_head: bool,
}

pub type WithdrawProposalResult = TransitionOutcome<
    (),
    WithdrawProposalNoEffect,
    WithdrawProposalConflict,
    WithdrawProposalRefusal,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WithdrawProposalConflict {
    ChangedHead,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WithdrawProposalRefusal {
    WrongScope,
    WrongAdmission,
    StaleProposalRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WithdrawProposalNoEffect {
    UnsupportedCause,
    TerminalSupersession,
    ClosureNotOpen,
}

reason_codes!(WithdrawProposalConflict { ChangedHead => "changed_head" });
reason_codes!(WithdrawProposalRefusal {
    WrongScope => "wrong_scope",
    WrongAdmission => "wrong_admission",
    StaleProposalRevision => "stale_proposal_revision",
});
reason_codes!(WithdrawProposalNoEffect {
    UnsupportedCause => "unsupported_cause",
    TerminalSupersession => "terminal_supersession",
    ClosureNotOpen => "closure_not_open",
});

/// Classify one Withdrawal by the author or by the current producer.
pub fn withdraw_proposal(command: &WithdrawProposal) -> WithdrawProposalResult {
    if !command.scope_matches {
        return TransitionOutcome::Refused(WithdrawProposalRefusal::WrongScope);
    }
    if command.terminal_supersession {
        return TransitionOutcome::NoEffect(WithdrawProposalNoEffect::TerminalSupersession);
    }
    if !command.proposal_revision_current {
        return TransitionOutcome::Refused(WithdrawProposalRefusal::StaleProposalRevision);
    }
    match command.cause {
        WithdrawalCause::Author if !command.admission_valid => {
            return TransitionOutcome::Refused(WithdrawProposalRefusal::WrongAdmission);
        }
        WithdrawalCause::CurrentProducer if !command.producer_matches => {
            return TransitionOutcome::NoEffect(WithdrawProposalNoEffect::UnsupportedCause);
        }
        WithdrawalCause::Author | WithdrawalCause::CurrentProducer => {}
    }
    if !command.closure_open {
        return TransitionOutcome::NoEffect(WithdrawProposalNoEffect::ClosureNotOpen);
    }
    if !command.expected_target_matches_head {
        return TransitionOutcome::Conflicted(WithdrawProposalConflict::ChangedHead);
    }
    TransitionOutcome::Applied(())
}

#[cfg(test)]
#[path = "withdraw_proposal_tests.rs"]
mod tests;
