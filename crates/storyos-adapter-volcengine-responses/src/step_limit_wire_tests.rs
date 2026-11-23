use storyos_application::CreateRequest;
use storyos_core::AgentRunKind;

use crate::skill_wire_tests::{body, closing, first, summary};
use crate::tests::write_memory_paragraph;
use crate::wire::{
    READ_TOOL_RESULT_INSTRUCTION, SUMMARY_INSTRUCTIONS, SUMMARY_TOPICS_INSTRUCTION,
    TOOL_LOOP_INSTRUCTIONS, TOOL_LOOP_RULES_INSTRUCTIONS, TOOL_RESULT_STUB_INSTRUCTION,
    UPDATE_PLAN_INSTRUCTION, WRITING_RULES_INSTRUCTION,
};

const CURRENT: &str = storyos_core::AGENT_PLAN_SUBRUN_MAPPING_REVISION;

/// The decision request of a Run of `kind` at `position` from `request`, with the `tool_choice`
/// of the finish rule for a request that is not a closing request.
fn at(kind: AgentRunKind, position: u64, request: CreateRequest) -> CreateRequest {
    CreateRequest {
        tool_choice: (kind.fixed_tool_choice())
            .unwrap_or_else(|| storyos_core::step_tool_choice(kind.run_step_limit(position), &[])),
        run_step: kind.run_step_place(position),
        ..request
    }
}

fn author(position: u64) -> CreateRequest {
    let request = first(CURRENT, storyos_core::AGENT_PLAN_REGISTRATION.revision);
    at(AgentRunKind::Author, position, request)
}

fn capture(position: u64) -> CreateRequest {
    at(
        AgentRunKind::MemoryCapture,
        position,
        crate::capture_wire::tests::conversation(),
    )
}

async fn instructions(request: CreateRequest) -> String {
    body(request).await["instructions"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// ADR 0103 section 5: each request of a Run of the author gives its limit of 24 RunSteps
/// directly after the paragraph of the steps. Only the decision requests of the last 4 RunSteps
/// have the sentence near the limit, at the end of the instructions.
#[tokio::test]
async fn a_request_of_a_run_of_the_author_gives_its_limit_and_the_last_four_give_the_steps_left() {
    let before = instructions(author(0)).await;
    let limit = format!(
        "{TOOL_LOOP_INSTRUCTIONS}\n\nYou have at most 24 steps for one author request. The last \
         step is only for your final answer: StoryOS executes no function call of the last step.\
         \n\n{TOOL_LOOP_RULES_INSTRUCTIONS}\n\n{WRITING_RULES_INSTRUCTION}\n\n\
         {READ_TOOL_RESULT_INSTRUCTION}\n\n{TOOL_RESULT_STUB_INSTRUCTION}\n\n\
         {UPDATE_PLAN_INSTRUCTION}\n\n{}",
        write_memory_paragraph()
    );
    let last = body(author(23)).await;

    assert_eq!(
        (
            &before,
            instructions(author(19)).await,
            instructions(author(20)).await,
            instructions(author(21)).await,
            instructions(author(22)).await,
            last["instructions"].as_str().unwrap(),
            last["tool_choice"].as_str().unwrap(),
        ),
        (
            &limit,
            limit.clone(),
            format!(
                "{limit}\n\nStep limit: after this step, 3 steps are left. Complete the \
                 necessary work, and call finish in time."
            ),
            format!(
                "{limit}\n\nStep limit: after this step, 2 steps are left. Complete the \
                 necessary work, and call finish in time."
            ),
            format!(
                "{limit}\n\nStep limit: after this step, 1 step is left. Complete the necessary \
                 work, and call finish in time."
            ),
            format!(
                "{limit}\n\nStep limit: this is your last step. StoryOS executes no function \
                 call now. Write your final answer with the results that you have."
            )
            .as_str(),
            "none",
        )
    );
}

/// ADR 0103 section 5: a capture keeps its one limit sentence, and the sentences near the limit
/// name its output Tool.
#[tokio::test]
async fn a_capture_has_one_limit_sentence_and_its_last_four_name_the_output_tool() {
    let before = instructions(capture(3)).await;

    assert_eq!(
        (
            before.matches("at most 8 steps").count(),
            before.matches("You have at most").count(),
            before == crate::capture_wire::capture_instructions(),
            instructions(capture(4)).await,
            instructions(capture(6)).await,
            instructions(capture(7)).await,
        ),
        (
            1,
            0,
            true,
            format!(
                "{before}\n\nStep limit: after this step, 3 steps are left. Call \
                 record_memory_observations in time."
            ),
            format!(
                "{before}\n\nStep limit: after this step, 1 step is left. Call \
                 record_memory_observations in time."
            ),
            format!(
                "{before}\n\nStep limit: this is your last step. StoryOS executes only a \
                 record_memory_observations call now."
            ),
        )
    );
}

/// ADR 0103 section 5: a closing request after a `finish` call that counted is not the last
/// RunStep, so it has no sentence near the limit, also in the last 4 RunSteps.
#[tokio::test]
async fn a_closing_request_before_the_last_run_step_has_no_sentence_near_the_limit() {
    let closing_at = |position| CreateRequest {
        run_step: AgentRunKind::Author.run_step_place(position),
        ..closing(author(0))
    };
    let early = instructions(closing_at(5)).await;

    assert_eq!(
        (
            early.contains("Step limit:"),
            instructions(closing_at(21)).await,
        ),
        (false, early.clone())
    );
}

/// ADR 0091 section 5 and ADR 0103 section 5: the output of a call that the Host did not execute
/// names the limit of the kind of the Run.
#[tokio::test]
async fn the_output_of_an_unexecuted_call_names_the_limit_of_the_kind() {
    let unexecuted = |request: CreateRequest| CreateRequest {
        unexecuted_call_ids: vec!["call_1".to_owned()],
        ..request
    };
    let output = |body: serde_json::Value| body["input"][1]["output"].as_str().unwrap().to_owned();

    assert_eq!(
        [
            output(body(unexecuted(author(23))).await),
            output(body(unexecuted(capture(7))).await),
        ],
        [24, 8].map(|limit| format!(
            "StoryOS did not execute this call, because the Run reached its limit of {limit} \
             RunSteps. No result is available, and this is not an empty result."
        ))
    );
}

/// ADR 0103 section 5: a compaction summary request is not a decision request, so it has no
/// limit sentence and no sentence near the limit, also in the last 4 RunSteps.
#[tokio::test]
async fn a_compaction_summary_request_has_no_sentence_of_the_step_limit() {
    let mut summaries = Vec::new();
    for position in [0, 20, 23] {
        summaries.push(instructions(summary(author(position))).await);
    }

    assert_eq!(
        summaries,
        vec![format!("{SUMMARY_INSTRUCTIONS}\n\n{SUMMARY_TOPICS_INSTRUCTION}"); 3]
    );
}
