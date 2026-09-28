//! Classify one explicit reopen of a withdrawn Proposal.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReopenWithdrawnProposal {
    pub scope_matches: bool,
    pub admission_valid: bool,
    pub proposal_revision_current: bool,
    pub closure_withdrawn: bool,
    pub terminal_supersession: bool,
    pub withdrawal_event_matches: bool,
    pub expected_target_matches_head: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReopenWithdrawnProposalResult {
    Resolved,
    Conflicted {
        reason: ReopenWithdrawnProposalConflict,
    },
    Refused {
        reason: ReopenWithdrawnProposalRefusal,
    },
    NoEffect {
        reason: ReopenWithdrawnProposalNoEffect,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReopenWithdrawnProposalConflict {
    ChangedHead,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReopenWithdrawnProposalRefusal {
    WrongScope,
    WrongAdmission,
    StaleProposalRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReopenWithdrawnProposalNoEffect {
    TerminalSupersession,
    ClosureNotWithdrawn,
    WithdrawalEventMismatch,
}

/// Classify one permitted reopen of a withdrawn Proposal.
pub fn reopen_withdrawn_proposal(
    command: &ReopenWithdrawnProposal,
) -> ReopenWithdrawnProposalResult {
    if !command.scope_matches {
        return ReopenWithdrawnProposalResult::Refused {
            reason: ReopenWithdrawnProposalRefusal::WrongScope,
        };
    }
    if command.terminal_supersession {
        return ReopenWithdrawnProposalResult::NoEffect {
            reason: ReopenWithdrawnProposalNoEffect::TerminalSupersession,
        };
    }
    if !command.proposal_revision_current {
        return ReopenWithdrawnProposalResult::Refused {
            reason: ReopenWithdrawnProposalRefusal::StaleProposalRevision,
        };
    }
    if !command.admission_valid {
        return ReopenWithdrawnProposalResult::Refused {
            reason: ReopenWithdrawnProposalRefusal::WrongAdmission,
        };
    }
    if !command.withdrawal_event_matches {
        return ReopenWithdrawnProposalResult::NoEffect {
            reason: ReopenWithdrawnProposalNoEffect::WithdrawalEventMismatch,
        };
    }
    if !command.closure_withdrawn {
        return ReopenWithdrawnProposalResult::NoEffect {
            reason: ReopenWithdrawnProposalNoEffect::ClosureNotWithdrawn,
        };
    }
    if !command.expected_target_matches_head {
        return ReopenWithdrawnProposalResult::Conflicted {
            reason: ReopenWithdrawnProposalConflict::ChangedHead,
        };
    }
    ReopenWithdrawnProposalResult::Resolved
}

#[cfg(test)]
#[path = "reopen_withdrawn_proposal_tests.rs"]
mod tests;
