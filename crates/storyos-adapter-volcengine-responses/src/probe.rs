//! The request body of one function calling qualification probe (ADR 0049). It sends one
//! function tool and no Provider-side tool (ADR 0084).

use serde_json::json;
use storyos_application::{CreateRequest, RequestBounds, ToolProbeRequest};

/// The Host instruction of a probe. The probe sends no project content.
const PROBE_INSTRUCTIONS: &str = "You answer one capability probe of StoryOS. Do what the request \
     asks, and use the supplied tools.";

/// The constant output of the probe function that the continuation sends.
const PROBE_FUNCTION_OUTPUT: &str = "{\"text\":\"storyos-probe\"}";

/// The canonical JSON body of one probe. It is not streamed, so that the complete response
/// object holds each native item.
pub(crate) fn probe_body(
    probe: &ToolProbeRequest,
    request: &CreateRequest,
    provider_model_id: &str,
    bounds: RequestBounds,
) -> String {
    let input = match probe {
        ToolProbeRequest::FunctionCall => json!([{
            "role": "user",
            "content": [{"type": "input_text", "text": request.author_message}],
        }]),
        ToolProbeRequest::FunctionCallOutput { call_id } => json!([{
            "type": "function_call_output",
            "call_id": call_id,
            "output": PROBE_FUNCTION_OUTPUT,
        }]),
    };
    let mut body = json!({
        "model": provider_model_id,
        "instructions": PROBE_INSTRUCTIONS,
        "input": input,
        "tools": [{
            "type": "function",
            "name": "storyos_probe_echo",
            "description": "Returns the text that it receives.",
            "parameters": {
                "type": "object",
                "properties": {"text": {"type": "string", "description": "The text to return."}},
                "required": ["text"],
                "additionalProperties": false,
            },
        }],
        "max_output_tokens": bounds.max_output_tokens,
        "stream": false,
        "store": true,
    });
    if let Some(reference) = &request.previous_response_reference {
        body["previous_response_id"] = json!(reference);
    }
    storyos_core::canonical_json(&body)
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
