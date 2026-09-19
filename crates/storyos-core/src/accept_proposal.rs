//! Classify one Acceptance Attempt without changing Authoritative State.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptProposal {
    pub scope_matches: bool,
    pub admission_valid: bool,
    pub proposal_revision_current: bool,
    pub retention_retained: bool,
    pub generation_ready: bool,
    pub closure_open: bool,
    pub validation_current: bool,
    pub validation_receipt_valid: bool,
    pub validation_receipt_matches_revision: bool,
    pub selected_operation_pending: bool,
    pub selection_duplicate_free: bool,
    pub required_dependencies_met: bool,
    pub bundle_closure_complete: bool,
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
    DuplicateIdentities,
    MissingRequiredDependencies,
    IncompleteBundleClosure,
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

/// One durable Operation used to classify a declared selection set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalOperationSelection {
    pub operation_id: String,
    pub resolution: String,
    pub predecessor_operation_ids: Vec<String>,
}

/// Observable selection-set facts for one declared Accept or Reject.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalSelectionFacts {
    pub duplicate_free: bool,
    pub all_selected_pending: bool,
    pub required_dependencies_met: bool,
    pub bundle_closure_complete: bool,
}

/// Bundle closure rule for one declared selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProposalBundlePolicy {
    None,
    Atomic,
}

/// Decision kind that owns the predecessor-satisfaction rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProposalSelectionIntent {
    Accept,
    Reject,
}

impl ProposalBundlePolicy {
    /// Map one stored Proposal Bundle policy token.
    pub fn from_stored(value: &str) -> Self {
        if value == "atomic" {
            Self::Atomic
        } else {
            Self::None
        }
    }
}

/// Classify one declared Operation set against pending incarnations, dependencies, and Bundle policy.
pub fn classify_proposal_selection(
    selected_ids: &[String],
    operations: &[ProposalOperationSelection],
    bundle_policy: ProposalBundlePolicy,
    intent: ProposalSelectionIntent,
) -> ProposalSelectionFacts {
    let mut seen = std::collections::BTreeSet::new();
    let duplicate_free = selected_ids.iter().all(|id| seen.insert(id.as_str()));
    let selected: std::collections::BTreeSet<&str> =
        selected_ids.iter().map(String::as_str).collect();
    let by_id: std::collections::BTreeMap<&str, &ProposalOperationSelection> = operations
        .iter()
        .map(|operation| (operation.operation_id.as_str(), operation))
        .collect();
    let all_selected_pending = !selected.is_empty()
        && selected.iter().all(|id| {
            by_id
                .get(id)
                .is_some_and(|operation| operation.resolution == "pending")
        });
    let required_dependencies_met = selected.iter().all(|id| {
        let Some(operation) = by_id.get(id) else {
            return false;
        };
        operation
            .predecessor_operation_ids
            .iter()
            .all(|predecessor| {
                by_id.get(predecessor.as_str()).is_some_and(|required| {
                    selected.contains(required.operation_id.as_str())
                        || match intent {
                            ProposalSelectionIntent::Accept => required.resolution == "applied",
                            ProposalSelectionIntent::Reject => required.resolution != "pending",
                        }
                })
            })
    });
    let pending: std::collections::BTreeSet<&str> = operations
        .iter()
        .filter(|operation| operation.resolution == "pending")
        .map(|operation| operation.operation_id.as_str())
        .collect();
    let bundle_closure_complete =
        bundle_policy != ProposalBundlePolicy::Atomic || pending == selected;
    ProposalSelectionFacts {
        duplicate_free,
        all_selected_pending,
        required_dependencies_met,
        bundle_closure_complete,
    }
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
    if !command.retention_retained
        || !command.generation_ready
        || !command.closure_open
        || !command.validation_current
    {
        return AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::NotEligible,
        };
    }
    if !command.validation_receipt_valid || !command.validation_receipt_matches_revision {
        return AcceptProposalResult::Invalid {
            reason: AcceptProposalInvalid::InvalidValidation,
        };
    }
    if !command.selection_duplicate_free {
        return AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::DuplicateIdentities,
        };
    }
    if !command.selected_operation_pending {
        return AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::OperationNotPending,
        };
    }
    if !command.required_dependencies_met {
        return AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::MissingRequiredDependencies,
        };
    }
    if !command.bundle_closure_complete {
        return AcceptProposalResult::Refused {
            reason: AcceptProposalRefusal::IncompleteBundleClosure,
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
