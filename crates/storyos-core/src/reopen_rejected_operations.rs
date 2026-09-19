//! Classify one explicit reopen of rejected Proposal Operations.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReopenRejectedOperations {
    pub scope_matches: bool,
    pub admission_valid: bool,
    pub proposal_revision_current: bool,
    pub closure_open: bool,
    pub selected_operations_rejected: bool,
    pub rejection_event_matches: bool,
    pub reservation_available: bool,
    pub expected_target_matches_head: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReopenRejectedOperationsResult {
    Resolved,
    Conflicted {
        reason: ReopenRejectedOperationsConflict,
    },
    Refused {
        reason: ReopenRejectedOperationsRefusal,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReopenRejectedOperationsRefusal {
    WrongScope,
    WrongAdmission,
    StaleProposalRevision,
    NotEligible,
    OperationNotRejected,
    UnavailableProof,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReopenRejectedOperationsConflict {
    ChangedHead,
}

/// Classify one explicit reopen against Scope, Admission, current Revision, proof, and Head.
pub fn reopen_rejected_operations(
    command: &ReopenRejectedOperations,
) -> ReopenRejectedOperationsResult {
    if !command.scope_matches {
        return ReopenRejectedOperationsResult::Refused {
            reason: ReopenRejectedOperationsRefusal::WrongScope,
        };
    }
    if !command.admission_valid {
        return ReopenRejectedOperationsResult::Refused {
            reason: ReopenRejectedOperationsRefusal::WrongAdmission,
        };
    }
    if !command.proposal_revision_current {
        return ReopenRejectedOperationsResult::Refused {
            reason: ReopenRejectedOperationsRefusal::StaleProposalRevision,
        };
    }
    if !command.closure_open || !command.reservation_available {
        return ReopenRejectedOperationsResult::Refused {
            reason: ReopenRejectedOperationsRefusal::NotEligible,
        };
    }
    if !command.selected_operations_rejected {
        return ReopenRejectedOperationsResult::Refused {
            reason: ReopenRejectedOperationsRefusal::OperationNotRejected,
        };
    }
    if !command.rejection_event_matches {
        return ReopenRejectedOperationsResult::Refused {
            reason: ReopenRejectedOperationsRefusal::UnavailableProof,
        };
    }
    if !command.expected_target_matches_head {
        return ReopenRejectedOperationsResult::Conflicted {
            reason: ReopenRejectedOperationsConflict::ChangedHead,
        };
    }
    ReopenRejectedOperationsResult::Resolved
}

#[cfg(test)]
#[path = "reopen_rejected_operations_tests.rs"]
mod tests;
