use super::{FinishRuleOutcome, PriorStepCall, ToolChoice, apply_finish_rule, step_tool_choice};
use crate::{
    AgentDecisionKind, AgentDecisionOutcome, NativeStreamItem, StreamItemRole, StreamItemState,
    ToolCallState, ToolOperation,
};

fn call(operation: ToolOperation, state: ToolCallState) -> PriorStepCall {
    PriorStepCall { operation, state }
}

fn answer(text: &str) -> AgentDecisionOutcome {
    AgentDecisionOutcome::Decision {
        kind: AgentDecisionKind::Advisory {
            text: text.to_owned(),
        },
        selected: true,
        advances_continuation: true,
    }
}

fn function_call_item() -> NativeStreamItem {
    NativeStreamItem {
        item_id: "fc_finish".to_owned(),
        role: StreamItemRole::Tool,
        state: StreamItemState::Complete,
        text: None,
        summary: None,
        call_id: Some("call_finish".to_owned()),
        arguments: Some("{}".to_owned()),
        refusal: None,
        native_type: Some(crate::FUNCTION_CALL_NATIVE_TYPE.to_owned()),
        name: Some("finish".to_owned()),
    }
}

#[test]
fn only_a_settled_finish_call_with_no_refused_call_beside_it_closes_the_run() {
    let finish = call(ToolOperation::Finish, ToolCallState::Settled);
    let search = |state| call(ToolOperation::WebSearch, state);
    let cases = [
        (vec![], ToolChoice::Required),
        (vec![finish], ToolChoice::None),
        (
            vec![search(ToolCallState::Settled), finish],
            ToolChoice::None,
        ),
        (
            vec![search(ToolCallState::Failed), finish],
            ToolChoice::None,
        ),
        (
            vec![search(ToolCallState::Refused), finish],
            ToolChoice::Required,
        ),
        (vec![search(ToolCallState::Settled)], ToolChoice::Required),
    ];
    for (prior_step, expected) in cases {
        assert_eq!(
            step_tool_choice(/*declares_host_tools*/ true, &prior_step),
            expected,
            "{prior_step:?}"
        );
    }
    assert_eq!(
        step_tool_choice(/*declares_host_tools*/ false, &[finish]),
        ToolChoice::Auto
    );
}

#[test]
fn a_text_before_a_finish_call_opens_the_next_run_step() {
    assert_eq!(
        apply_finish_rule(ToolChoice::Required, answer("Done."), &[]),
        FinishRuleOutcome::NextRunStep
    );
    assert_eq!(
        apply_finish_rule(ToolChoice::Required, AgentDecisionOutcome::NoDecision, &[]),
        FinishRuleOutcome::NextRunStep
    );
    // A function request that did not validate keeps its refusal.
    assert_eq!(
        apply_finish_rule(
            ToolChoice::Required,
            AgentDecisionOutcome::NoDecision,
            &[function_call_item()]
        ),
        FinishRuleOutcome::Settle(AgentDecisionOutcome::NoDecision)
    );
}

#[test]
fn the_closing_text_is_the_final_answer_and_an_empty_text_gives_no_result() {
    assert_eq!(
        apply_finish_rule(ToolChoice::None, answer("The tide turns at six."), &[]),
        FinishRuleOutcome::Settle(answer("The tide turns at six."))
    );
    for empty in ["", " \n"] {
        assert_eq!(
            apply_finish_rule(ToolChoice::None, answer(empty), &[]),
            FinishRuleOutcome::Settle(AgentDecisionOutcome::NoDecision)
        );
    }
    // The reply form profile keeps each outcome.
    assert_eq!(
        apply_finish_rule(ToolChoice::Auto, answer(""), &[]),
        FinishRuleOutcome::Settle(answer(""))
    );
}
