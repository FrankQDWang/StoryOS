use serde_json::{Value, json};

use super::{
    PlanRefusal, PlanStep, PlanStepStatus, StoredPlanCall, UPDATE_PLAN_TOOL_SPEC, run_plan,
    settle_plan_call,
};
use crate::{IntakeRefusal, ToolCallState, ToolIntake};

fn step(text: &str, status: PlanStepStatus) -> PlanStep {
    PlanStep {
        text: text.to_owned(),
        status,
    }
}

fn call(state: ToolCallState, arguments: &Value) -> StoredPlanCall<'_> {
    StoredPlanCall { state, arguments }
}

fn pending(text: &str) -> PlanStep {
    step(text, PlanStepStatus::Pending)
}

/// The state, the refusal label, and the result text of a call of `steps`.
fn settled(steps: &[PlanStep]) -> (ToolCallState, Option<&'static str>, String) {
    let (state, refusal, result) = settle_plan_call(steps);
    (state, refusal, result.text)
}

fn refused(refusal: PlanRefusal) -> (ToolCallState, Option<&'static str>, String) {
    (
        ToolCallState::Refused,
        Some(refusal.label()),
        refusal.result_text().to_owned(),
    )
}

/// ADR 0103 section 2: a call is accepted when the list has 1 through 12 steps, each text has 1
/// through 100 characters on one line and is not only white space, and at most one step is
/// `in_progress`. A refused call gets the first sentence of the rules that applies.
#[test]
fn a_plan_call_is_accepted_only_in_the_bounds_and_a_refusal_gives_the_first_rule() {
    let accepted = (
        ToolCallState::Settled,
        None,
        "The plan is recorded.".to_owned(),
    );
    let longest = "雪".repeat(100);
    let twelve: Vec<PlanStep> = (1..=12).map(|n| pending(&format!("第{n}步"))).collect();
    let thirteen: Vec<PlanStep> = (1..=13).map(|n| pending(&format!("第{n}步"))).collect();
    let two_in_progress = [
        step("读第一章", PlanStepStatus::InProgress),
        step("读第二章", PlanStepStatus::InProgress),
    ];
    let one_of_each = [
        step("读第一章", PlanStepStatus::Completed),
        step("读第二章", PlanStepStatus::InProgress),
        pending("回答"),
    ];

    assert_eq!(
        [
            settled(&[pending("读第一章")]),
            settled(&twelve),
            settled(&one_of_each),
            settled(&[pending(&longest)]),
            settled(&[]),
            settled(&thirteen),
            settled(&[pending(&"雪".repeat(101))]),
            settled(&[pending("")]),
            settled(&[pending(" \t\u{3000}")]),
            settled(&[pending("读\n第一章")]),
            settled(&[pending("读\u{2028}第一章")]),
            settled(&[pending("读\u{2029}第一章")]),
            settled(&two_in_progress),
            // Thirteen steps with an empty text: the count is the first rule.
            settled(&[thirteen.clone(), vec![pending("")]].concat()),
            // An empty text and two steps in_progress: the text is the first rule.
            settled(&[two_in_progress.to_vec(), vec![pending("")]].concat()),
        ],
        [
            accepted.clone(),
            accepted.clone(),
            accepted.clone(),
            accepted,
            refused(PlanRefusal::StepCount),
            refused(PlanRefusal::StepCount),
            refused(PlanRefusal::StepText),
            refused(PlanRefusal::StepText),
            refused(PlanRefusal::StepText),
            refused(PlanRefusal::StepText),
            refused(PlanRefusal::StepText),
            refused(PlanRefusal::StepText),
            refused(PlanRefusal::InProgressCount),
            refused(PlanRefusal::StepCount),
            refused(PlanRefusal::StepText),
        ]
    );
    assert_eq!(
        [
            PlanRefusal::StepCount,
            PlanRefusal::StepText,
            PlanRefusal::InProgressCount
        ]
        .map(PlanRefusal::result_text),
        [
            "The plan must have 1 through 12 steps.",
            "A step text must have 1 through 100 characters on one line.",
            "At most one step can be in_progress.",
        ]
    );
}

/// Arguments of another form are refused as each other Host Tool refuses them: a missing or an
/// unknown field, a status that is not one of the three, and a text of another type. The
/// ToolCall record cannot hold U+0000.
#[test]
fn the_intake_takes_only_a_list_of_texts_with_a_status() {
    let intake = |arguments: Value| UPDATE_PLAN_TOOL_SPEC.intake(&arguments);
    let row = |status: &str| json!({"steps": [{"text": "读第一章", "status": status}]});

    assert_eq!(
        [
            intake(row("in_progress")),
            intake(json!({"steps": []})),
            intake(row("done")),
            intake(json!({"steps": [{"text": 7, "status": "pending"}]})),
            intake(json!({"steps": [{"text": "读", "status": "pending", "id": 1}]})),
            intake(json!({"plan": []})),
            intake(json!({"steps": [{"text": "读\u{0}", "status": "pending"}]})),
        ],
        [
            Ok(ToolIntake::UpdatePlan {
                steps: vec![step("读第一章", PlanStepStatus::InProgress)]
            }),
            Ok(ToolIntake::UpdatePlan { steps: Vec::new() }),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::Malformed),
            Err(IntakeRefusal::UnstorableText),
        ]
    );
}

/// ADR 0103 section 2: the plan of a Run is the list of its newest accepted call. A refused call
/// after an accepted call does not change the plan, and a Run with only refused calls has no
/// plan.
#[test]
fn the_plan_of_a_run_is_the_list_of_its_newest_accepted_call() {
    let first = json!({"steps": [{"text": "读第一章", "status": "in_progress"}]});
    let second = json!({"steps": [{"text": "读第一章", "status": "completed"},
                                  {"text": "回答", "status": "in_progress"}]});
    let refused_text = Value::String(r#"{"steps":[]}"#.to_owned());
    let (first, second, refused_text) = (&first, &second, &refused_text);

    assert_eq!(
        [
            run_plan(&[
                call(ToolCallState::Settled, first),
                call(ToolCallState::Settled, second),
            ]),
            run_plan(&[
                call(ToolCallState::Settled, first),
                call(ToolCallState::Refused, refused_text),
                call(ToolCallState::Refused, second),
            ]),
            run_plan(&[
                call(ToolCallState::Refused, refused_text),
                call(ToolCallState::Refused, first),
            ]),
            run_plan(&[]),
        ],
        [
            Some(vec![
                step("读第一章", PlanStepStatus::Completed),
                step("回答", PlanStepStatus::InProgress),
            ]),
            Some(vec![step("读第一章", PlanStepStatus::InProgress)]),
            None,
            None,
        ]
    );
}
