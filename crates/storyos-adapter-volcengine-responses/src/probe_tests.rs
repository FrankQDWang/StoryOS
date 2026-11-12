use serde_json::json;
use storyos_application::{
    CreatePurpose, CreateRequest, DestinationRequest, ModelProviderAdapter, Observation,
    PreDispatchRefusal, RequestAttempt, RequestRoute, ToolProbeRequest,
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

    assert_eq!(
        (
            observed
                .items
                .iter()
                .map(|item| (
                    item.role,
                    item.native_type.as_deref(),
                    item.name.as_deref(),
                    item.call_id.as_deref()
                ))
                .collect::<Vec<_>>(),
            observed.output,
        ),
        (
            vec![
                (StreamItemRole::Assistant, None, None, None),
                (StreamItemRole::Tool, Some("web_search_call"), None, None),
                (
                    StreamItemRole::Tool,
                    Some("function_call"),
                    Some("storyos_probe_echo"),
                    Some("call_01")
                ),
                (StreamItemRole::Assistant, None, None, None),
            ],
            None,
        )
    );
}

/// The Host instruction of the `tool_choice` probe of #1452. It tells the finish rule of ADR 0091.
const CHOICE_INSTRUCTIONS: &str = "You answer a capability probe of StoryOS. Use the supplied \
     tools as the request asks. When your work is complete, call finish. After the result of \
     finish, write one short final answer.";

/// The probe Host refuses an add call with an argument `b` above this value.
const ADD_LIMIT: i64 = 2;

const TWO_CALLS: &str = "Call storyos_probe_echo with the text alpha, and call storyos_probe_add \
     with a = 2 and b = 3. Make the two calls in this response.";

/// The tools of one probe request: the three declarations with a `tool_choice` value, or no
/// `tools` field.
#[derive(Clone, Copy, Debug)]
enum Choice {
    Required,
    None,
    NoTools,
}

/// Discards the items of a complete response. The probe reads them from the observation.
struct Discard;

impl storyos_application::ModelStreamSink for Discard {
    async fn append(
        &mut self,
        _events: &[storyos_core::NativeStreamItem],
    ) -> storyos_application::StreamControl {
        storyos_application::StreamControl::Continue
    }

    async fn retain_reference(
        &mut self,
        _reference: &storyos_application::ResponseReference,
    ) -> storyos_application::StreamControl {
        storyos_application::StreamControl::Continue
    }
}

/// Sends the live requests of the probe through the `exchange` of the adapter and logs each one.
struct ChoiceProbe {
    credential: storyos_application::ResolvedCredential,
    sent: usize,
}

impl ChoiceProbe {
    /// Reads the key through the typed settings (ADR 0050). It needs
    /// `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`.
    fn new() -> Self {
        let loaded: storyos_settings::Loaded<std::collections::HashMap<String, String>> =
            storyos_settings::load(
                &["agent_plan_api_key"],
                storyos_settings::EnvFile::RealRoute,
            )
            .unwrap();
        let mut settings = loaded.settings;
        Self {
            credential: storyos_application::ResolvedCredential::new(
                settings.remove("agent_plan_api_key").unwrap(),
            ),
            sent: 0,
        }
    }

    async fn send(
        &mut self,
        choice: Choice,
        input: serde_json::Value,
        previous: Option<&str>,
    ) -> storyos_application::ModelResponse {
        self.sent += 1;
        let sent = self.sent;
        let bounds = storyos_core::BudgetBounds::AgentPlanRequestV3
            .request_bounds()
            .unwrap();
        let mut body = json!({
            "model": storyos_core::AGENT_PLAN_REGISTRATION.provider_model_id,
            "instructions": CHOICE_INSTRUCTIONS, "input": input,
            "max_output_tokens": bounds.max_output_tokens, "stream": false, "store": true,
        });
        let tool_choice = match choice {
            Choice::Required => Some("required"),
            Choice::None => Some("none"),
            Choice::NoTools => None,
        };
        if let Some(tool_choice) = tool_choice {
            body["tools"] = choice_tools();
            body["tool_choice"] = json!(tool_choice);
        }
        if let Some(previous) = previous {
            body["previous_response_id"] = json!(previous);
        }
        let body = storyos_core::canonical_json(&body);
        let digest = storyos_core::hex_sha256(body.as_bytes());
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let adapter = AgentPlanResponses {
            resolver: CanaryResolver::default(),
        };
        let observation = adapter
            .exchange(
                crate::AgentPlanExchange(crate::Exchange::Send {
                    url: format!("{}/responses", storyos_core::AGENT_PLAN_ENDPOINT),
                    body,
                    credential: storyos_application::ResolvedCredential::new(
                        self.credential.expose().to_owned(),
                    ),
                    timeout: bounds.timeout,
                    targets: Vec::new(),
                    continues: previous.is_some(),
                    stream: None,
                }),
                &mut Discard,
            )
            .await;
        println!("R{sent} sent at unix {at}, {choice:?}, body sha256:{digest}");
        let Observation::Terminal(response) = observation else {
            panic!("R{sent} did not complete: {observation:?}")
        };
        println!(
            "R{sent} {:?} {:?} {:?}",
            response.response_reference, response.state, response.usage
        );
        for item in &response.items {
            println!("R{sent} item {item:?}");
        }
        response
    }
}

fn choice_tools() -> serde_json::Value {
    let strict = |name: &str, description: &str, properties: serde_json::Value| {
        let required: Vec<_> = properties.as_object().unwrap().keys().cloned().collect();
        json!({"type": "function", "name": name, "description": description, "strict": true,
               "parameters": {"type": "object", "properties": properties, "required": required,
                              "additionalProperties": false}})
    };
    json!([
        strict(
            "storyos_probe_echo",
            "Returns the text that it receives.",
            json!({"text": {"type": "string", "description": "The text to return."}})
        ),
        strict(
            "storyos_probe_add",
            "Returns the sum of two integers.",
            json!({"a": {"type": "integer"}, "b": {"type": "integer"}})
        ),
        strict(
            "finish",
            "Tells StoryOS that your work is complete.",
            json!({})
        ),
    ])
}

fn author_input(text: &str) -> serde_json::Value {
    json!([{"role": "user", "content": [{"type": "input_text", "text": text}]}])
}

/// The function calls of a response.
fn calls(response: &storyos_application::ModelResponse) -> Vec<&storyos_core::NativeStreamItem> {
    response
        .items
        .iter()
        .filter(|item| item.native_type.as_deref() == Some("function_call"))
        .collect()
}

/// The output of each call, and whether the probe Host refused a call. An echo or a finish call
/// gets an empty output.
fn choice_outputs(calls: &[&storyos_core::NativeStreamItem]) -> (serde_json::Value, bool) {
    let mut refused = false;
    let outputs = calls
        .iter()
        .map(|item| {
            let arguments: serde_json::Value =
                serde_json::from_str(item.arguments.as_deref().unwrap_or("{}")).unwrap_or_default();
            let output = match (
                item.name.as_deref(),
                arguments["a"].as_i64(),
                arguments["b"].as_i64(),
            ) {
                (Some("storyos_probe_add"), Some(a), Some(b)) if b <= ADD_LIMIT => {
                    (a + b).to_string()
                }
                (Some("storyos_probe_add"), _, _) => {
                    refused = true;
                    format!(
                        "Refused: the argument b must be {ADD_LIMIT} or less. Correct the call."
                    )
                }
                _ => String::new(),
            };
            json!({"type": "function_call_output", "call_id": item.call_id, "output": output})
        })
        .collect();
    (serde_json::Value::Array(outputs), refused)
}

/// R1 to R7 of the plan on #1452: at most 7 live requests. The finish rule of ADR 0091 section 4
/// with enforcement in the request decides the `tool_choice` value of each continuation.
#[tokio::test]
#[ignore = "live requests of the tool_choice probe of #1452; run only with a written plan"]
async fn live_tool_choice_probe_of_the_agent_plan_route() {
    let mut probe = ChoiceProbe::new();
    // R1 and R2: a fresh request whose author message asks against the tool_choice value.
    let no_call = author_input("Reply with the word hello. Do not call a tool.");
    probe.send(Choice::Required, no_call, None).await;
    let call = author_input("Call storyos_probe_echo with the text beta.");
    probe.send(Choice::None, call, None).await;
    // R3: three declarations and two calls asked. R4 to R7: at most four continuations.
    let mut response = probe
        .send(Choice::Required, author_input(TWO_CALLS), None)
        .await;
    for _continuation in 0..4 {
        let calls = calls(&response);
        if calls.is_empty() {
            break;
        }
        let (outputs, refused) = choice_outputs(&calls);
        let finished = !refused
            && calls
                .iter()
                .any(|item| item.name.as_deref() == Some("finish"));
        let choice = if finished {
            Choice::None
        } else {
            Choice::Required
        };
        let previous = response.response_reference.clone().unwrap();
        response = probe.send(choice, outputs, Some(&previous)).await;
    }
}

/// R7 to R9 of the amendment on #1452: 3 live requests. A request with no `tools` field, fresh
/// and as the continuation of a response with a refused call.
#[tokio::test]
#[ignore = "live requests of the tool_choice probe of #1452; run only with a written plan"]
async fn live_no_tools_probe_of_the_agent_plan_route() {
    let mut probe = ChoiceProbe::new();
    // The labels continue after R6 of the first test.
    probe.sent = 6;
    let call = author_input("Call storyos_probe_echo with the text beta.");
    probe.send(Choice::NoTools, call, None).await;
    let response = probe
        .send(Choice::Required, author_input(TWO_CALLS), None)
        .await;
    let (outputs, _refused) = choice_outputs(&calls(&response));
    let previous = response.response_reference.clone().unwrap();
    probe.send(Choice::NoTools, outputs, Some(&previous)).await;
}
