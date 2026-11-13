use super::{
    FinishRuleOutcome, PriorStepCall, RunStepLimit, ToolChoice, apply_finish_rule, step_tool_choice,
};
use crate::{
    AgentDecisionKind, AgentDecisionOutcome, NativeStreamItem, StreamItemRole, StreamItemState,
    ToolCallState, ToolOperation,
};

fn call(operation: ToolOperation, state: ToolCallState) -> PriorStepCall {
    PriorStepCall {
        operation: Some(operation),
        state,
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
            step_tool_choice(
                /*declares_host_tools*/ true,
                RunStepLimit::Open,
                &prior_step
            ),
            expected,
            "{prior_step:?}"
        );
    }
    assert_eq!(
        step_tool_choice(
            /*declares_host_tools*/ false,
            RunStepLimit::Last,
            &[finish]
        ),
        ToolChoice::Auto
    );
}

#[test]
fn only_a_complete_closing_response_gives_its_text_as_the_final_answer() {
    let text = NativeStreamItem {
        item_id: "msg_1".to_owned(),
        role: StreamItemRole::Assistant,
        state: StreamItemState::Complete,
        text: Some("The answer.".to_owned()),
        summary: None,
        call_id: None,
        arguments: None,
        refusal: None,
        native_type: None,
        name: None,
    };
    let closing = |state| {
        apply_finish_rule(
            ToolChoice::None,
            RunStepLimit::Open,
            AgentDecisionOutcome::NoDecision,
            state,
            std::slice::from_ref(&text),
        )
    };
    assert_eq!(
        closing(StreamItemState::Complete),
        FinishRuleOutcome::Settle(AgentDecisionOutcome::Decision {
            kind: AgentDecisionKind::Advisory {
                text: "The answer.".to_owned()
            },
            selected: true,
            advances_continuation: true,
        })
    );
    assert_eq!(
        closing(StreamItemState::Incomplete),
        FinishRuleOutcome::Settle(AgentDecisionOutcome::NoDecision)
    );
}

#[test]
fn each_run_step_at_or_after_the_limit_is_the_last_and_sends_none() {
    let finish = call(ToolOperation::Finish, ToolCallState::Settled);
    assert_eq!(
        (
            [22, 23, 24].map(RunStepLimit::at),
            [
                (true, RunStepLimit::Last),
                (false, RunStepLimit::Last),
                (true, RunStepLimit::Open)
            ]
            .map(|(host_tools, limit)| limit.executes_calls(host_tools)),
            step_tool_choice(/*declares_host_tools*/ true, RunStepLimit::Last, &[]),
            step_tool_choice(
                /*declares_host_tools*/ true,
                RunStepLimit::Last,
                &[finish]
            ),
        ),
        (
            [RunStepLimit::Open, RunStepLimit::Last, RunStepLimit::Last],
            [false, true, true],
            ToolChoice::None,
            ToolChoice::None,
        )
    );
}
