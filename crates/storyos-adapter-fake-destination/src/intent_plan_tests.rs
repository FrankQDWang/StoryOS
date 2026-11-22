use storyos_application::{
    CreatePurpose, CreateRequest, DestinationRequest, ModelProviderAdapter, RequestContextItem,
};
use storyos_core::{AgentRunKind, CharacterFile, ContextSourceClass, ExposureFacts, ToolExposure};

use crate::{FAKE_INTENT_ARGUMENTS, FakeDestination, FakeExchange, FakePlan};

/// The first request of an intent pass for the file with the manuscript part `manuscript_text`.
fn intent_pass(manuscript_text: &str) -> CreateRequest {
    let file = CharacterFile {
        name: "林舟",
        author_text: None,
        manuscript_text,
    };
    let tools = ToolExposure::compute(&ExposureFacts {
        bound: &[],
        model_registration_revision: storyos_core::HOST_FAKE_TOOL_LOOP_REGISTRATION.revision,
        skill_catalog: false,
        memory_use: false,
        hybrid_search: false,
        run_kind: AgentRunKind::IntentPass,
    });
    let context = vec![RequestContextItem {
        source_class: ContextSourceClass::CharacterFile,
        content: file.text(),
        full_input_only: false,
    }];
    let DestinationRequest::Create(create) = crate::capture_refusal_tests::capture(context, tools)
    else {
        unreachable!("the helper makes a create request")
    };
    CreateRequest {
        purpose: CreatePurpose::IntentPass,
        ..create
    }
}

/// The name and the arguments of each item of the response that the fake adapter prepares for
/// `request`.
async fn calls(request: CreateRequest) -> Vec<(Option<String>, Option<String>)> {
    let request = DestinationRequest::Create(request);
    let prepared = FakeDestination.prepare(&request).await.ok().unwrap();
    let FakeExchange(FakePlan::Create(planned, _)) = prepared.prepared else {
        panic!("the fake must plan a response");
    };
    (planned.items.into_iter())
        .map(|item| (item.name, item.arguments))
        .collect()
}

/// A file with no script line gets the fixed reply: one `record_character_intent` call with the
/// fixed arguments. A script line gives its own arguments, and the line `SCRIPT:intent-no-call`
/// gives a response with no call.
#[tokio::test]
async fn the_fake_gives_its_fixed_reply_to_a_request_of_the_pass() {
    let call = |arguments: &str| {
        vec![(
            Some("record_character_intent".to_owned()),
            Some(arguments.to_owned()),
        )]
    };
    let scripted = r#"{"intent":"我去渡口。","questions":[]}"#;
    let fixed = calls(intent_pass("林舟是船夫。")).await;
    let script = format!("林舟是船夫。\nSCRIPT:intent {scripted}");
    assert_eq!(
        [fixed, calls(intent_pass(&script)).await],
        [call(FAKE_INTENT_ARGUMENTS), call(scripted)]
    );
    let no_call = calls(intent_pass("林舟是船夫。\nSCRIPT:intent-no-call")).await;
    assert!(!no_call.is_empty() && no_call.iter().all(|(name, _)| name.is_none()));
}
