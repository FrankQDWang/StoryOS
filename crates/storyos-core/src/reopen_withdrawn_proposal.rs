//! Classify one explicit reopen of a withdrawn Proposal.

use super::TransitionOutcome;
use crate::transition_outcome::reason_codes;

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

pub type ReopenWithdrawnProposalResult = TransitionOutcome<
    (),
    ReopenWithdrawnProposalNoEffect,
    ReopenWithdrawnProposalConflict,
    ReopenWithdrawnProposalRefusal,
>;

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

reason_codes!(ReopenWithdrawnProposalConflict { ChangedHead => "changed_head" });
reason_codes!(ReopenWithdrawnProposalRefusal {
    WrongScope => "wrong_scope",
    WrongAdmission => "wrong_admission",
    StaleProposalRevision => "stale_proposal_revision",
});
reason_codes!(ReopenWithdrawnProposalNoEffect {
    TerminalSupersession => "terminal_supersession",
    ClosureNotWithdrawn => "closure_not_withdrawn",
    WithdrawalEventMismatch => "withdrawal_event_mismatch",
});

/// Classify one permitted reopen of a withdrawn Proposal.
pub fn reopen_withdrawn_proposal(
    command: &ReopenWithdrawnProposal,
) -> ReopenWithdrawnProposalResult {
    if !command.scope_matches {
        return TransitionOutcome::Refused(ReopenWithdrawnProposalRefusal::WrongScope);
    }
    if command.terminal_supersession {
        return TransitionOutcome::NoEffect(ReopenWithdrawnProposalNoEffect::TerminalSupersession);
    }
    if !command.proposal_revision_current {
        return TransitionOutcome::Refused(ReopenWithdrawnProposalRefusal::StaleProposalRevision);
    }
    if !command.admission_valid {
        return TransitionOutcome::Refused(ReopenWithdrawnProposalRefusal::WrongAdmission);
    }
    if !command.withdrawal_event_matches {
        return TransitionOutcome::NoEffect(
            ReopenWithdrawnProposalNoEffect::WithdrawalEventMismatch,
        );
    }
    if !command.closure_withdrawn {
        return TransitionOutcome::NoEffect(ReopenWithdrawnProposalNoEffect::ClosureNotWithdrawn);
    }
    if !command.expected_target_matches_head {
        return TransitionOutcome::Conflicted(ReopenWithdrawnProposalConflict::ChangedHead);
    }
    TransitionOutcome::Applied(())
}

#[cfg(test)]
#[path = "reopen_withdrawn_proposal_tests.rs"]
mod tests;
