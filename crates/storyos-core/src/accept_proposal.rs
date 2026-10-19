//! Classify one Acceptance Attempt without changing Authoritative State.

use crate::TransitionOutcome;
use crate::transition_outcome::reason_codes;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptProposal {
    pub scope_matches: bool,
    pub admission_valid: bool,
    pub proposal_revision_current: bool,
    pub retention_retained: bool,
    pub generation_ready: bool,
    pub closure_open: bool,
    /// The validation of the Proposal Revision.
    pub validation: AcceptanceValidation,
    /// The Acceptance condition of the Proposal Revision.
    pub condition: AcceptanceCondition,
    /// The result of the Validation Receipt that the Acceptance names.
    pub validation_receipt: AcceptanceValidation,
    pub validation_receipt_matches_revision: bool,
    pub selected_operation_pending: bool,
    pub selection_duplicate_free: bool,
    pub required_dependencies_met: bool,
    pub bundle_closure_complete: bool,
    pub target_kind: AcceptanceTargetKind,
    pub base: AcceptanceBase,
    pub candidate_unaltered: bool,
}

/// The validation of a Proposal Revision or the result of its Validation Receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptanceValidation {
    Valid,
    Conflicted,
    /// `pending` or `invalid`.
    Other,
}

/// The Acceptance condition that a Proposal Revision has.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptanceCondition {
    Absent,
    ProposalConflict,
    RecoveryConflict,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptanceTargetKind {
    Block,
    Inline,
}

/// The Proposal Revision base compared with the current Chapter Head.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptanceBase {
    /// The base is the Chapter Head.
    ExactHead,
    /// The base is behind the Chapter Head.
    EarlierHead {
        target_blocks_present: bool,
        /// The exact Inline Anchor proof of ADR 0063 maps the Anchor range to the Chapter Head.
        inline_range_proven: bool,
    },
}

/// How an applied Acceptance found its target on the Chapter Head (ADR 0064).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptanceTargetResolution {
    ExactBase,
    BlockReplacement,
    ProvenRange,
    BaseBlockReplacement,
}

impl AcceptanceTargetResolution {
    pub fn code(self) -> &'static str {
        match self {
            Self::ExactBase => "exact_base",
            Self::BlockReplacement => "block_replacement",
            Self::ProvenRange => "proven_range",
            Self::BaseBlockReplacement => "base_block_replacement",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        [
            Self::ExactBase,
            Self::BlockReplacement,
            Self::ProvenRange,
            Self::BaseBlockReplacement,
        ]
        .into_iter()
        .find(|resolution| resolution.code() == code)
    }
}

/// The Core Transition Outcome of one Acceptance. An invalid Acceptance has no effect, and its
/// reason records the Receipt result kind `invalid`.
pub type AcceptProposalOutcome = TransitionOutcome<
    AcceptanceTargetResolution,
    AcceptProposalInvalid,
    AcceptProposalConflict,
    AcceptProposalRefusal,
>;

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
    MissingTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptProposalInvalid {
    InvalidValidation,
    AlteredCandidate,
}

/// Only an Acceptance Receipt from before ADR 0064 records this conflict.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptProposalConflict {
    ChangedHead,
}

reason_codes!(AcceptProposalRefusal {
    WrongScope => "wrong_scope",
    WrongAdmission => "wrong_admission",
    StaleProposalRevision => "stale_proposal_revision",
    NotEligible => "not_eligible",
    OperationNotPending => "operation_not_pending",
    DuplicateIdentities => "duplicate_identities",
    MissingRequiredDependencies => "missing_required_dependencies",
    IncompleteBundleClosure => "incomplete_bundle_closure",
    MissingTarget => "missing_target",
});
reason_codes!(AcceptProposalInvalid, result "invalid" {
    InvalidValidation => "invalid_validation",
    AlteredCandidate => "altered_candidate",
});
reason_codes!(AcceptProposalConflict { ChangedHead => "changed_head" });

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

/// Classify one Operation Acceptance against Scope, Admission, eligibility, and its target on the
/// current Chapter Head (ADR 0064).
pub fn accept_proposal(command: &AcceptProposal) -> AcceptProposalOutcome {
    let acceptable = |validation: AcceptanceValidation| match validation {
        AcceptanceValidation::Valid | AcceptanceValidation::Conflicted => true,
        AcceptanceValidation::Other => false,
    };
    let condition_allows = match command.condition {
        AcceptanceCondition::Absent | AcceptanceCondition::ProposalConflict => true,
        AcceptanceCondition::RecoveryConflict | AcceptanceCondition::Invalid => false,
    };
    if !command.scope_matches {
        return TransitionOutcome::Refused(AcceptProposalRefusal::WrongScope);
    }
    if !command.admission_valid {
        return TransitionOutcome::Refused(AcceptProposalRefusal::WrongAdmission);
    }
    if !command.proposal_revision_current {
        return TransitionOutcome::Refused(AcceptProposalRefusal::StaleProposalRevision);
    }
    if !command.retention_retained
        || !command.generation_ready
        || !command.closure_open
        || !acceptable(command.validation)
        || !condition_allows
    {
        return TransitionOutcome::Refused(AcceptProposalRefusal::NotEligible);
    }
    if !acceptable(command.validation_receipt) || !command.validation_receipt_matches_revision {
        return TransitionOutcome::NoEffect(AcceptProposalInvalid::InvalidValidation);
    }
    if !command.selection_duplicate_free {
        return TransitionOutcome::Refused(AcceptProposalRefusal::DuplicateIdentities);
    }
    if !command.selected_operation_pending {
        return TransitionOutcome::Refused(AcceptProposalRefusal::OperationNotPending);
    }
    if !command.required_dependencies_met {
        return TransitionOutcome::Refused(AcceptProposalRefusal::MissingRequiredDependencies);
    }
    if !command.bundle_closure_complete {
        return TransitionOutcome::Refused(AcceptProposalRefusal::IncompleteBundleClosure);
    }
    let resolution = match (command.target_kind, command.base) {
        (_, AcceptanceBase::ExactHead) => AcceptanceTargetResolution::ExactBase,
        (
            _,
            AcceptanceBase::EarlierHead {
                target_blocks_present: false,
                ..
            },
        ) => return TransitionOutcome::Refused(AcceptProposalRefusal::MissingTarget),
        (AcceptanceTargetKind::Block, AcceptanceBase::EarlierHead { .. }) => {
            AcceptanceTargetResolution::BlockReplacement
        }
        (
            AcceptanceTargetKind::Inline,
            AcceptanceBase::EarlierHead {
                inline_range_proven: true,
                ..
            },
        ) => AcceptanceTargetResolution::ProvenRange,
        (
            AcceptanceTargetKind::Inline,
            AcceptanceBase::EarlierHead {
                inline_range_proven: false,
                ..
            },
        ) => AcceptanceTargetResolution::BaseBlockReplacement,
    };
    if !command.candidate_unaltered {
        return TransitionOutcome::NoEffect(AcceptProposalInvalid::AlteredCandidate);
    }
    TransitionOutcome::Applied(resolution)
}

#[cfg(test)]
#[path = "accept_proposal_tests.rs"]
mod tests;
