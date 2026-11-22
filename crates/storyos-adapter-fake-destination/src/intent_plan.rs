//! The intent pass script of the fake model (ADR 0101 section 4). The Character File holds the
//! script: each line `SCRIPT:intent <JSON arguments>` is the one `record_character_intent` call of
//! one RunStep, in line order, and a line `SCRIPT:intent-no-call` makes the response of its RunStep
//! a text with no call. Each RunStep after the script lines, and each RunStep of a file with no
//! script line, gives the fixed reply.

use storyos_application::CreateRequest;
use storyos_core::{
    ContextSourceClass, NativeStreamItem, StreamItemRole, StreamItemState, ToolOperation,
};

use crate::create_plan::{FakeCreate, complete, evidence_item};

/// The fixed arguments of the fake reply.
pub(crate) const FAKE_INTENT_ARGUMENTS: &str =
    r#"{"intent":"我明天一早去渡口，看看那条船还在不在。","questions":["我以前去过河对岸吗？"]}"#;

/// The scripted response of the RunStep of `request`. Each RunStep has one call, so the number of
/// the Tool outputs of the request is the number of the RunStep before it.
pub(crate) fn plan(request: &CreateRequest) -> FakeCreate {
    let file = (request.context.iter())
        .find(|item| item.source_class == ContextSourceClass::CharacterFile)
        .map_or("", |item| item.content.as_str());
    let step = request.tool_outputs.len();
    let scripted = file
        .lines()
        .filter(|line| line.starts_with("SCRIPT:intent"))
        .nth(step);
    let arguments = match scripted {
        Some("SCRIPT:intent-no-call") => return complete("The character does nothing."),
        Some(line) => line
            .strip_prefix("SCRIPT:intent ")
            .unwrap_or(FAKE_INTENT_ARGUMENTS),
        None => FAKE_INTENT_ARGUMENTS,
    };
    FakeCreate {
        items: vec![NativeStreamItem {
            item_id: format!("fc_intent_{step}"),
            call_id: Some(format!("call_intent_{step}")),
            arguments: Some(arguments.to_owned()),
            native_type: Some(storyos_core::FUNCTION_CALL_NATIVE_TYPE.to_owned()),
            name: Some(
                ToolOperation::RecordCharacterIntent
                    .function_name()
                    .to_owned(),
            ),
            ..evidence_item(
                StreamItemRole::Tool,
                StreamItemState::Complete,
                "host_fake_function_call",
            )
        }],
    }
}
