//! Classify one BlockEditProposal open without changing Authoritative State.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenBlockProposal {
    pub scope_matches: bool,
    pub target_block_present: bool,
    pub expected_base_revision_id: String,
    pub current_base_revision_id: Option<String>,
    pub conflicting_reservation: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenBlockProposalResult {
    Applied,
    Refused { reason: OpenBlockProposalRefusal },
    Conflicted { reason: OpenBlockProposalConflict },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenBlockProposalRefusal {
    WrongScope,
    UnavailableTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenBlockProposalConflict {
    ChangedHead,
    ConflictingReservation,
}

/// Classify one prose-handler Block Proposal against exact Scope, target, Head, and reservation.
pub fn open_block_proposal(command: &OpenBlockProposal) -> OpenBlockProposalResult {
    if !command.scope_matches {
        return OpenBlockProposalResult::Refused {
            reason: OpenBlockProposalRefusal::WrongScope,
        };
    }
    if !command.target_block_present {
        return OpenBlockProposalResult::Refused {
            reason: OpenBlockProposalRefusal::UnavailableTarget,
        };
    }
    match command.current_base_revision_id.as_deref() {
        Some(current) if current == command.expected_base_revision_id => {}
        Some(_) => {
            return OpenBlockProposalResult::Conflicted {
                reason: OpenBlockProposalConflict::ChangedHead,
            };
        }
        None => {
            return OpenBlockProposalResult::Refused {
                reason: OpenBlockProposalRefusal::UnavailableTarget,
            };
        }
    }
    if command.conflicting_reservation {
        return OpenBlockProposalResult::Conflicted {
            reason: OpenBlockProposalConflict::ConflictingReservation,
        };
    }
    OpenBlockProposalResult::Applied
}

impl OpenBlockProposalResult {
    /// Core-issued Validation Receipt result for one successful open.
    pub fn validation_receipt_result(&self) -> Option<&'static str> {
        match self {
            Self::Applied => Some("valid"),
            Self::Refused { .. } | Self::Conflicted { .. } => None,
        }
    }
}

#[cfg(test)]
#[path = "open_block_proposal_tests.rs"]
mod tests;
