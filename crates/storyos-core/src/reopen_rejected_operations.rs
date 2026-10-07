//! Classify one explicit reopen of rejected Proposal Operations.

use std::convert::Infallible;

use super::TransitionOutcome;
use crate::transition_outcome::reason_codes;

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

pub type ReopenRejectedOperationsResult = TransitionOutcome<
    (),
    Infallible,
    ReopenRejectedOperationsConflict,
    ReopenRejectedOperationsRefusal,
>;

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

reason_codes!(ReopenRejectedOperationsConflict { ChangedHead => "changed_head" });
reason_codes!(ReopenRejectedOperationsRefusal {
    WrongScope => "wrong_scope",
    WrongAdmission => "wrong_admission",
    StaleProposalRevision => "stale_proposal_revision",
    NotEligible => "not_eligible",
    OperationNotRejected => "operation_not_rejected",
    UnavailableProof => "unavailable_proof",
});

/// Classify one explicit reopen against Scope, Admission, current Revision, proof, and Head.
pub fn reopen_rejected_operations(
    command: &ReopenRejectedOperations,
) -> ReopenRejectedOperationsResult {
    if !command.scope_matches {
        return TransitionOutcome::Refused(ReopenRejectedOperationsRefusal::WrongScope);
    }
    if !command.admission_valid {
        return TransitionOutcome::Refused(ReopenRejectedOperationsRefusal::WrongAdmission);
    }
    if !command.proposal_revision_current {
        return TransitionOutcome::Refused(ReopenRejectedOperationsRefusal::StaleProposalRevision);
    }
    if !command.closure_open || !command.reservation_available {
        return TransitionOutcome::Refused(ReopenRejectedOperationsRefusal::NotEligible);
    }
    if !command.selected_operations_rejected {
        return TransitionOutcome::Refused(ReopenRejectedOperationsRefusal::OperationNotRejected);
    }
    if !command.rejection_event_matches {
        return TransitionOutcome::Refused(ReopenRejectedOperationsRefusal::UnavailableProof);
    }
    if !command.expected_target_matches_head {
        return TransitionOutcome::Conflicted(ReopenRejectedOperationsConflict::ChangedHead);
    }
    TransitionOutcome::Applied(())
}

#[cfg(test)]
#[path = "reopen_rejected_operations_tests.rs"]
mod tests;
