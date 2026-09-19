//! Classify one Editor Input Fence pause without allocating an Author Action.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseProposalGeneration {
    pub scope_matches: bool,
    pub generation_state: String,
    pub expected_proposal_revision_id: String,
    pub current_proposal_revision_id: String,
    pub expected_candidate_digest: String,
    pub current_candidate_digest: String,
    pub last_applied_stream_seq: u64,
    pub admitted_through_seq: u64,
    pub existing_fence: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PauseProposalGenerationResult {
    Applied {
        allocates_author_action: bool,
    },
    Duplicate {
        allocates_author_action: bool,
    },
    Refused {
        reason: PauseProposalGenerationRefusal,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PauseProposalGenerationRefusal {
    WrongScope,
    NotGenerating,
    StaleHead,
}

/// Classify one author-input pause at the admitted Head.
pub fn pause_proposal_generation(
    command: &PauseProposalGeneration,
) -> PauseProposalGenerationResult {
    if !command.scope_matches {
        return PauseProposalGenerationResult::Refused {
            reason: PauseProposalGenerationRefusal::WrongScope,
        };
    }
    if command.existing_fence {
        return PauseProposalGenerationResult::Duplicate {
            allocates_author_action: false,
        };
    }
    if command.generation_state != "generating" {
        return PauseProposalGenerationResult::Refused {
            reason: PauseProposalGenerationRefusal::NotGenerating,
        };
    }
    if command.expected_proposal_revision_id != command.current_proposal_revision_id
        || command.expected_candidate_digest != command.current_candidate_digest
        || command.admitted_through_seq != command.last_applied_stream_seq
    {
        return PauseProposalGenerationResult::Refused {
            reason: PauseProposalGenerationRefusal::StaleHead,
        };
    }
    PauseProposalGenerationResult::Applied {
        allocates_author_action: false,
    }
}

#[cfg(test)]
#[path = "pause_proposal_generation_tests.rs"]
mod tests;
