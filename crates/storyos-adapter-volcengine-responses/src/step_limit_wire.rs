//! The sentences of the current mapping that give the model the limit of RunSteps of its Run and
//! tell it when its work comes near to that limit (ADR 0103 section 5).

use storyos_core::{RunStepPlace, StepLimitNotice, ToolChoice};

/// The paragraph of a Run of the author with its limit of RunSteps. It follows the paragraph of
/// the steps in `TOOL_LOOP_INSTRUCTIONS`.
pub(crate) fn author_limit_paragraph(max_run_steps: u64) -> String {
    format!(
        "You have at most {max_run_steps} steps for one author request. The last step is only for \
         your final answer: StoryOS executes no function call of the last step."
    )
}

/// The sentence near the limit at the end of the instructions of the decision request at `place`
/// with `tool_choice`, when it has one. A Run whose Host executes no call of the last RunStep
/// gets the sentences of a Run of the author. A Run with an output Tool gets the sentences that
/// name that Tool.
pub(crate) fn notice_sentence(place: RunStepPlace, tool_choice: ToolChoice) -> Option<String> {
    let left = |count: u64| match count {
        1 => "1 step is left".to_owned(),
        count => format!("{count} steps are left"),
    };
    let tool = place
        .last_step_operation
        .map(storyos_core::ToolOperation::function_name);
    Some(match (place.notice(tool_choice)?, tool) {
        (StepLimitNotice::StepsLeft(count), None) => format!(
            "Step limit: after this step, {}. Complete the necessary work, and call finish in time.",
            left(count)
        ),
        (StepLimitNotice::StepsLeft(count), Some(tool)) => format!(
            "Step limit: after this step, {}. Call {tool} in time.",
            left(count)
        ),
        (StepLimitNotice::LastStep, None) => {
            "Step limit: this is your last step. StoryOS executes \
            no function call now. Write your final answer with the results that you have."
                .to_owned()
        }
        (StepLimitNotice::LastStep, Some(tool)) => {
            format!("Step limit: this is your last step. StoryOS executes only a {tool} call now.")
        }
    })
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
