use super::{PriorStepCall, ToolChoice, step_tool_choice};
use crate::{ToolCallState, ToolOperation};

fn call(operation: ToolOperation, state: ToolCallState) -> PriorStepCall {
    PriorStepCall { operation, state }
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
