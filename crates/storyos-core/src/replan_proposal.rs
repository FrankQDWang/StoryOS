//! Classify one explicit Replan of a conflicted Proposal.

use std::convert::Infallible;

use super::TransitionOutcome;
use crate::transition_outcome::reason_codes;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplanProposal {
    pub scope_matches: bool,
    pub admission_valid: bool,
    pub proposal_revision_current: bool,
    pub expected_head_current: bool,
    pub closure_open: bool,
    pub source_condition_matches: bool,
    pub replacement_operations_preserve_identity: bool,
    pub expected_target_matches_head: bool,
}

pub type ReplanProposalResult =
    TransitionOutcome<(), Infallible, ReplanProposalConflict, ReplanProposalRefusal>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplanProposalRefusal {
    WrongScope,
    WrongAdmission,
    StaleProposalRevision,
    NotEligible,
    UnavailableProof,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplanProposalConflict {
    ChangedHead,
}

reason_codes!(ReplanProposalConflict { ChangedHead => "changed_head" });
reason_codes!(ReplanProposalRefusal {
    WrongScope => "wrong_scope",
    WrongAdmission => "wrong_admission",
    StaleProposalRevision => "stale_proposal_revision",
    NotEligible => "not_eligible",
    UnavailableProof => "unavailable_proof",
});

/// Classify one author-cause Replan against Scope, Admission, current Conflict, and Heads.
pub fn replan_proposal(command: &ReplanProposal) -> ReplanProposalResult {
    if !command.scope_matches {
        return TransitionOutcome::Refused(ReplanProposalRefusal::WrongScope);
    }
    if !command.admission_valid {
        return TransitionOutcome::Refused(ReplanProposalRefusal::WrongAdmission);
    }
    if !command.proposal_revision_current || !command.expected_head_current {
        return TransitionOutcome::Refused(ReplanProposalRefusal::StaleProposalRevision);
    }
    if !command.closure_open {
        return TransitionOutcome::Refused(ReplanProposalRefusal::NotEligible);
    }
    if !command.source_condition_matches || !command.replacement_operations_preserve_identity {
        return TransitionOutcome::Refused(ReplanProposalRefusal::UnavailableProof);
    }
    if !command.expected_target_matches_head {
        return TransitionOutcome::Conflicted(ReplanProposalConflict::ChangedHead);
    }
    TransitionOutcome::Applied(())
}

#[cfg(test)]
#[path = "replan_proposal_tests.rs"]
mod tests;
