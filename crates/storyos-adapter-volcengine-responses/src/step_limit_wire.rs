//! The sentences of the current mapping that give the model the limit of RunSteps of its Run and
//! tell it when its work comes near to that limit (ADR 0103 section 5). `{limit}`, `{left}`, and
//! `{tool}` are the places of the numbers and of the name of the output Tool.

use storyos_core::{RunStepPlace, StepLimitNotice, ToolChoice};

/// The paragraph of a Run of the author with its limit of RunSteps. It follows the paragraph of
/// the steps in `TOOL_LOOP_INSTRUCTIONS`.
pub(crate) const AUTHOR_LIMIT_SENTENCE: &str = "You have at most {limit} steps for one author \
request. The last step is only for your final answer: StoryOS executes no function call of the \
last step.";

/// The paragraph of a Subrun with its limit of RunSteps. It follows the instructions of a Subrun.
pub(crate) const SUBRUN_LIMIT_SENTENCE: &str = "You have at most {limit} steps for this task. The \
last step is only for your final answer: StoryOS executes no function call of the last step.";

/// The sentence near the limit before the last RunStep of a Run whose Host executes no call of
/// the last RunStep.
pub(crate) const STEPS_LEFT_SENTENCE: &str = "Step limit: after this step, {left}. Complete the \
necessary work, and call finish in time.";

/// The sentence near the limit before the last RunStep of a Run with an output Tool.
pub(crate) const STEPS_LEFT_OUTPUT_SENTENCE: &str =
    "Step limit: after this step, {left}. Call {tool} in time.";

/// The sentence of the last RunStep of a Run whose Host executes no call of that RunStep.
pub(crate) const LAST_STEP_SENTENCE: &str = "Step limit: this is your last step. StoryOS executes \
no function call now. Write your final answer with the results that you have.";

/// The sentence of the last RunStep of a Run with an output Tool.
pub(crate) const LAST_STEP_OUTPUT_SENTENCE: &str =
    "Step limit: this is your last step. StoryOS executes only a {tool} call now.";

/// `AUTHOR_LIMIT_SENTENCE` with the limit `max_run_steps`.
pub(crate) fn author_limit_paragraph(max_run_steps: u64) -> String {
    AUTHOR_LIMIT_SENTENCE.replace("{limit}", &max_run_steps.to_string())
}

/// `SUBRUN_LIMIT_SENTENCE` with the limit `max_run_steps`.
pub(crate) fn subrun_limit_paragraph(max_run_steps: u64) -> String {
    SUBRUN_LIMIT_SENTENCE.replace("{limit}", &max_run_steps.to_string())
}

/// The sentence near the limit at the end of the instructions of the decision request at `place`
/// with `tool_choice`, when it has one. A Run with an output Tool gets the sentences that name
/// that Tool.
fn notice_sentence(place: RunStepPlace, tool_choice: ToolChoice) -> Option<String> {
    let left = |count: u64| match count {
        1 => "1 step is left".to_owned(),
        count => format!("{count} steps are left"),
    };
    let tool = place
        .last_step_operation
        .map(storyos_core::ToolOperation::function_name);
    let (sentence, count) = match (place.notice(tool_choice)?, tool) {
        (StepLimitNotice::StepsLeft(count), None) => (STEPS_LEFT_SENTENCE, Some(count)),
        (StepLimitNotice::StepsLeft(count), Some(_)) => (STEPS_LEFT_OUTPUT_SENTENCE, Some(count)),
        (StepLimitNotice::LastStep, None) => (LAST_STEP_SENTENCE, None),
        (StepLimitNotice::LastStep, Some(_)) => (LAST_STEP_OUTPUT_SENTENCE, None),
    };
    let sentence = match count {
        Some(count) => sentence.replace("{left}", &left(count)),
        None => sentence.to_owned(),
    };
    Some(sentence.replace("{tool}", tool.unwrap_or_default()))
}

/// `instructions` with the sentence near the limit of `place` at its end, as one more paragraph.
pub(crate) fn with_notice(
    instructions: String,
    place: RunStepPlace,
    tool_choice: ToolChoice,
) -> String {
    match notice_sentence(place, tool_choice) {
        Some(sentence) => format!("{instructions}\n\n{sentence}"),
        None => instructions,
    }
}

#[cfg(test)]
#[path = "step_limit_wire_tests.rs"]
mod tests;
