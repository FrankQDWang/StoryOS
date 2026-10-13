//! Decide whether one unknown create may fence its predecessor and dispatch one successor.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownCreateScript {
    NotSubject,
    Once,
    MissingReference,
    UnsupportedRetrieval,
    Budget,
    UnresolvedEffect,
    AbsentEffect,
    ContextChanged,
    RequestChanged,
    RouteChanged,
    Authority,
    Late,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuccessorLookup {
    StillUnknown,
    Unavailable { reason: LookupUnavailable },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LookupUnavailable {
    MissingReference,
    UnsupportedRetrieval,
}

impl LookupUnavailable {
    /// Stable inspect label for one unavailable original-result lookup.
    pub const fn label(self) -> &'static str {
        match self {
            Self::MissingReference => "missing_reference",
            Self::UnsupportedRetrieval => "unsupported_retrieval",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuccessorEffect {
    None,
    Unresolved,
    AbsentUnsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuccessorAllowance {
    Available,
    ConsumedPendingDispatch,
    Dispatched,
}

/// The Host gates of one successor when the unknown create is recorded. The fake profile
/// scripts them, and a real route derives them from its records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SuccessorConditions {
    pub same_request: bool,
    pub same_route: bool,
    pub current_authority: bool,
    pub budget_covers_both: bool,
    pub effect: SuccessorEffect,
    pub context_changed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnknownCreateSuccessorFacts {
    pub lookup: SuccessorLookup,
    pub same_request: bool,
    pub same_route: bool,
    pub current_authority: bool,
    pub budget_covers_both: bool,
    pub effect: SuccessorEffect,
    pub cancelled: bool,
    pub allowance: SuccessorAllowance,
    pub context_changed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuccessorPauseReason {
    RequestChanged,
    RouteChanged,
    AuthorityUnavailable,
    BudgetInsufficient,
    UnresolvedEffect,
    UnsupportedAbsentEffect,
    ContextChanged,
}

impl SuccessorPauseReason {
    /// Stable inspect label for one paused unknown-create recovery.
    pub const fn label(self) -> &'static str {
        match self {
            Self::RequestChanged => "request_changed",
            Self::RouteChanged => "route_changed",
            Self::AuthorityUnavailable => "authority_unavailable",
            Self::BudgetInsufficient => "budget_insufficient",
            Self::UnresolvedEffect => "unresolved_effect",
            Self::UnsupportedAbsentEffect => "unsupported_absent_effect",
            Self::ContextChanged => "changed_effective_model_context",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownCreateSuccessorDecision {
    FenceAndDispatch,
    ResumePendingDispatch,
    AlreadyDispatched,
    Pause { reason: SuccessorPauseReason },
    ProhibitedByCancellation,
}

/// Classify one author message as an unknown-create successor subject.
pub fn unknown_create_script(author_message: &str) -> UnknownCreateScript {
    match author_message {
        "SCRIPT:successor-once" => UnknownCreateScript::Once,
        "SCRIPT:successor-missing" => UnknownCreateScript::MissingReference,
        "SCRIPT:successor-unsupported" => UnknownCreateScript::UnsupportedRetrieval,
        "SCRIPT:successor-budget" => UnknownCreateScript::Budget,
        "SCRIPT:successor-effect" => UnknownCreateScript::UnresolvedEffect,
        "SCRIPT:successor-absent-effect" => UnknownCreateScript::AbsentEffect,
        "SCRIPT:successor-context" => UnknownCreateScript::ContextChanged,
        "SCRIPT:successor-request" => UnknownCreateScript::RequestChanged,
        "SCRIPT:successor-route" => UnknownCreateScript::RouteChanged,
        "SCRIPT:successor-authority" => UnknownCreateScript::Authority,
        "SCRIPT:successor-late" => UnknownCreateScript::Late,
        _ => UnknownCreateScript::NotSubject,
    }
}

/// Map one successor script to its scripted Host gates.
pub fn scripted_successor_conditions(script: UnknownCreateScript) -> Option<SuccessorConditions> {
    let permitted = SuccessorConditions {
        same_request: true,
        same_route: true,
        current_authority: true,
        budget_covers_both: true,
        effect: SuccessorEffect::None,
        context_changed: false,
    };
    Some(match script {
        UnknownCreateScript::NotSubject => return None,
        UnknownCreateScript::Once
        | UnknownCreateScript::Late
        | UnknownCreateScript::MissingReference
        | UnknownCreateScript::UnsupportedRetrieval => permitted,
        UnknownCreateScript::Budget => SuccessorConditions {
            budget_covers_both: false,
            ..permitted
        },
        UnknownCreateScript::UnresolvedEffect => SuccessorConditions {
            effect: SuccessorEffect::Unresolved,
            ..permitted
        },
        UnknownCreateScript::AbsentEffect => SuccessorConditions {
            effect: SuccessorEffect::AbsentUnsupported,
            ..permitted
        },
        UnknownCreateScript::ContextChanged => SuccessorConditions {
            context_changed: true,
            ..permitted
        },
        UnknownCreateScript::RequestChanged => SuccessorConditions {
            same_request: false,
            ..permitted
        },
        UnknownCreateScript::RouteChanged => SuccessorConditions {
            same_route: false,
            ..permitted
        },
        UnknownCreateScript::Authority => SuccessorConditions {
            current_authority: false,
            ..permitted
        },
    })
}

/// The gates of the one successor on a real route. The successor sends the committed request
/// again, so the request and its Context stay the same, and the dispatch claim compares the wire
/// digest. The budget for both requests is a known request bound of the route (author decision,
/// 2026-10-13). Each decision checks the route and the authority again.
pub fn route_successor_conditions(
    bounds: crate::BudgetBounds,
    effect: SuccessorEffect,
) -> SuccessorConditions {
    SuccessorConditions {
        same_request: true,
        same_route: true,
        current_authority: true,
        budget_covers_both: bounds.request_bounds().is_some(),
        effect,
        context_changed: false,
    }
}

/// Fence at most one successor, or pause. Lookup availability does not grant or ban it.
pub fn decide_unknown_create_successor(
    facts: &UnknownCreateSuccessorFacts,
) -> UnknownCreateSuccessorDecision {
    match facts.lookup {
        SuccessorLookup::StillUnknown | SuccessorLookup::Unavailable { .. } => {}
    }
    if matches!(facts.allowance, SuccessorAllowance::Dispatched) {
        return UnknownCreateSuccessorDecision::AlreadyDispatched;
    }
    if facts.cancelled {
        return UnknownCreateSuccessorDecision::ProhibitedByCancellation;
    }
    if matches!(facts.allowance, SuccessorAllowance::ConsumedPendingDispatch) {
        return if facts.current_authority {
            UnknownCreateSuccessorDecision::ResumePendingDispatch
        } else {
            UnknownCreateSuccessorDecision::Pause {
                reason: SuccessorPauseReason::AuthorityUnavailable,
            }
        };
    }
    let reason = if !facts.current_authority {
        SuccessorPauseReason::AuthorityUnavailable
    } else if !facts.same_request {
        SuccessorPauseReason::RequestChanged
    } else if !facts.same_route {
        SuccessorPauseReason::RouteChanged
    } else if !facts.budget_covers_both {
        SuccessorPauseReason::BudgetInsufficient
    } else if matches!(facts.effect, SuccessorEffect::Unresolved) {
        SuccessorPauseReason::UnresolvedEffect
    } else if matches!(facts.effect, SuccessorEffect::AbsentUnsupported) {
        SuccessorPauseReason::UnsupportedAbsentEffect
    } else if facts.context_changed {
        SuccessorPauseReason::ContextChanged
    } else {
        return UnknownCreateSuccessorDecision::FenceAndDispatch;
    };
    UnknownCreateSuccessorDecision::Pause { reason }
}

#[cfg(test)]
#[path = "unknown_create_successor_tests.rs"]
mod tests;
