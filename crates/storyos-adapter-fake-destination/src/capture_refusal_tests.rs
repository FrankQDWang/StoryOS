use storyos_application::{
    CreatePurpose, CreateRequest, DestinationRequest, ModelProviderAdapter, PreDispatchRefusal,
    PriorInput, RequestAttempt, RequestContextItem, RequestRoute,
};
use storyos_core::{
    AgentRunKind, CaptureInputKind, ContextSourceClass, ExposureFacts, ModelAdapter, ToolChoice,
    ToolExposure,
};

use super::FakeDestination;

/// The first request of a capture of a Chapter with `context` and `tool_exposure`.
pub(crate) fn capture(
    context: Vec<RequestContextItem>,
    tool_exposure: ToolExposure,
) -> DestinationRequest {
    DestinationRequest::Create(CreateRequest {
        attempt: RequestAttempt::New,
        purpose: CreatePurpose::MemoryCapture(CaptureInputKind::Chapter),
        route: RequestRoute {
            adapter: ModelAdapter::HostFake,
            provider_model_id: "host-fake".to_owned(),
            mapping_revision: storyos_core::HOST_FAKE_TOOL_LOOP_MAPPING_REVISION.to_owned(),
            endpoint: None,
            credential_reference: None,
            bounds: None,
        },
        author_message: String::new(),
        chapter_id: String::new(),
        passage_resolution: None,
        passage_input: None,
        context,
        earlier_author_messages: Vec::new(),
        earlier_turns: None,
        declared_targets: Vec::new(),
        selection_references: Vec::new(),
        previous_response_reference: None,
        prior_input: PriorInput::Current,
        tool_exposure,
        tool_choice: ToolChoice::Required,
        successor_of: None,
        tool_outputs: Vec::new(),
        run_step: storyos_core::AgentRunKind::MemoryCapture
            .run_step_place(/*decision_position*/ 0),
        unexecuted_call_ids: Vec::new(),
    })
}

/// The fake refuses each capture request that the Agent Plan mapping refuses: a request with no
/// input item or with no output Tool. A complete request names the kind of its input.
#[tokio::test]
async fn the_fake_refuses_a_capture_with_no_input_or_no_output_tool_as_the_real_route_does() {
    let tools = ToolExposure::compute(&ExposureFacts {
        bound: &[],
        model_registration_revision: storyos_core::HOST_FAKE_TOOL_LOOP_REGISTRATION.revision,
        skill_catalog: false,
        memory_use: false,
        hybrid_search: false,
        run_kind: AgentRunKind::MemoryCapture,
    });
    let input = vec![RequestContextItem {
        source_class: ContextSourceClass::MemoryCaptureInput,
        content: "雨下了一整夜。".to_owned(),
        full_input_only: false,
    }];
    let mut results = Vec::new();
    for request in [
        capture(Vec::new(), tools.clone()),
        capture(input.clone(), ToolExposure::empty()),
        capture(input, tools),
    ] {
        results.push(match FakeDestination.prepare(&request).await {
            Ok(prepared) => {
                let payload = prepared.projection.serialized_payload.unwrap();
                let wire: serde_json::Value = serde_json::from_str(&payload).unwrap();
                Ok(wire["capture_input"].clone())
            }
            Err(refusal) => Err(refusal),
        });
    }
    assert_eq!(
        results,
        [
            Err(PreDispatchRefusal::UnsupportedRequest),
            Err(PreDispatchRefusal::UnsupportedRequest),
            Ok(serde_json::json!("chapter"))
        ]
    );
}
