//! Classify one Acceptance Attempt without changing Authoritative State.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptProposal {
    pub scope_matches: bool,
    pub admission_valid: bool,
    pub proposal_revision_current: bool,
    pub retention_retained: bool,
    pub generation_ready: bool,
    pub closure_open: bool,
    pub validation_receipt_valid: bool,
    pub validation_receipt_matches_revision: bool,
    pub selected_operation_pending: bool,
    pub expected_target_matches_head: bool,
    pub candidate_unaltered: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptProposalResult {
    Applied,
    Invalid { reason: AcceptProposalInvalid },
    Conflicted { reason: AcceptProposalConflict },
    Refused { reason: AcceptProposalRefusal },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptProposalRefusal {
    WrongScope,
    WrongAdmission,
    StaleProposalRevision,
    NotEligible,
    OperationNotPending,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptProposalInvalid {
    InvalidValidation,
    AlteredCandidate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptProposalConflict {
    ChangedHead,
}

/// Classify one exact Operation Acceptance against Scope, Admission, eligibility, and current Head.
pub fn accept_proposal(command: &AcceptProposal) -> AcceptProposalResult {
    if !command.scope_matches {
        return AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::WrongScope,
        };
    }
    if !command.admission_valid {
        return AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::WrongAdmission,
        };
    }
    if !command.proposal_revision_current {
        return AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::StaleProposalRevision,
        };
    }
    if !command.retention_retained || !command.generation_ready || !command.closure_open {
        return AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::NotEligible,
        };
    }
    if !command.validation_receipt_valid || !command.validation_receipt_matches_revision {
        return AcceptProposalResult::Invalid {
            reason: AcceptProposalInvalid::InvalidValidation,
        };
    }
    if !command.selected_operation_pending {
        return AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::OperationNotPending,
        };
    }
    if !command.expected_target_matches_head {
        return AcceptProposalResult::Conflicted {
            reason: AcceptProposalConflict::ChangedHead,
        };
    }
    if !command.candidate_unaltered {
        return AcceptProposalResult::Invalid {
            reason: AcceptProposalInvalid::AlteredCandidate,
        };
    }
    AcceptProposalResult::Applied
}

#[cfg(test)]
#[path = "accept_proposal_tests.rs"]
mod tests;
