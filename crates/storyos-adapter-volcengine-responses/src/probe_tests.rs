use serde_json::json;
use storyos_application::{
    CreatePurpose, CreateRequest, DestinationRequest, ModelProviderAdapter, Observation,
    PreDispatchRefusal, RequestAttempt, RequestRoute, ToolItemEvidence, ToolProbeRequest,
};
use storyos_core::StreamItemRole;

use crate::AgentPlanResponses;
use crate::tests::{CanaryResolver, create, message, route};

#[tokio::test]
async fn a_probe_sends_one_function_tool_and_constant_input_and_no_project_content() {
    let adapter = AgentPlanResponses {
        resolver: CanaryResolver::default(),
    };
    let route = RequestRoute {
        mapping_revision: storyos_core::AGENT_PLAN_COMPACTION_MAPPING_REVISION.to_owned(),
        ..route("http://127.0.0.1:9")
    };
    let DestinationRequest::Create(request) = create(route, RequestAttempt::New) else {
        unreachable!("create() makes a Create request")
    };
    let mut sent = Vec::new();
    for (probe, previous_response_reference) in [
        (ToolProbeRequest::FunctionCall, None),
        (
            ToolProbeRequest::FunctionCallOutput {
                call_id: Some("call_01".to_owned()),
            },
            Some("resp_01".to_owned()),
        ),
    ] {
        let prepared = adapter
            .prepare(&DestinationRequest::Create(CreateRequest {
                purpose: CreatePurpose::ToolQualification(probe),
                author_message: "StoryOS Tool probe".to_owned(),
                previous_response_reference,
                ..request.clone()
            }))
            .await
            .ok()
            .unwrap();
        let body = prepared.projection.serialized_payload.unwrap();
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        sent.push((
            value["tools"]
                .as_array()
                .unwrap()
                .iter()
                .map(|tool| (tool["type"].clone(), tool["name"].clone()))
                .collect::<Vec<_>>(),
            value["input"].clone(),
            value["stream"].clone(),
            value.get("previous_response_id").cloned(),
            [
                "The rain fell hard",
                "web_search",
                "\"mcp\"",
                "max_tool_calls",
            ]
            .iter()
            .any(|text| body.contains(text)),
        ));
    }
    let echo = vec![(json!("function"), json!("storyos_probe_echo"))];
    // A function output probe without the call of an earlier probe sends nothing.
    let uncalled = adapter
        .prepare(&DestinationRequest::Create(CreateRequest {
            purpose: CreatePurpose::ToolQualification(ToolProbeRequest::FunctionCallOutput {
                call_id: None,
            }),
            ..request
        }))
        .await
        .err();

    assert_eq!(uncalled, Some(PreDispatchRefusal::UnsupportedRequest));
    assert_eq!(
        sent,
        [
            (
                echo.clone(),
                json!([{"role": "user", "content": [
                    {"type": "input_text", "text": "StoryOS Tool probe"}
                ]}]),
                json!(false),
                None,
                false
            ),
            (
                echo,
                json!([{"type": "function_call_output", "call_id": "call_01",
                        "output": "{\"text\":\"storyos-probe\"}"}]),
                json!(false),
                Some(json!("resp_01")),
                false,
            ),
        ]
    );
}

#[test]
fn a_native_item_that_the_request_did_not_ask_for_is_a_tool_item_with_no_decision() {
    let response = json!({
        "id": "resp_01",
        "status": "completed",
        "output": [
            {"id": "rs_01", "type": "reasoning", "summary": []},
            {"id": "ws_01", "type": "web_search_call", "status": "completed",
             "action": {"type": "search", "query": "Rust 1.0"}},
            {"id": "fc_01", "type": "function_call", "status": "completed", "call_id": "call_01",
             "name": "storyos_probe_echo", "arguments": "{\"text\":\"storyos-probe\"}"},
            message("2015-05-15")
        ],
    });
    let Observation::Terminal(observed) = crate::response::observe_value(&response, &[]) else {
        unreachable!("a completed response is terminal")
    };
    let item = |kind: &str, item_id: &str| ToolItemEvidence {
        kind: kind.to_owned(),
        item_id: Some(item_id.to_owned()),
        status: Some("completed".to_owned()),
        call_id: None,
        name: None,
        arguments: None,
    };

    assert_eq!(
        (
            observed
                .items
                .iter()
                .map(|item| item.role)
                .collect::<Vec<_>>(),
            observed.output,
            observed.tool_items,
        ),
        (
            vec![
                StreamItemRole::Assistant,
                StreamItemRole::Tool,
                StreamItemRole::Tool,
                StreamItemRole::Assistant,
            ],
            None,
            vec![
                item("web_search_call", "ws_01"),
                ToolItemEvidence {
                    call_id: Some("call_01".to_owned()),
                    name: Some("storyos_probe_echo".to_owned()),
                    arguments: Some("{\"text\":\"storyos-probe\"}".to_owned()),
                    ..item("function_call", "fc_01")
                },
            ],
        )
    );
}
