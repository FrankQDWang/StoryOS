//! The Host Tool `update_plan` (ADR 0103 section 2): the Agent of a Run of the author records the
//! plan of its work as a short list of steps. Each accepted call replaces the list. The plan of a
//! Run is the list of its newest accepted call: it has no table, and it gives no capability, no
//! budget, and no authority. The Host changes no step, also when the Run ends.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{IntakeRefusal, ToolCallState, ToolIntake, ToolOperation, ToolResultText, ToolSpec};

pub const UPDATE_PLAN_TOOL_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.update-plan.v1",
    operation: ToolOperation::UpdatePlan,
    intake_contract: &[],
    envelope: None,
    result_text_max_bytes: 512,
};

/// The most steps of one plan.
pub const PLAN_STEPS_MAX: usize = 12;
/// The most characters of the text of one step.
pub const PLAN_STEP_TEXT_MAX_CHARS: usize = 100;

pub(crate) const DESCRIPTION: &str = "Record the plan of this work as a short list of steps, or \
replace the list that you recorded before. The author sees the newest list beside your work.";

/// The result of an accepted call.
pub const PLAN_RECORDED: &str = "The plan is recorded.";

/// The state of one step of a plan.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlanStepStatus {
    Pending,
    InProgress,
    Completed,
}

impl PlanStepStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
        }
    }
}

/// One step of a plan. A step has no identity across two calls.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlanStep {
    pub text: String,
    pub status: PlanStepStatus,
}

/// The strict JSON Schema of the arguments.
pub(crate) fn parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "steps": {
                "type": "array",
                "description": format!(
                    "The complete list of 1 to {PLAN_STEPS_MAX} steps, in their order."
                ),
                "items": {
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": format!(
                                "The step in at most {PLAN_STEP_TEXT_MAX_CHARS} characters on one \
                                 line."
                            )
                        },
                        "status": {
                            "type": "string",
                            "enum": ["pending", "in_progress", "completed"]
                        }
                    },
                    "required": ["text", "status"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["steps"],
        "additionalProperties": false
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    steps: Vec<PlanStep>,
}

/// The typed intake. The ToolCall record cannot hold U+0000, so a text with it does not fit. The
/// rules of a plan are not part of the intake: the Host refuses a call that breaks one of them
/// with `plan_refusal`.
pub(crate) fn intake(arguments: &Value) -> Result<ToolIntake, IntakeRefusal> {
    // serde also reads a struct from a JSON array, and the schema gives each step only as an
    // object.
    let objects = (arguments.get("steps").and_then(Value::as_array))
        .is_some_and(|steps| steps.iter().all(Value::is_object));
    if !objects {
        return Err(IntakeRefusal::Malformed);
    }
    let Arguments { steps } =
        Arguments::deserialize(arguments).map_err(|_| IntakeRefusal::Malformed)?;
    match steps.iter().any(|step| step.text.contains('\0')) {
        true => Err(IntakeRefusal::UnstorableText),
        false => Ok(ToolIntake::UpdatePlan { steps }),
    }
}

/// The validated arguments of an intake of `steps`.
pub(crate) fn arguments(steps: &[PlanStep]) -> Value {
    let steps: Vec<Value> = (steps.iter())
        .map(|step| json!({ "text": step.text, "status": step.status.as_str() }))
        .collect();
    json!({ "steps": steps })
}

/// Why the Host refused one `update_plan` call. A refused call changes no plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanRefusal {
    StepCount,
    StepText,
    InProgressCount,
}

impl PlanRefusal {
    /// The stable label of the refusal, for the `refusal` of the ToolCall row.
    pub fn label(self) -> &'static str {
        match self {
            Self::StepCount => "plan_step_count",
            Self::StepText => "plan_step_text",
            Self::InProgressCount => "plan_in_progress_count",
        }
    }

    /// The result of the refused call.
    pub fn result_text(self) -> &'static str {
        match self {
            Self::StepCount => "The plan must have 1 through 12 steps.",
            Self::StepText => "A step text must have 1 through 100 characters on one line.",
            Self::InProgressCount => "At most one step can be in_progress.",
        }
    }
}

/// Whether `c` is a Unicode line break: the line feed, the vertical tab, the form feed, the
/// carriage return, the next line, the line separator, or the paragraph separator.
pub(crate) fn line_break(c: char) -> bool {
    matches!(
        c,
        '\n' | '\u{b}' | '\u{c}' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

/// The first rule of a plan that `steps` breaks, in the order of the results: 1 through 12 steps,
/// a text of 1 through 100 characters on one line that is not only white space, and at most one
/// step `in_progress`.
pub fn plan_refusal(steps: &[PlanStep]) -> Option<PlanRefusal> {
    if steps.is_empty() || steps.len() > PLAN_STEPS_MAX {
        return Some(PlanRefusal::StepCount);
    }
    if steps.iter().any(|step| {
        step.text.trim().is_empty()
            || step.text.chars().count() > PLAN_STEP_TEXT_MAX_CHARS
            || step.text.chars().any(line_break)
    }) {
        return Some(PlanRefusal::StepText);
    }
    let in_progress = (steps.iter())
        .filter(|step| step.status == PlanStepStatus::InProgress)
        .count();
    (in_progress > 1).then_some(PlanRefusal::InProgressCount)
}

/// The resolution of one `update_plan` call: its state, its refusal label, and its result.
pub fn settle_plan_call(
    steps: &[PlanStep],
) -> (ToolCallState, Option<&'static str>, ToolResultText) {
    match plan_refusal(steps) {
        None => (
            ToolCallState::Settled,
            None,
            crate::host_tool_result(PLAN_RECORDED),
        ),
        Some(refusal) => (
            ToolCallState::Refused,
            Some(refusal.label()),
            crate::host_tool_result(refusal.result_text()),
        ),
    }
}

/// One stored `update_plan` call of a Run: its state and its stored arguments.
#[derive(Clone, Copy, Debug)]
pub struct StoredPlanCall<'a> {
    pub state: ToolCallState,
    pub arguments: &'a Value,
}

/// The plan of a Run from its stored `update_plan` calls `calls`, in the order of the RunSteps
/// and of the calls: the steps of the newest settled call. A Run with no settled call has no
/// plan.
pub fn run_plan(calls: &[StoredPlanCall<'_>]) -> Option<Vec<PlanStep>> {
    calls.iter().rev().find_map(|call| match call.state {
        ToolCallState::Settled => Arguments::deserialize(call.arguments)
            .ok()
            .map(|arguments| arguments.steps),
        ToolCallState::Pending
        | ToolCallState::Dispatched
        | ToolCallState::Failed
        | ToolCallState::Unknown
        | ToolCallState::Refused
        | ToolCallState::AwaitingApproval => None,
    })
}

#[cfg(test)]
#[path = "update_plan_tests.rs"]
mod tests;
