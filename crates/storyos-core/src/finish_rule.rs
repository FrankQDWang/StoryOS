//! The finish rule of ADR 0091 section 4: a Run of the tool loop profile ends only after a
//! `finish` call that counted. Each request before such a call has `tool_choice: "required"`, and
//! the closing request after it has `tool_choice: "none"`.

use crate::{
    AgentDecisionKind, AgentDecisionOutcome, NativeStreamItem, StreamItemRole, StreamItemState,
    ToolCallState, ToolOperation, requests_function,
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
    /// A complete closing response with no text: the Run completes with no result, and the
    /// next Run of the Conversation continues from this response.
    NoResult,
}

/// Applies the finish rule to the outcome of one terminal response of a request with
/// `tool_choice`. A function request continues to its ToolCalls in each state. In the closing
/// RunStep, the text of a complete response is the final answer as an Advisory, also when it has
/// the shape of a reply form, and a final answer with no text gives no result.
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
        _ if function_request => FinishRuleOutcome::Settle(outcome),
        ToolChoice::Required => FinishRuleOutcome::NextRunStep,
        ToolChoice::None => closing_answer(items),
        ToolChoice::Auto => FinishRuleOutcome::Settle(outcome),
    }
}

/// The final answer of a closing response with no function call: the text of its messages. A
/// response with an incomplete item, a refusal, or a Tool item makes no Agent Decision.
fn closing_answer(items: &[NativeStreamItem]) -> FinishRuleOutcome {
    if items.iter().any(|item| {
        item.state != StreamItemState::Complete
            || item.refusal.is_some()
            || item.role != StreamItemRole::Assistant
    }) {
        return FinishRuleOutcome::Settle(AgentDecisionOutcome::NoDecision);
    }
    let text = items
        .iter()
        .filter_map(|item| item.text.as_deref())
        .collect::<Vec<_>>()
        .join("\n\n");
    // PostgreSQL refuses U+0000 in `text` and `jsonb`.
    if text.trim().is_empty() || text.contains('\0') {
        return FinishRuleOutcome::NoResult;
    }
    FinishRuleOutcome::Settle(AgentDecisionOutcome::Decision {
        kind: AgentDecisionKind::Advisory { text },
        selected: true,
        advances_continuation: true,
    })
}

#[cfg(test)]
#[path = "finish_rule_tests.rs"]
mod tests;
