//! The capture script of the fake model (ADR 0094 section 5). The input of a capture holds the
//! script: each line `SCRIPT:capture <function name> <JSON arguments>` is the one call of one
//! RunStep, in line order. Each RunStep after the script lines, and each RunStep of an input with
//! no script line, calls `record_memory_observations` with an empty list.

use storyos_application::CreateRequest;
use storyos_core::{
    ContextSourceClass, NativeStreamItem, StreamItemRole, StreamItemState, ToolOperation,
};

use crate::create_plan::{FakeCreate, evidence_item};

/// The scripted call of the RunStep of `request`. Each RunStep has one call, so the number of
/// the Tool outputs of the request is the number of the RunStep before it.
pub(crate) fn plan(request: &CreateRequest) -> FakeCreate {
    let input = request
        .context
        .iter()
        .find(|item| item.source_class == ContextSourceClass::MemoryCaptureInput)
        .map_or("", |item| item.content.as_str());
    let step = request.tool_outputs.len();
    let (name, arguments) = input
        .lines()
        .filter_map(|line| line.strip_prefix("SCRIPT:capture "))
        .nth(step)
        .and_then(|line| line.split_once(' '))
        .unwrap_or((
            ToolOperation::RecordMemoryObservations.function_name(),
            r#"{"observations":[]}"#,
        ));
    FakeCreate {
        items: vec![NativeStreamItem {
            item_id: format!("fc_capture_{step}"),
            call_id: Some(format!("call_capture_{step}")),
            arguments: Some(arguments.to_owned()),
            native_type: Some(storyos_core::FUNCTION_CALL_NATIVE_TYPE.to_owned()),
            name: Some(name.to_owned()),
            ..evidence_item(
                StreamItemRole::Tool,
                StreamItemState::Complete,
                "host_fake_function_call",
            )
        }],
    }
}
