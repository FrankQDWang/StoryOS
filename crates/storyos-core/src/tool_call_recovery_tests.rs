use super::*;
use crate::WEB_SEARCH_TOOL_SPEC;

#[test]
fn each_class_comes_from_the_state_the_attempt_and_the_late_settlement() {
    use ToolAttemptSettlement::{NativeError, NotSubmitted, OutcomeUnknown, TerminalResult};
    use ToolCallState::{Dispatched, Failed, Pending, Refused, Settled, Unknown};
    let cases = [
        (Pending, None, None),
        (Dispatched, Some(OutcomeUnknown), None),
        (Refused, None, None),
        (Failed, Some(NotSubmitted), None),
        (Settled, Some(TerminalResult), None),
        (Failed, Some(NativeError), None),
        (Unknown, Some(OutcomeUnknown), Some(TerminalResult)),
        (Unknown, Some(OutcomeUnknown), Some(NativeError)),
        (Unknown, Some(OutcomeUnknown), None),
        (Unknown, Some(OutcomeUnknown), Some(OutcomeUnknown)),
        (Unknown, Some(OutcomeUnknown), Some(NotSubmitted)),
    ];

    assert_eq!(
        cases.map(
            |(state, attempt, late)| tool_call_class(state, attempt, late)
                .map(ToolCallClass::as_str)
        ),
        [
            None,
            None,
            Some("not_attempted"),
            Some("not_attempted"),
            Some("confirmed"),
            Some("confirmed"),
            Some("partial"),
            Some("partial"),
            Some("unknown"),
            Some("unknown"),
            Some("unknown"),
        ]
    );
}

#[test]
fn a_late_observation_confirms_only_what_it_proves_and_keeps_the_rest_unknown() {
    let late = |observation| late_tool_evidence(&WEB_SEARCH_TOOL_SPEC, &observation);
    let outcome = |external_read, outbound_disclosure| ToolEffectOutcome {
        external_read,
        outbound_disclosure,
    };
    let result = late(ToolObservation::Called {
        text: vec!["High water at 10.42.".to_owned()],
        is_error: false,
    });

    assert_eq!(
        (
            (
                result.attempt,
                result.outcome,
                result.result.map(|kept| kept.text)
            ),
            late(ToolObservation::NotSubmitted),
            late(ToolObservation::AnsweredWithoutResult {
                reason: "http_502".to_owned()
            }),
        ),
        (
            (
                ToolAttemptSettlement::TerminalResult,
                outcome(EffectOutcome::Confirmed, EffectOutcome::Confirmed),
                Some("High water at 10.42.".to_owned()),
            ),
            LateToolEvidence {
                attempt: ToolAttemptSettlement::NotSubmitted,
                outcome: outcome(EffectOutcome::Unknown, EffectOutcome::Unknown),
                result: None,
            },
            LateToolEvidence {
                attempt: ToolAttemptSettlement::OutcomeUnknown,
                outcome: outcome(EffectOutcome::Unknown, EffectOutcome::Confirmed),
                result: None,
            },
        )
    );
}
