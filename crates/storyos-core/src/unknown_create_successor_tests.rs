use super::{
    LookupUnavailable, SuccessorAllowance, SuccessorLookup, SuccessorPauseReason,
    UnknownCreateScript, UnknownCreateSuccessorDecision, UnknownCreateSuccessorFacts,
    decide_unknown_create_successor, scripted_successor_conditions, unknown_create_script,
};

fn facts(script: UnknownCreateScript) -> UnknownCreateSuccessorFacts {
    let conditions = scripted_successor_conditions(script).expect("successor script");
    UnknownCreateSuccessorFacts {
        lookup: conditions.lookup,
        same_request: conditions.same_request,
        same_route: conditions.same_route,
        current_authority: conditions.current_authority,
        budget_covers_both: conditions.budget_covers_both,
        effect: conditions.effect,
        cancelled: false,
        allowance: SuccessorAllowance::Available,
        context_changed: conditions.context_changed,
    }
}

#[test]
fn successor_scripts_are_distinct_from_ordinary_and_retrieval_text() {
    assert_eq!(
        unknown_create_script("Help with this passage."),
        UnknownCreateScript::NotSubject
    );
    assert_eq!(
        unknown_create_script("SCRIPT:retrieve-unknown"),
        UnknownCreateScript::NotSubject
    );
    assert!(scripted_successor_conditions(UnknownCreateScript::NotSubject).is_none());
    assert_eq!(
        unknown_create_script("SCRIPT:successor-once"),
        UnknownCreateScript::Once
    );
    assert!(
        scripted_successor_conditions(UnknownCreateScript::Late)
            .expect("late")
            .late_complete
    );
}

#[test]
fn missing_or_unsupported_lookup_still_permits_one_successor() {
    for script in [
        UnknownCreateScript::Once,
        UnknownCreateScript::Late,
        UnknownCreateScript::MissingReference,
        UnknownCreateScript::UnsupportedRetrieval,
    ] {
        assert_eq!(
            decide_unknown_create_successor(&facts(script)),
            UnknownCreateSuccessorDecision::FenceAndDispatch,
            "{script:?}"
        );
    }
    assert_eq!(
        facts(UnknownCreateScript::MissingReference).lookup,
        SuccessorLookup::Unavailable {
            reason: LookupUnavailable::MissingReference,
        }
    );
}

#[test]
fn failed_gates_pause_without_consuming_the_allowance() {
    let cases = [
        (
            UnknownCreateScript::Authority,
            SuccessorPauseReason::AuthorityUnavailable,
        ),
        (
            UnknownCreateScript::RequestChanged,
            SuccessorPauseReason::RequestChanged,
        ),
        (
            UnknownCreateScript::RouteChanged,
            SuccessorPauseReason::RouteChanged,
        ),
        (
            UnknownCreateScript::Budget,
            SuccessorPauseReason::BudgetInsufficient,
        ),
        (
            UnknownCreateScript::UnresolvedEffect,
            SuccessorPauseReason::UnresolvedEffect,
        ),
        (
            UnknownCreateScript::AbsentEffect,
            SuccessorPauseReason::UnsupportedAbsentEffect,
        ),
        (
            UnknownCreateScript::ContextChanged,
            SuccessorPauseReason::ContextChanged,
        ),
    ];
    for (script, reason) in cases {
        assert_eq!(
            decide_unknown_create_successor(&facts(script)),
            UnknownCreateSuccessorDecision::Pause { reason },
            "{script:?}"
        );
    }
}

#[test]
fn cancellation_and_spent_allowance_do_not_reset_or_duplicate() {
    let mut pending = facts(UnknownCreateScript::Once);
    pending.allowance = SuccessorAllowance::ConsumedPendingDispatch;
    pending.budget_covers_both = false;
    assert_eq!(
        decide_unknown_create_successor(&pending),
        UnknownCreateSuccessorDecision::ResumePendingDispatch
    );
    pending.cancelled = true;
    assert_eq!(
        decide_unknown_create_successor(&pending),
        UnknownCreateSuccessorDecision::ProhibitedByCancellation
    );
    let mut spent = facts(UnknownCreateScript::MissingReference);
    spent.allowance = SuccessorAllowance::Dispatched;
    spent.cancelled = true;
    spent.context_changed = true;
    assert_eq!(
        decide_unknown_create_successor(&spent),
        UnknownCreateSuccessorDecision::AlreadyDispatched
    );
    let mut open = facts(UnknownCreateScript::Once);
    open.cancelled = true;
    assert_eq!(
        decide_unknown_create_successor(&open),
        UnknownCreateSuccessorDecision::ProhibitedByCancellation
    );
}
