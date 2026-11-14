use storyos_application::{
    CreatePurpose, CreateRequest, DestinationRequest, ModelProviderAdapter, Observation,
    PriorInput, RequestAttempt, RequestRoute, ToolOutput, ToolOutputOrigin,
};
use storyos_core::{ModelAdapter, ToolChoice, ToolExposure};

use super::{FakeDestination, FakeExchange, FakePlan, STUB_CALL_MISSING_REASON};

fn stub(call_id: &str, read_call_id: &str) -> String {
    format!(
        "<tool_result source=\"{call_id}\" state=\"ok\" form=\"stub\">The Host replaced this \
         result with a stub. To read the full result again, call read_tool_result with the \
         call_id {read_call_id}.</tool_result>"
    )
}

fn output(call_id: &str, name: &str, text: String) -> ToolOutput {
    ToolOutput {
        call_id: call_id.to_owned(),
        name: name.to_owned(),
        arguments: "{}".to_owned(),
        output: text,
        origin: ToolOutputOrigin::EarlierStep,
    }
}

/// The later request of a compaction: a full input with `tool_outputs`.
fn later_request(tool_outputs: Vec<ToolOutput>) -> DestinationRequest {
    DestinationRequest::Create(CreateRequest {
        attempt: RequestAttempt::New,
        purpose: CreatePurpose::Decision,
        route: RequestRoute {
            adapter: ModelAdapter::HostFake,
            provider_model_id: "host-fake".to_owned(),
            mapping_revision: storyos_core::HOST_FAKE_TOOL_LOOP_MAPPING_REVISION.to_owned(),
            endpoint: None,
            credential_reference: None,
            bounds: None,
        },
        author_message: "Help with this passage.".to_owned(),
        chapter_id: "chapter-1".to_owned(),
        passage_resolution: None,
        passage_input: None,
        context: Vec::new(),
        declared_targets: Vec::new(),
        selection_references: Vec::new(),
        previous_response_reference: None,
        prior_input: PriorInput::Current,
        tool_exposure: ToolExposure::empty(),
        tool_choice: ToolChoice::Required,
        successor_of: None,
        tool_outputs,
        unexecuted_call_ids: Vec::new(),
    })
}

/// The rejection reason of the prepared request, if any.
async fn rejection(request: DestinationRequest) -> Option<String> {
    let prepared = FakeDestination.prepare(&request).await.ok().unwrap();
    match prepared.prepared {
        FakeExchange(FakePlan::Observed(Observation::Rejected { reason })) => Some(reason),
        FakeExchange(_) => None,
    }
}

#[tokio::test]
async fn the_fake_refuses_a_stub_that_names_a_call_that_the_request_does_not_send() {
    let search = |text| output("call_web_search", "web_search", text);
    let read = |text| output("call_read_tool_result", "read_tool_result", text);
    let cases = [
        vec![search(stub("call_web_search", "call_web_search"))],
        vec![
            search(stub("call_web_search", "call_web_search")),
            read(stub("call_read_tool_result", "call_web_search")),
        ],
        // The stub names another call as its own.
        vec![search(stub("call_web_fetch", "call_web_fetch"))],
        // The read stub names a call that the request does not send.
        vec![read(stub("call_read_tool_result", "call_web_search"))],
    ];
    let mut reasons = Vec::new();
    for outputs in cases {
        reasons.push(rejection(later_request(outputs)).await);
    }

    assert_eq!(
        reasons,
        [
            None,
            None,
            Some(STUB_CALL_MISSING_REASON.to_owned()),
            Some(STUB_CALL_MISSING_REASON.to_owned()),
        ]
    );
}
