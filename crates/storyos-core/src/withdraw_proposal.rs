//! Classify one explicit Withdrawal of an open Proposal.

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WithdrawalAllocation {
    AuthorForward,
    CurrentProducerOwned,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WithdrawProposalResult {
    Resolved { allocation: WithdrawalAllocation },
    Conflicted { reason: WithdrawProposalConflict },
    Refused { reason: WithdrawProposalRefusal },
    NoEffect { reason: WithdrawProposalNoEffect },
}

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

/// Classify one Withdrawal. Author success allocates one forward action. Producer success allocates none.
pub fn withdraw_proposal(command: &WithdrawProposal) -> WithdrawProposalResult {
    if !command.scope_matches {
        return WithdrawProposalResult::Refused {
            reason: WithdrawProposalRefusal::WrongScope,
        };
    }
    if command.terminal_supersession {
        return WithdrawProposalResult::NoEffect {
            reason: WithdrawProposalNoEffect::TerminalSupersession,
        };
    }
    if !command.proposal_revision_current {
        return WithdrawProposalResult::Refused {
            reason: WithdrawProposalRefusal::StaleProposalRevision,
        };
    }
    match command.cause {
        WithdrawalCause::Author if !command.admission_valid => {
            return WithdrawProposalResult::Refused {
                reason: WithdrawProposalRefusal::WrongAdmission,
            };
        }
        WithdrawalCause::CurrentProducer if !command.producer_matches => {
            return WithdrawProposalResult::NoEffect {
                reason: WithdrawProposalNoEffect::UnsupportedCause,
            };
        }
        WithdrawalCause::Author | WithdrawalCause::CurrentProducer => {}
    }
    if !command.closure_open {
        return WithdrawProposalResult::NoEffect {
            reason: WithdrawProposalNoEffect::ClosureNotOpen,
        };
    }
    if !command.expected_target_matches_head {
        return WithdrawProposalResult::Conflicted {
            reason: WithdrawProposalConflict::ChangedHead,
        };
    }
    let allocation = match command.cause {
        WithdrawalCause::Author => WithdrawalAllocation::AuthorForward,
        WithdrawalCause::CurrentProducer => WithdrawalAllocation::CurrentProducerOwned,
    };
    WithdrawProposalResult::Resolved { allocation }
}

#[cfg(test)]
#[path = "withdraw_proposal_tests.rs"]
mod tests;
