//! Classify one explicit Rejection without changing Authoritative State.

use std::convert::Infallible;

use super::TransitionOutcome;
use crate::transition_outcome::reason_codes;

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

pub type RejectProposalOperationsResult = TransitionOutcome<
    (),
    Infallible,
    RejectProposalOperationsConflict,
    RejectProposalOperationsRefusal,
>;

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

reason_codes!(RejectProposalOperationsConflict { ChangedHead => "changed_head" });
reason_codes!(RejectProposalOperationsRefusal {
    WrongScope => "wrong_scope",
    WrongAdmission => "wrong_admission",
    StaleProposalRevision => "stale_proposal_revision",
    NotEligible => "not_eligible",
    OperationNotPending => "operation_not_pending",
    DuplicateIdentities => "duplicate_identities",
    MissingRequiredDependencies => "missing_required_dependencies",
    IncompleteBundleClosure => "incomplete_bundle_closure",
});

/// Classify one explicit Rejection against Scope, Admission, current Revision, and Head.
pub fn reject_proposal_operations(
    command: &RejectProposalOperations,
) -> RejectProposalOperationsResult {
    if !command.scope_matches {
        return TransitionOutcome::Refused(RejectProposalOperationsRefusal::WrongScope);
    }
    if !command.admission_valid {
        return TransitionOutcome::Refused(RejectProposalOperationsRefusal::WrongAdmission);
    }
    if !command.proposal_revision_current {
        return TransitionOutcome::Refused(RejectProposalOperationsRefusal::StaleProposalRevision);
    }
    if !command.closure_open {
        return TransitionOutcome::Refused(RejectProposalOperationsRefusal::NotEligible);
    }
    if !command.selection_duplicate_free {
        return TransitionOutcome::Refused(RejectProposalOperationsRefusal::DuplicateIdentities);
    }
    if !command.selected_operations_pending {
        return TransitionOutcome::Refused(RejectProposalOperationsRefusal::OperationNotPending);
    }
    if !command.required_dependencies_met {
        return TransitionOutcome::Refused(
            RejectProposalOperationsRefusal::MissingRequiredDependencies,
        );
    }
    if !command.bundle_closure_complete {
        return TransitionOutcome::Refused(
            RejectProposalOperationsRefusal::IncompleteBundleClosure,
        );
    }
    if !command.expected_target_matches_head {
        return TransitionOutcome::Conflicted(RejectProposalOperationsConflict::ChangedHead);
    }
    TransitionOutcome::Applied(())
}

#[cfg(test)]
#[path = "reject_proposal_operations_tests.rs"]
mod tests;
