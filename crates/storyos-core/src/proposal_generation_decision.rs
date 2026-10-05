//! Classify Complete and Continue without accepting Proposal content.

use std::convert::Infallible;

use super::TransitionOutcome;
use crate::transition_outcome::reason_codes;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteReadyPartialProposal {
    pub revision_current: bool,
    pub closure_open: bool,
    pub generation_state: String,
    pub generation_id_matches: bool,
    pub candidate_digest_matches: bool,
    pub stream_seq_matches: bool,
    pub expected_target_matches_head: bool,
}

pub type CompleteReadyPartialProposalResult = TransitionOutcome<
    (),
    Infallible,
    ProposalGenerationConflict,
    CompleteReadyPartialProposalRefusal,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompleteReadyPartialProposalRefusal {
    StaleProposalRevision,
    NotEligible,
    NotReadyPartial,
    StaleGeneration,
    StaleCandidate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinueProposalGeneration {
    pub revision_current: bool,
    pub closure_open: bool,
    pub generation_state: String,
    pub expected_generation_state: String,
    pub generation_id_matches: bool,
    pub candidate_digest_matches: bool,
    pub selected_operations_pending: bool,
    pub selection_duplicate_free: bool,
    pub expected_target_matches_head: bool,
}

pub type ContinueProposalGenerationResult = TransitionOutcome<
    (),
    Infallible,
    ProposalGenerationConflict,
    ContinueProposalGenerationRefusal,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContinueProposalGenerationRefusal {
    StaleProposalRevision,
    NotEligible,
    NotContinuable,
    StaleGeneration,
    StaleCandidate,
    OperationNotPending,
    DuplicateIdentities,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalGenerationConflict {
    ChangedHead,
}

reason_codes!(ProposalGenerationConflict { ChangedHead => "changed_head" });
reason_codes!(CompleteReadyPartialProposalRefusal {
    StaleProposalRevision => "stale_proposal_revision",
    NotEligible => "not_eligible",
    NotReadyPartial => "not_ready_partial",
    StaleGeneration => "stale_generation",
    StaleCandidate => "stale_candidate",
});
reason_codes!(ContinueProposalGenerationRefusal {
    StaleProposalRevision => "stale_proposal_revision",
    NotEligible => "not_eligible",
    NotContinuable => "not_continuable",
    StaleGeneration => "stale_generation",
    StaleCandidate => "stale_candidate",
    OperationNotPending => "operation_not_pending",
    DuplicateIdentities => "duplicate_identities",
});

/// Classify an explicit completion of the current partial candidate.
pub fn complete_ready_partial_proposal(
    command: &CompleteReadyPartialProposal,
) -> CompleteReadyPartialProposalResult {
    if !command.revision_current {
        return TransitionOutcome::Refused(
            CompleteReadyPartialProposalRefusal::StaleProposalRevision,
        );
    }
    if !command.closure_open {
        return TransitionOutcome::Refused(CompleteReadyPartialProposalRefusal::NotEligible);
    }
    if command.generation_state != "ready_partial" {
        return TransitionOutcome::Refused(CompleteReadyPartialProposalRefusal::NotReadyPartial);
    }
    if !command.generation_id_matches {
        return TransitionOutcome::Refused(CompleteReadyPartialProposalRefusal::StaleGeneration);
    }
    if !command.candidate_digest_matches || !command.stream_seq_matches {
        return TransitionOutcome::Refused(CompleteReadyPartialProposalRefusal::StaleCandidate);
    }
    if !command.expected_target_matches_head {
        return TransitionOutcome::Conflicted(ProposalGenerationConflict::ChangedHead);
    }
    TransitionOutcome::Applied(())
}

/// Classify an explicit continuation onto a fresh Generation.
pub fn continue_proposal_generation(
    command: &ContinueProposalGeneration,
) -> ContinueProposalGenerationResult {
    if !command.revision_current {
        return TransitionOutcome::Refused(
            ContinueProposalGenerationRefusal::StaleProposalRevision,
        );
    }
    if !command.closure_open {
        return TransitionOutcome::Refused(ContinueProposalGenerationRefusal::NotEligible);
    }
    if command.expected_generation_state != command.generation_state
        || !matches!(command.generation_state.as_str(), "ready_partial" | "ready")
    {
        return TransitionOutcome::Refused(ContinueProposalGenerationRefusal::NotContinuable);
    }
    if !command.generation_id_matches {
        return TransitionOutcome::Refused(ContinueProposalGenerationRefusal::StaleGeneration);
    }
    if !command.candidate_digest_matches {
        return TransitionOutcome::Refused(ContinueProposalGenerationRefusal::StaleCandidate);
    }
    if !command.selection_duplicate_free {
        return TransitionOutcome::Refused(ContinueProposalGenerationRefusal::DuplicateIdentities);
    }
    if !command.selected_operations_pending {
        return TransitionOutcome::Refused(ContinueProposalGenerationRefusal::OperationNotPending);
    }
    if !command.expected_target_matches_head {
        return TransitionOutcome::Conflicted(ProposalGenerationConflict::ChangedHead);
    }
    TransitionOutcome::Applied(())
}

#[cfg(test)]
#[path = "proposal_generation_decision_tests.rs"]
mod tests;
