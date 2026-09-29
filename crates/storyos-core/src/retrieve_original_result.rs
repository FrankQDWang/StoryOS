//! Decide whether an uncertain create may be reconciled by its original result.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OriginalResultScript {
    NotSubject,
    MissingReference,
    Unsupported,
    UnknownBounds,
    ForeignScope,
    ForeignConversation,
    ForeignMapping,
    ForeignDestination,
    Incomplete,
    UnknownResult,
    CompleteSelected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetrievalCapability {
    Supported,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetrievalBounds {
    DeclaredReadOnly,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainedResponseReference {
    Absent,
    Present,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetrievedOriginalResult {
    NotRetrieved,
    Unknown,
    Incomplete,
    CompleteSelected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OriginalResultRetrievalFacts {
    pub capability: RetrievalCapability,
    pub bounds: RetrievalBounds,
    pub reference: RetainedResponseReference,
    pub scope_permitted: bool,
    pub conversation_permitted: bool,
    pub destination_permitted: bool,
    pub mapping_permitted: bool,
    pub retrieved: RetrievedOriginalResult,
    pub run_fenced: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OriginalResultKeepReason {
    MissingReference,
    UnsupportedRetrieval,
    UnknownBounds,
    ScopeMismatch,
    ConversationMismatch,
    DestinationMismatch,
    MappingMismatch,
    UnknownResult,
    IncompleteResult,
}

impl OriginalResultKeepReason {
    /// Stable inspect label for one kept-unknown retrieval.
    pub const fn label(self) -> &'static str {
        match self {
            Self::MissingReference => "missing_reference",
            Self::UnsupportedRetrieval => "unsupported_retrieval",
            Self::UnknownBounds => "unknown_bounds",
            Self::ScopeMismatch => "scope_mismatch",
            Self::ConversationMismatch => "conversation_mismatch",
            Self::DestinationMismatch => "destination_mismatch",
            Self::MappingMismatch => "mapping_mismatch",
            Self::UnknownResult => "unknown_result",
            Self::IncompleteResult => "incomplete_result",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OriginalResultRetrievalDecision {
    KeepUnknown {
        reason: OriginalResultKeepReason,
        admit_retrieval: bool,
    },
    EvidenceOnly,
    SettleSelected,
}

/// Classify one author message as an original-result retrieval subject.
pub fn original_result_script(author_message: &str) -> OriginalResultScript {
    match author_message {
        "SCRIPT:retrieve-missing" => OriginalResultScript::MissingReference,
        "SCRIPT:retrieve-unsupported" => OriginalResultScript::Unsupported,
        "SCRIPT:retrieve-unbounded" => OriginalResultScript::UnknownBounds,
        "SCRIPT:retrieve-foreign-scope" => OriginalResultScript::ForeignScope,
        "SCRIPT:retrieve-foreign-conversation" => OriginalResultScript::ForeignConversation,
        "SCRIPT:retrieve-foreign-mapping" => OriginalResultScript::ForeignMapping,
        "SCRIPT:retrieve-foreign-destination" => OriginalResultScript::ForeignDestination,
        "SCRIPT:retrieve-incomplete" => OriginalResultScript::Incomplete,
        "SCRIPT:retrieve-unknown" => OriginalResultScript::UnknownResult,
        "SCRIPT:retrieve-complete" => OriginalResultScript::CompleteSelected,
        _ => OriginalResultScript::NotSubject,
    }
}

/// Admit original-result retrieval, or keep the unknown disposition and reservation.
pub fn decide_original_result_retrieval(
    facts: &OriginalResultRetrievalFacts,
) -> OriginalResultRetrievalDecision {
    let blocked = |reason| OriginalResultRetrievalDecision::KeepUnknown {
        reason,
        admit_retrieval: false,
    };
    if matches!(facts.reference, RetainedResponseReference::Absent) {
        return blocked(OriginalResultKeepReason::MissingReference);
    }
    if matches!(facts.capability, RetrievalCapability::Unsupported) {
        return blocked(OriginalResultKeepReason::UnsupportedRetrieval);
    }
    if matches!(facts.bounds, RetrievalBounds::Unknown) {
        return blocked(OriginalResultKeepReason::UnknownBounds);
    }
    if !facts.scope_permitted {
        return blocked(OriginalResultKeepReason::ScopeMismatch);
    }
    if !facts.conversation_permitted {
        return blocked(OriginalResultKeepReason::ConversationMismatch);
    }
    if !facts.destination_permitted {
        return blocked(OriginalResultKeepReason::DestinationMismatch);
    }
    if !facts.mapping_permitted {
        return blocked(OriginalResultKeepReason::MappingMismatch);
    }
    match facts.retrieved {
        RetrievedOriginalResult::NotRetrieved | RetrievedOriginalResult::Unknown => {
            OriginalResultRetrievalDecision::KeepUnknown {
                reason: OriginalResultKeepReason::UnknownResult,
                admit_retrieval: true,
            }
        }
        RetrievedOriginalResult::Incomplete => OriginalResultRetrievalDecision::KeepUnknown {
            reason: OriginalResultKeepReason::IncompleteResult,
            admit_retrieval: true,
        },
        RetrievedOriginalResult::CompleteSelected if facts.run_fenced => {
            OriginalResultRetrievalDecision::EvidenceOnly
        }
        RetrievedOriginalResult::CompleteSelected => {
            OriginalResultRetrievalDecision::SettleSelected
        }
    }
}

#[cfg(test)]
#[path = "retrieve_original_result_tests.rs"]
mod tests;
