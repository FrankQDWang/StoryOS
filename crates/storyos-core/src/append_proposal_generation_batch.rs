//! Classify one canonical Proposal generation batch without I/O.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppendProposalGenerationBatch {
    pub scope_matches: bool,
    pub generation_state: String,
    pub last_applied_stream_seq: u64,
    pub stream_seq: u64,
    pub expected_previous_stream_seq: u64,
    pub expected_proposal_revision_id: String,
    pub current_proposal_revision_id: String,
    pub expected_candidate_digest: String,
    pub current_candidate_digest: String,
    pub batch_digest: String,
    pub existing_batch_digest: Option<String>,
    pub reservation_owns_target: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppendProposalGenerationBatchResult {
    Applied,
    Duplicate,
    Wait,
    Refused {
        reason: AppendProposalGenerationBatchRefusal,
    },
    Conflicted {
        reason: AppendProposalGenerationBatchConflict,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppendProposalGenerationBatchRefusal {
    WrongScope,
    GenerationClosed,
    ReservationMismatch,
    StaleHead,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppendProposalGenerationBatchConflict {
    DigestMismatch,
}

/// Classify one canonical batch against sequence, digest, reservation, and fence state.
pub fn append_proposal_generation_batch(
    command: &AppendProposalGenerationBatch,
) -> AppendProposalGenerationBatchResult {
    if !command.scope_matches {
        return AppendProposalGenerationBatchResult::Refused {
            reason: AppendProposalGenerationBatchRefusal::WrongScope,
        };
    }
    if command.generation_state != "generating" {
        return AppendProposalGenerationBatchResult::Refused {
            reason: AppendProposalGenerationBatchRefusal::GenerationClosed,
        };
    }
    if !command.reservation_owns_target {
        return AppendProposalGenerationBatchResult::Refused {
            reason: AppendProposalGenerationBatchRefusal::ReservationMismatch,
        };
    }
    if let Some(existing) = command.existing_batch_digest.as_deref() {
        return if existing == command.batch_digest {
            AppendProposalGenerationBatchResult::Duplicate
        } else {
            AppendProposalGenerationBatchResult::Conflicted {
                reason: AppendProposalGenerationBatchConflict::DigestMismatch,
            }
        };
    }
    if command.stream_seq > command.last_applied_stream_seq + 1
        || command.expected_previous_stream_seq != command.last_applied_stream_seq
    {
        return AppendProposalGenerationBatchResult::Wait;
    }
    if command.stream_seq != command.last_applied_stream_seq + 1
        || command.expected_proposal_revision_id != command.current_proposal_revision_id
        || command.expected_candidate_digest != command.current_candidate_digest
    {
        return AppendProposalGenerationBatchResult::Refused {
            reason: AppendProposalGenerationBatchRefusal::StaleHead,
        };
    }
    AppendProposalGenerationBatchResult::Applied
}

#[cfg(test)]
#[path = "append_proposal_generation_batch_tests.rs"]
mod tests;
