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
            step_tool_choice(RunStepLimit::Open, &prior_step),
            expected,
            "{prior_step:?}"
        );
    }
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
            [RunStepLimit::Last, RunStepLimit::Open].map(RunStepLimit::executes_calls),
            step_tool_choice(RunStepLimit::Last, &[]),
            step_tool_choice(RunStepLimit::Last, &[finish]),
        ),
        (
            [RunStepLimit::Open, RunStepLimit::Last, RunStepLimit::Last],
            [false, true],
            ToolChoice::None,
            ToolChoice::None,
        )
    );
}

/// One complete assistant text and one function call of `state`.
fn text_and_call(state: StreamItemState) -> [NativeStreamItem; 2] {
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
    let call = NativeStreamItem {
        item_id: "fc_1".to_owned(),
        role: StreamItemRole::Tool,
        state,
        text: None,
        call_id: Some("call_1".to_owned()),
        arguments: Some("{}".to_owned()),
        native_type: Some(crate::FUNCTION_CALL_NATIVE_TYPE.to_owned()),
        name: Some("look_up_notes".to_owned()),
        ..text.clone()
    };
    [text, call]
}

#[test]
fn a_response_with_a_call_that_no_output_can_answer_does_not_advance_the_continuation() {
    let [text, call] = text_and_call(StreamItemState::Complete);
    let [_, cut_call] = text_and_call(StreamItemState::Incomplete);
    // A valid batch: each of its calls has a call identifier that an output can name.
    let batch = AgentDecisionOutcome::Decision {
        kind: AgentDecisionKind::ToolRequest {
            calls: vec![crate::RequestedToolCall::Refused(crate::RefusedToolCall {
                call_id: "call_1".to_owned(),
                name: "look_up_notes".to_owned(),
                tool: None,
                arguments_text: "{}".to_owned(),
                refusal: crate::CallValidationRefusal::Unexposed,
            })],
        },
        selected: true,
        advances_continuation: false,
    };
    let answer = |advances_continuation| {
        FinishRuleOutcome::Settle(AgentDecisionOutcome::Decision {
            kind: AgentDecisionKind::Advisory {
                text: "The answer.".to_owned(),
            },
            selected: true,
            advances_continuation,
        })
    };
    let no_decision = FinishRuleOutcome::Settle(AgentDecisionOutcome::NoDecision);
    let cases = [
        // The last RunStep: a later request gives each call of a valid batch an output that
        // says that StoryOS did not execute it.
        (
            RunStepLimit::Last,
            batch.clone(),
            vec![text.clone(), call.clone()],
            answer(true),
        ),
        (
            RunStepLimit::Last,
            batch,
            vec![call.clone()],
            FinishRuleOutcome::NoResult,
        ),
        // A batch with a structural fault has a call that no output can answer.
        (
            RunStepLimit::Last,
            AgentDecisionOutcome::NoDecision,
            vec![text.clone(), cut_call.clone()],
            answer(false),
        ),
        (
            RunStepLimit::Last,
            AgentDecisionOutcome::NoDecision,
            vec![cut_call.clone()],
            no_decision.clone(),
        ),
        (
            RunStepLimit::Last,
            AgentDecisionOutcome::NoDecision,
            vec![text.clone()],
            answer(true),
        ),
        // Before a `finish` call that counted, a text with a cut call is not the prior response
        // of the next RunStep: the Run completes with the results that it made.
        (
            RunStepLimit::Open,
            AgentDecisionOutcome::NoDecision,
            vec![text.clone(), cut_call],
            no_decision,
        ),
        (
            RunStepLimit::Open,
            AgentDecisionOutcome::NoDecision,
            vec![text],
            FinishRuleOutcome::NextRunStep,
        ),
    ];
    for (limit, outcome, items, expected) in cases {
        let choice = step_tool_choice(limit, &[]);
        assert_eq!(
            apply_finish_rule(choice, limit, outcome, StreamItemState::Complete, &items),
            expected,
            "{limit:?} {items:?}"
        );
    }
}
