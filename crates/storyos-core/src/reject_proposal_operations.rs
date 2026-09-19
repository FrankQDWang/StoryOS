//! Classify one explicit Rejection without changing Authoritative State.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectProposalOperations {
    pub scope_matches: bool,
    pub admission_valid: bool,
    pub proposal_revision_current: bool,
    pub closure_open: bool,
    pub selected_operations_pending: bool,
    pub selection_duplicate_free: bool,
    pub required_dependencies_met: bool,
    pub bundle_closure_complete: bool,
    pub expected_target_matches_head: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RejectProposalOperationsResult {
    Resolved,
    Conflicted {
        reason: RejectProposalOperationsConflict,
    },
    Refused {
        reason: RejectProposalOperationsRefusal,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RejectProposalOperationsRefusal {
    WrongScope,
    WrongAdmission,
    StaleProposalRevision,
    NotEligible,
    OperationNotPending,
    DuplicateIdentities,
    MissingRequiredDependencies,
    IncompleteBundleClosure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RejectProposalOperationsConflict {
    ChangedHead,
}

/// Classify one explicit Rejection against Scope, Admission, current Revision, and Head.
pub fn reject_proposal_operations(
    command: &RejectProposalOperations,
) -> RejectProposalOperationsResult {
    if !command.scope_matches {
        return RejectProposalOperationsResult::Refused {
            reason: RejectProposalOperationsRefusal::WrongScope,
        };
    }
    if !command.admission_valid {
        return RejectProposalOperationsResult::Refused {
            reason: RejectProposalOperationsRefusal::WrongAdmission,
        };
    }
    if !command.proposal_revision_current {
        return RejectProposalOperationsResult::Refused {
            reason: RejectProposalOperationsRefusal::StaleProposalRevision,
        };
    }
    if !command.closure_open {
        return RejectProposalOperationsResult::Refused {
            reason: RejectProposalOperationsRefusal::NotEligible,
        };
    }
    if !command.selection_duplicate_free {
        return RejectProposalOperationsResult::Refused {
            reason: RejectProposalOperationsRefusal::DuplicateIdentities,
        };
    }
    if !command.selected_operations_pending {
        return RejectProposalOperationsResult::Refused {
            reason: RejectProposalOperationsRefusal::OperationNotPending,
        };
    }
    if !command.required_dependencies_met {
        return RejectProposalOperationsResult::Refused {
            reason: RejectProposalOperationsRefusal::MissingRequiredDependencies,
        };
    }
    if !command.bundle_closure_complete {
        return RejectProposalOperationsResult::Refused {
            reason: RejectProposalOperationsRefusal::IncompleteBundleClosure,
        };
    }
    if !command.expected_target_matches_head {
        return RejectProposalOperationsResult::Conflicted {
            reason: RejectProposalOperationsConflict::ChangedHead,
        };
    }
    RejectProposalOperationsResult::Resolved
}

#[cfg(test)]
#[path = "reject_proposal_operations_tests.rs"]
mod tests;
