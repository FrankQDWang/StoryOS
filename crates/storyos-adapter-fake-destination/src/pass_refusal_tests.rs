use storyos_application::{
    CreatePurpose, CreateRequest, DestinationRequest, ModelProviderAdapter, PreDispatchRefusal,
    RequestContextItem,
};
use storyos_core::{AgentRunKind, ContextSourceClass, ExposureFacts, ToolExposure};

use super::FakeDestination;

/// The pass request with `context`, `tool_exposure`, and the prior response `reference`.
fn pass(
    context: Vec<RequestContextItem>,
    tool_exposure: ToolExposure,
    reference: Option<&str>,
) -> DestinationRequest {
    let DestinationRequest::Create(create) =
        crate::capture_refusal_tests::capture(context, tool_exposure)
    else {
        unreachable!("the helper makes a create request")
    };
    DestinationRequest::Create(CreateRequest {
        purpose: CreatePurpose::MemoryConsolidation,
        previous_response_reference: reference.map(str::to_owned),
        ..create
    })
}

/// The fake refuses each pass request that the Agent Plan mapping refuses: a request with no
/// observations item, with no output Tool, or with a prior response reference.
#[tokio::test]
async fn the_fake_refuses_a_pass_with_no_observation_no_output_tool_or_a_prior_reference() {
    let tools = ToolExposure::compute(&ExposureFacts {
        bound: &[],
        model_registration_revision: storyos_core::HOST_FAKE_TOOL_LOOP_REGISTRATION.revision,
        skill_catalog: false,
        memory_use: false,
        hybrid_search: false,
        run_kind: AgentRunKind::MemoryConsolidation,
    });
    let item = |source_class, content: &str| RequestContextItem {
        source_class,
        content: content.to_owned(),
        full_input_only: false,
    };
    let entries = item(ContextSourceClass::MemoryEntries, "{\"id\":1}");
    let observations = item(
        ContextSourceClass::MemoryObservations,
        "{\"observation\":1}",
    );
    let both = vec![entries.clone(), observations];
    let mut results = Vec::new();
    for request in [
        pass(vec![entries], tools.clone(), None),
        pass(both.clone(), ToolExposure::empty(), None),
        pass(both.clone(), tools.clone(), Some("resp_1")),
        pass(both, tools, None),
    ] {
        results.push(match FakeDestination.prepare(&request).await {
            Ok(prepared) => {
                let payload = prepared.projection.serialized_payload.unwrap();
                let wire: serde_json::Value = serde_json::from_str(&payload).unwrap();
                Ok(wire["purpose"].clone())
            }
            Err(refusal) => Err(refusal),
        });
    }
    assert_eq!(
        results,
        [
            Err(PreDispatchRefusal::UnsupportedRequest),
            Err(PreDispatchRefusal::UnsupportedRequest),
            Err(PreDispatchRefusal::UnsupportedRequest),
            Ok(serde_json::json!("memory_consolidation"))
        ]
    );
}
