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

/// Scripted Host facts of the fake profile. The fake adapter reports the reference facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScriptedSuccessorConditions {
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
pub fn scripted_successor_conditions(
    script: UnknownCreateScript,
) -> Option<ScriptedSuccessorConditions> {
    let permitted = ScriptedSuccessorConditions {
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
        UnknownCreateScript::Budget => ScriptedSuccessorConditions {
            budget_covers_both: false,
            ..permitted
        },
        UnknownCreateScript::UnresolvedEffect => ScriptedSuccessorConditions {
            effect: SuccessorEffect::Unresolved,
            ..permitted
        },
        UnknownCreateScript::AbsentEffect => ScriptedSuccessorConditions {
            effect: SuccessorEffect::AbsentUnsupported,
            ..permitted
        },
        UnknownCreateScript::ContextChanged => ScriptedSuccessorConditions {
            context_changed: true,
            ..permitted
        },
        UnknownCreateScript::RequestChanged => ScriptedSuccessorConditions {
            same_request: false,
            ..permitted
        },
        UnknownCreateScript::RouteChanged => ScriptedSuccessorConditions {
            same_route: false,
            ..permitted
        },
        UnknownCreateScript::Authority => ScriptedSuccessorConditions {
            current_authority: false,
            ..permitted
        },
    })
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
        return UnknownCreateSuccessorDecision::ResumePendingDispatch;
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
