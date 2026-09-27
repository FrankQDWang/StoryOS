//! Classify one explicit Replan of a conflicted Proposal.

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplanProposalResult {
    Resolved,
    Conflicted { reason: ReplanProposalConflict },
    Refused { reason: ReplanProposalRefusal },
}

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

/// Classify one author-cause Replan against Scope, Admission, current Conflict, and Heads.
pub fn replan_proposal(command: &ReplanProposal) -> ReplanProposalResult {
    if !command.scope_matches {
        return ReplanProposalResult::Refused {
            reason: ReplanProposalRefusal::WrongScope,
        };
    }
    if !command.admission_valid {
        return ReplanProposalResult::Refused {
            reason: ReplanProposalRefusal::WrongAdmission,
        };
    }
    if !command.proposal_revision_current || !command.expected_head_current {
        return ReplanProposalResult::Refused {
            reason: ReplanProposalRefusal::StaleProposalRevision,
        };
    }
    if !command.closure_open {
        return ReplanProposalResult::Refused {
            reason: ReplanProposalRefusal::NotEligible,
        };
    }
    if !command.source_condition_matches || !command.replacement_operations_preserve_identity {
        return ReplanProposalResult::Refused {
            reason: ReplanProposalRefusal::UnavailableProof,
        };
    }
    if !command.expected_target_matches_head {
        return ReplanProposalResult::Conflicted {
            reason: ReplanProposalConflict::ChangedHead,
        };
    }
    ReplanProposalResult::Resolved
}

#[cfg(test)]
#[path = "replan_proposal_tests.rs"]
mod tests;
