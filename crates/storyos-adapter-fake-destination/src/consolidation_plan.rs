//! The pass script of the fake model (ADR 0094 section 6). The observations of a pass hold the
//! script: each text line `SCRIPT:operation <JSON operation>` of an observation is one operation of
//! the one `apply_memory_operations` call, in input order. A text line `SCRIPT:no-call` makes the
//! response a text with no call. An input with no script line gives an empty list.

use storyos_application::CreateRequest;
use storyos_core::{
    ContextSourceClass, NativeStreamItem, StreamItemRole, StreamItemState, ToolOperation,
};

use crate::create_plan::{FakeCreate, complete, evidence_item};

/// The scripted response of the one RunStep of `request`.
pub(crate) fn plan(request: &CreateRequest) -> FakeCreate {
    let lines: Vec<String> = request
        .context
        .iter()
        .filter(|item| item.source_class == ContextSourceClass::MemoryObservations)
        .flat_map(|item| item.content.lines())
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|line| line["text"].as_str().map(str::to_owned))
        .flat_map(|text| text.lines().map(str::to_owned).collect::<Vec<_>>())
        .collect();
    if lines.iter().any(|line| line == "SCRIPT:no-call") {
        return complete("The Memory needs no change.");
    }
    let operations: Vec<&str> = lines
        .iter()
        .filter_map(|line| line.strip_prefix("SCRIPT:operation "))
        .collect();
    FakeCreate {
        items: vec![NativeStreamItem {
            item_id: "fc_consolidation".to_owned(),
            call_id: Some("call_consolidation".to_owned()),
            arguments: Some(format!(r#"{{"operations":[{}]}}"#, operations.join(","))),
            native_type: Some(storyos_core::FUNCTION_CALL_NATIVE_TYPE.to_owned()),
            name: Some(
                ToolOperation::ApplyMemoryOperations
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
