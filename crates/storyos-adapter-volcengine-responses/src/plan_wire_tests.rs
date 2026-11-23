use serde_json::json;
use storyos_application::CreateRequest;
use storyos_core::ToolChoice;

use crate::skill_wire_tests::{body, closing, first, summary};
use crate::wire::UPDATE_PLAN_INSTRUCTION;

/// The Tools that the body declares by name, and the count of the paragraph of `update_plan` in
/// its Host instructions.
fn plan_facts(body: &serde_json::Value) -> (Vec<String>, usize) {
    let names = (body["tools"].as_array().into_iter().flatten())
        .map(|tool| tool["name"].as_str().unwrap().to_owned())
        .collect();
    let paragraphs = (body["instructions"].as_str().unwrap()).matches(UPDATE_PLAN_INSTRUCTION);
    (names, paragraphs.count())
}

/// ADR 0103 section 2: a decision request and a closing request of a Run of the author declare
/// `update_plan` before `finish`, with the exact description and a strict schema, and have the
/// paragraph of `update_plan` one time. The request of a capture, of a consolidation pass, of an
/// intent pass, and a summary request have no such Tool and no such paragraph.
#[tokio::test]
async fn only_a_request_of_a_run_of_the_author_declares_update_plan_with_its_paragraph() {
    let decision = CreateRequest {
        tool_choice: ToolChoice::Required,
        ..first(
            storyos_core::AGENT_PLAN_RUN_PLAN_MAPPING_REVISION,
            storyos_core::AGENT_PLAN_REGISTRATION.revision,
        )
    };
    let declared = body(decision.clone()).await;
    let names = plan_facts(&declared).0;
    let without_plan = |(names, paragraphs): (Vec<String>, usize)| {
        (names.iter().any(|name| name == "update_plan"), paragraphs)
    };

    assert_eq!(
        (
            &names[names.len() - 2..],
            &declared["tools"][names.len() - 2],
            plan_facts(&declared).1,
            plan_facts(&body(closing(decision.clone())).await).1,
        ),
        (
            &["update_plan".to_owned(), "finish".to_owned()][..],
            &json!({
                "type": "function",
                "name": "update_plan",
                "description": "Record the plan of this work as a short list of steps, or \
                                replace the list that you recorded before. The author sees the \
                                newest list beside your work.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "steps": {
                            "type": "array",
                            "description": "The complete list of 1 to 12 steps, in their order.",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "text": {
                                        "type": "string",
                                        "description": "The step in at most 100 characters on \
                                                        one line."
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
                },
                "strict": true
            }),
            1,
            1,
        )
    );
    assert_eq!(
        [
            without_plan(plan_facts(
                &body(crate::capture_wire::tests::conversation()).await
            )),
            without_plan(plan_facts(&body(crate::pass_wire::tests::full()).await)),
            without_plan(plan_facts(
                &body(crate::intent_wire::tests::intent_pass(
                    &crate::intent_wire::tests::FILE,
                    storyos_core::AGENT_PLAN_RUN_PLAN_MAPPING_REVISION,
                ))
                .await
            )),
            without_plan(plan_facts(&body(summary(decision)).await)),
        ],
        [(false, 0); 4]
    );
}
