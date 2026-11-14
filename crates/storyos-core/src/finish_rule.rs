//! The finish rule of ADR 0091 section 4: a Run ends only after a `finish` call that counted. Each request before such a call has `tool_choice: "required"`, and
//! the closing request after it has `tool_choice: "none"`. The RunStep limit of section 5 ends a
//! Run at its last RunStep.

use crate::{
    AgentDecisionKind, AgentDecisionOutcome, NativeStreamItem, StreamItemRole, StreamItemState,
    ToolCallState, ToolOperation, requests_function,
};

/// The `tool_choice` of one decision request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolChoice {
    /// A request before a `finish` call that counted: the response holds a function call.
    Required,
    /// The closing request after a `finish` call that counted, the request of the last RunStep,
    /// and a compaction summary request. The route does not enforce it.
    None,
}

impl ToolChoice {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::None => "none",
        }
    }
}

/// The most RunSteps of one Run (ADR 0091 section 5).
pub const MAX_RUN_STEPS: u64 = 24;

/// The output of each function call of a response of the last RunStep. The request that names
/// that response sends it, because each function call needs an output (ADR 0091 section 5).
pub const UNEXECUTED_CALL_OUTPUT: &str = "StoryOS did not execute this call, because the Run \
     reached its limit of 24 RunSteps. No result is available, and this is not an empty result.";

/// The place of the active RunStep against `MAX_RUN_STEPS`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunStepLimit {
    Open,
    /// The request has `tool_choice: "none"`, the text of its response is the final answer, and
    /// the Host executes no function call of that response.
    Last,
}

impl RunStepLimit {
    /// The limit of the RunStep at `decision_position`, from 0. A steering input opens no new
    /// count, so each RunStep at or after the limit is the last.
    pub fn at(decision_position: u64) -> Self {
        if decision_position.saturating_add(1) >= MAX_RUN_STEPS {
            Self::Last
        } else {
            Self::Open
        }
    }

    /// Whether the Host executes the function calls of a response of this RunStep. It executes
    /// none in the last RunStep, also when a recovery path settles the response.
    pub fn executes_calls(self) -> bool {
        self == Self::Open
    }
}

/// One ToolCall of the RunStep before the active RunStep.
#[derive(Clone, Copy, Debug)]
pub struct PriorStepCall {
    /// `None` for a refused call whose name the Run does not expose.
    pub operation: Option<ToolOperation>,
    pub state: ToolCallState,
}

/// The `tool_choice` of the request of the active RunStep. The last RunStep sends `none`. A `finish` call of the prior RunStep counts when
/// it settled and no other call of that response was refused: then the request is the closing
/// request.
pub fn step_tool_choice(limit: RunStepLimit, prior_step: &[PriorStepCall]) -> ToolChoice {
    if limit == RunStepLimit::Last {
        return ToolChoice::None;
    }
    let finished = prior_step.iter().any(|call| {
        call.operation == Some(ToolOperation::Finish) && call.state == ToolCallState::Settled
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
    /// The response settles with this outcome: the Agent Decision of a function request, a
    /// response with function calls that makes no Agent Decision, or the final answer.
    Settle(AgentDecisionOutcome),
    /// A response with no function call before a `finish` call that counted: it makes no Agent
    /// Decision, and the Host opens the next RunStep with the same `tool_choice`.
    NextRunStep,
    /// A complete closing response with no text: the Run completes with no result, and the
    /// next Run of the Conversation continues from this response.
    NoResult,
}

/// Applies the finish rule to the outcome of one terminal response in `response_state` of a
/// request with `tool_choice`. A function request continues to its ToolCalls in each state
/// before the last RunStep. In the closing RunStep, the text of a complete response is the final
/// answer as an Advisory, and a final answer with no text gives no result. A response with a Tool
/// item that no function output can answer is never a prior response (ADR 0091 section 5).
pub fn apply_finish_rule(
    tool_choice: ToolChoice,
    limit: RunStepLimit,
    outcome: AgentDecisionOutcome,
    response_state: StreamItemState,
    items: &[NativeStreamItem],
) -> FinishRuleOutcome {
    let function_request = matches!(
        outcome,
        AgentDecisionOutcome::Decision {
            kind: AgentDecisionKind::ToolRequest { .. },
            ..
        }
    ) || requests_function(items);
    let tool_item = items.iter().any(|item| item.role == StreamItemRole::Tool);
    match (tool_choice, limit) {
        // The Host executes no call of the last RunStep. A later request gives each call of a
        // valid batch the output `UNEXECUTED_CALL_OUTPUT`. A batch with a structural fault has a
        // call that no output can name, so its response does not advance the continuation.
        (ToolChoice::None, RunStepLimit::Last) => {
            let answer: Vec<_> = items
                .iter()
                .filter(|item| item.role != StreamItemRole::Tool)
                .cloned()
                .collect();
            let answerable = !tool_item
                || matches!(
                    outcome,
                    AgentDecisionOutcome::Decision {
                        kind: AgentDecisionKind::ToolRequest { .. },
                        ..
                    }
                );
            match closing_answer(response_state, &answer) {
                FinishRuleOutcome::Settle(AgentDecisionOutcome::Decision {
                    kind,
                    selected,
                    ..
                }) if !answerable => FinishRuleOutcome::Settle(AgentDecisionOutcome::Decision {
                    kind,
                    selected,
                    advances_continuation: false,
                }),
                FinishRuleOutcome::NoResult if !answerable => {
                    FinishRuleOutcome::Settle(AgentDecisionOutcome::NoDecision)
                }
                rule => rule,
            }
        }
        _ if function_request => FinishRuleOutcome::Settle(outcome),
        // A Tool item that is not a complete function call is a structural fault (ADR 0091).
        (ToolChoice::Required, _) if tool_item => FinishRuleOutcome::Settle(outcome),
        (ToolChoice::Required, _) => FinishRuleOutcome::NextRunStep,
        (ToolChoice::None, RunStepLimit::Open) => closing_answer(response_state, items),
    }
}

/// The final answer of a closing response with no function call: the text of its messages. A
/// response that is not complete, or that has an incomplete item, a refusal, or a Tool item,
/// makes no Agent Decision (ADR 0052).
fn closing_answer(
    response_state: StreamItemState,
    items: &[NativeStreamItem],
) -> FinishRuleOutcome {
    if response_state != StreamItemState::Complete
        || items.iter().any(|item| {
            item.state != StreamItemState::Complete
                || item.refusal.is_some()
                || item.role != StreamItemRole::Assistant
        })
    {
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
