use super::{
    ADVISORY_TEXT, CLARIFICATION_QUESTION, ExecutionCapability, FakeAttemptOutcome,
    FakeDecisionKind, FakeDispatchPlan, INLINE_PROSE_CHANGE_TEXT, NativeStreamItem,
    NoDecisionReason, PROSE_CHANGE_TEXT, StreamItemRole, StreamItemState, host_fake_wire_digest,
    plan_fake_model_decision,
};

fn assistant(state: StreamItemState) -> NativeStreamItem {
    NativeStreamItem {
        item_id: "1",
        role: StreamItemRole::Assistant,
        state,
        text: Some(ADVISORY_TEXT),
        summary: Some("host_fake_native_text"),
        call_id: None,
        arguments: None,
        refusal: None,
        hosted_report: None,
    }
}

#[test]
fn plans_one_selected_advisory_decision_for_ordinary_help() {
    assert_eq!(
        plan_fake_model_decision("Help with this passage."),
        FakeDispatchPlan::Dispatch {
            items: vec![assistant(StreamItemState::Complete)],
            outcome: FakeAttemptOutcome::Decision {
                kind: FakeDecisionKind::Advisory {
                    text: ADVISORY_TEXT
                },
                selected: true,
                advances_continuation: true,
            },
        }
    );
}

#[test]
fn plans_prose_change_input_without_treating_it_as_authority() {
    assert_eq!(
        plan_fake_model_decision("Revise this passage: make it quieter."),
        FakeDispatchPlan::Dispatch {
            items: vec![NativeStreamItem {
                text: Some(PROSE_CHANGE_TEXT),
                ..assistant(StreamItemState::Complete)
            }],
            outcome: FakeAttemptOutcome::Decision {
                kind: FakeDecisionKind::ProseChange {
                    text: PROSE_CHANGE_TEXT,
                    producer_input: PROSE_CHANGE_TEXT,
                },
                selected: true,
                advances_continuation: true,
            },
        }
    );
}

#[test]
fn plans_inline_phrase_change_without_treating_it_as_authority() {
    assert_eq!(
        plan_fake_model_decision("Revise this phrase: keep the voice."),
        FakeDispatchPlan::Dispatch {
            items: vec![NativeStreamItem {
                text: Some(INLINE_PROSE_CHANGE_TEXT),
                ..assistant(StreamItemState::Complete)
            }],
            outcome: FakeAttemptOutcome::Decision {
                kind: FakeDecisionKind::ProseChange {
                    text: INLINE_PROSE_CHANGE_TEXT,
                    producer_input: INLINE_PROSE_CHANGE_TEXT,
                },
                selected: true,
                advances_continuation: true,
            },
        }
    );
}

#[test]
fn plans_material_clarification_without_continuation() {
    assert_eq!(
        plan_fake_model_decision("Which wording should I keep?"),
        FakeDispatchPlan::Dispatch {
            items: vec![NativeStreamItem {
                text: Some(CLARIFICATION_QUESTION),
                ..assistant(StreamItemState::Complete)
            }],
            outcome: FakeAttemptOutcome::Decision {
                kind: FakeDecisionKind::Clarification {
                    question: CLARIFICATION_QUESTION,
                },
                selected: true,
                advances_continuation: false,
            },
        }
    );
}

#[test]
fn refuses_tool_and_memory_requests_before_dispatch() {
    assert_eq!(
        plan_fake_model_decision("Please invoke a tool on this passage."),
        FakeDispatchPlan::RefuseWithoutDispatch {
            capability: ExecutionCapability::Tool,
        }
    );
    assert_eq!(
        plan_fake_model_decision("Extract conversation memory now."),
        FakeDispatchPlan::RefuseWithoutDispatch {
            capability: ExecutionCapability::Memory,
        }
    );
}

#[test]
fn keeps_partial_and_unselected_output_as_evidence_only() {
    assert_eq!(
        plan_fake_model_decision("SCRIPT:partial"),
        FakeDispatchPlan::Dispatch {
            items: vec![assistant(StreamItemState::Provisional)],
            outcome: FakeAttemptOutcome::NoDecision {
                reason: NoDecisionReason::Partial,
            },
        }
    );
    assert_eq!(
        plan_fake_model_decision("SCRIPT:unselected"),
        FakeDispatchPlan::Dispatch {
            items: vec![assistant(StreamItemState::Complete)],
            outcome: FakeAttemptOutcome::Decision {
                kind: FakeDecisionKind::Advisory {
                    text: ADVISORY_TEXT
                },
                selected: false,
                advances_continuation: false,
            },
        }
    );
}

#[test]
fn digests_non_secret_wire_material_with_the_host_fake_mapping() {
    assert_eq!(
        host_fake_wire_digest("Help with this passage.", "chapter-1"),
        "sha256:1d5ffe1781cb8b2d488fa67fa2e1bd8cab14bd83c4052f211dac313d7d996290"
    );
}
