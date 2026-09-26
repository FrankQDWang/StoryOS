//! Classify Complete and Continue without accepting Proposal content.

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompleteReadyPartialProposalResult {
    Completed,
    Conflicted {
        reason: ProposalGenerationConflict,
    },
    Refused {
        reason: CompleteReadyPartialProposalRefusal,
    },
}

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContinueProposalGenerationResult {
    Started,
    Conflicted {
        reason: ProposalGenerationConflict,
    },
    Refused {
        reason: ContinueProposalGenerationRefusal,
    },
}

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

/// Classify an explicit completion of the current partial candidate.
pub fn complete_ready_partial_proposal(
    command: &CompleteReadyPartialProposal,
) -> CompleteReadyPartialProposalResult {
    if !command.revision_current {
        return CompleteReadyPartialProposalResult::Refused {
            reason: CompleteReadyPartialProposalRefusal::StaleProposalRevision,
        };
    }
    if !command.closure_open {
        return CompleteReadyPartialProposalResult::Refused {
            reason: CompleteReadyPartialProposalRefusal::NotEligible,
        };
    }
    if command.generation_state != "ready_partial" {
        return CompleteReadyPartialProposalResult::Refused {
            reason: CompleteReadyPartialProposalRefusal::NotReadyPartial,
        };
    }
    if !command.generation_id_matches {
        return CompleteReadyPartialProposalResult::Refused {
            reason: CompleteReadyPartialProposalRefusal::StaleGeneration,
        };
    }
    if !command.candidate_digest_matches || !command.stream_seq_matches {
        return CompleteReadyPartialProposalResult::Refused {
            reason: CompleteReadyPartialProposalRefusal::StaleCandidate,
        };
    }
    if !command.expected_target_matches_head {
        return CompleteReadyPartialProposalResult::Conflicted {
            reason: ProposalGenerationConflict::ChangedHead,
        };
    }
    CompleteReadyPartialProposalResult::Completed
}

/// Classify an explicit continuation onto a fresh Generation.
pub fn continue_proposal_generation(
    command: &ContinueProposalGeneration,
) -> ContinueProposalGenerationResult {
    if !command.revision_current {
        return ContinueProposalGenerationResult::Refused {
            reason: ContinueProposalGenerationRefusal::StaleProposalRevision,
        };
    }
    if !command.closure_open {
        return ContinueProposalGenerationResult::Refused {
            reason: ContinueProposalGenerationRefusal::NotEligible,
        };
    }
    if command.expected_generation_state != command.generation_state
        || !matches!(command.generation_state.as_str(), "ready_partial" | "ready")
    {
        return ContinueProposalGenerationResult::Refused {
            reason: ContinueProposalGenerationRefusal::NotContinuable,
        };
    }
    if !command.generation_id_matches {
        return ContinueProposalGenerationResult::Refused {
            reason: ContinueProposalGenerationRefusal::StaleGeneration,
        };
    }
    if !command.candidate_digest_matches {
        return ContinueProposalGenerationResult::Refused {
            reason: ContinueProposalGenerationRefusal::StaleCandidate,
        };
    }
    if !command.selection_duplicate_free {
        return ContinueProposalGenerationResult::Refused {
            reason: ContinueProposalGenerationRefusal::DuplicateIdentities,
        };
    }
    if !command.selected_operations_pending {
        return ContinueProposalGenerationResult::Refused {
            reason: ContinueProposalGenerationRefusal::OperationNotPending,
        };
    }
    if !command.expected_target_matches_head {
        return ContinueProposalGenerationResult::Conflicted {
            reason: ProposalGenerationConflict::ChangedHead,
        };
    }
    ContinueProposalGenerationResult::Started
}
