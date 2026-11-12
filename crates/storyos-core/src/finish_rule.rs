//! The finish rule of ADR 0091 section 4: a Run of the tool loop profile ends only after a
//! `finish` call that counted. Each request before such a call has `tool_choice: "required"`, and
//! the closing request after it has `tool_choice: "none"`.

use crate::{
    AgentDecisionKind, AgentDecisionOutcome, NativeStreamItem, ToolCallState, ToolOperation,
    requests_function,
};

/// The `tool_choice` of one decision request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolChoice {
    /// The reply form profile: the model calls an exposed Tool or replies.
    Auto,
    /// A request before a `finish` call that counted: the response holds a function call.
    Required,
    /// The closing request after a `finish` call that counted. The route does not enforce it.
    None,
}

impl ToolChoice {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Required => "required",
            Self::None => "none",
        }
    }
}

/// One ToolCall of the RunStep before the active RunStep.
#[derive(Clone, Copy, Debug)]
pub struct PriorStepCall {
    pub operation: ToolOperation,
    pub state: ToolCallState,
}

/// The `tool_choice` of the request of the active RunStep. A mapping that declares no Host Tool
/// sends `auto`. A `finish` call of the prior RunStep counts when it settled and no other call of
/// that response was refused: then the request is the closing request.
pub fn step_tool_choice(declares_host_tools: bool, prior_step: &[PriorStepCall]) -> ToolChoice {
    if !declares_host_tools {
        return ToolChoice::Auto;
    }
    let finished = prior_step.iter().any(|call| {
        call.operation == ToolOperation::Finish && call.state == ToolCallState::Settled
    });
    let refused = prior_step
        .iter()
        .any(|call| call.state == ToolCallState::Refused);
    if finished && !refused {
        ToolChoice::None
    } else {
        ToolChoice::Required
    }
}

/// What one terminal response does to its Run under the finish rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FinishRuleOutcome {
    /// The response settles with this outcome, as in the reply form profile.
    Settle(AgentDecisionOutcome),
    /// A response with no function call before a `finish` call that counted: it makes no Agent
    /// Decision, and the Host opens the next RunStep with the same `tool_choice`.
    NextRunStep,
}

/// Applies the finish rule to the outcome of one terminal response of a request with
/// `tool_choice`. A function request continues to its ToolCalls in each state. In the closing
/// RunStep, the text is the final answer, and a final answer with no text gives no result.
pub fn apply_finish_rule(
    tool_choice: ToolChoice,
    outcome: AgentDecisionOutcome,
    items: &[NativeStreamItem],
) -> FinishRuleOutcome {
    let function_request = matches!(
        outcome,
        AgentDecisionOutcome::Decision {
            kind: AgentDecisionKind::ToolRequest { .. },
            ..
        }
    ) || requests_function(items);
    match tool_choice {
        ToolChoice::Required if !function_request => FinishRuleOutcome::NextRunStep,
        ToolChoice::None => match outcome {
            AgentDecisionOutcome::Decision {
                kind: AgentDecisionKind::Advisory { ref text },
                ..
            } if text.trim().is_empty() => {
                FinishRuleOutcome::Settle(AgentDecisionOutcome::NoDecision)
            }
            outcome => FinishRuleOutcome::Settle(outcome),
        },
        ToolChoice::Auto | ToolChoice::Required => FinishRuleOutcome::Settle(outcome),
    }
}

#[cfg(test)]
#[path = "finish_rule_tests.rs"]
mod tests;
