//! The request body of one Tool qualification probe and the native Tool facts of its response
//! (ADR 0049).

use serde_json::{Value, json};
use storyos_application::{
    CreateRequest, RequestBounds, ToolEvidence, ToolItemEvidence, ToolProbeRequest,
};

/// The Host instruction of a probe. The probe sends no project content.
const PROBE_INSTRUCTIONS: &str = "You answer one capability probe of StoryOS. Do what the request \
     asks, and use the supplied tools.";

const PROBE_FUNCTION: &str = "storyos_probe_echo";

/// The constant output of the probe function that the continuation sends.
const PROBE_FUNCTION_OUTPUT: &str = "{\"text\":\"storyos-probe\"}";

/// A keyless public MCP server that the Ark remote MCP documentation uses as its example.
const PROBE_MCP_SERVER: &str = "https://mcp.deepwiki.com/mcp";

/// The header that the remote MCP tool needs while it is in beta.
pub(crate) const MCP_BETA_HEADER: (&str, &str) = ("ark-beta-mcp", "true");

/// The canonical JSON body of one probe. It is not streamed, so that the complete response
/// object holds each native item.
pub(crate) fn probe_body(
    probe: &ToolProbeRequest,
    request: &CreateRequest,
    provider_model_id: &str,
    bounds: RequestBounds,
) -> String {
    let author_message = json!([{
        "role": "user",
        "content": [{"type": "input_text", "text": request.author_message}],
    }]);
    let echo = json!({
        "type": "function",
        "name": PROBE_FUNCTION,
        "description": "Returns the text that it receives.",
        "parameters": {
            "type": "object",
            "properties": {"text": {"type": "string", "description": "The text to return."}},
            "required": ["text"],
            "additionalProperties": false,
        },
    });
    let web_search = json!({"type": "web_search", "sources": ["doubao"]});
    let (input, tool) = match probe {
        ToolProbeRequest::FunctionCall => (author_message, echo),
        ToolProbeRequest::FunctionCallOutput { call_id } => (
            json!([{
                "type": "function_call_output",
                "call_id": call_id,
                "output": PROBE_FUNCTION_OUTPUT,
            }]),
            echo,
        ),
        ToolProbeRequest::HostedWebSearch | ToolProbeRequest::MaxToolCallsBound => {
            (author_message, web_search)
        }
        ToolProbeRequest::RemoteMcp => (
            author_message,
            json!({
                "type": "mcp",
                "server_label": "deepwiki",
                "server_url": PROBE_MCP_SERVER,
                "allowed_tools": {"tool_names": ["read_wiki_structure"]},
                "require_approval": "never",
            }),
        ),
    };
    let mut body = json!({
        "model": provider_model_id,
        "instructions": PROBE_INSTRUCTIONS,
        "input": input,
        "tools": [tool],
        "max_output_tokens": bounds.max_output_tokens,
        "stream": false,
        "store": true,
    });
    if let ToolProbeRequest::MaxToolCallsBound = probe {
        body["max_tool_calls"] = json!(1);
    }
    if let Some(reference) = &request.previous_response_reference {
        body["previous_response_id"] = json!(reference);
    }
    storyos_core::canonical_json(&body)
}

/// The native Tool facts of one Responses object, or `None` when it reports no Tool fact.
pub(crate) fn tool_evidence(response: &Value) -> Option<ToolEvidence> {
    let mut evidence = ToolEvidence::default();
    for item in response
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match item.get("type").and_then(Value::as_str).unwrap_or_default() {
            "message" => evidence.citations.extend(citations(item)),
            "reasoning" => {}
            kind => evidence.items.push(tool_item(kind, item)),
        }
    }
    evidence.tool_usage = response
        .pointer("/usage/tool_usage")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(name, count)| Some((name.clone(), count.as_u64()?)))
        .collect();
    (!evidence.items.is_empty()
        || !evidence.citations.is_empty()
        || evidence.tool_usage.iter().any(|(_, count)| *count > 0))
    .then_some(evidence)
}

fn tool_item(kind: &str, item: &Value) -> ToolItemEvidence {
    let string = |field: &str| item.get(field).and_then(Value::as_str).map(str::to_owned);
    let present = |field: &str| item.get(field).is_some_and(|value| !value.is_null());
    ToolItemEvidence {
        kind: if kind.is_empty() { "unknown" } else { kind }.to_owned(),
        item_id: string("id"),
        status: string("status"),
        call_id: string("call_id"),
        name: string("name"),
        server_label: string("server_label"),
        arguments: string("arguments").or_else(|| item.get("action").map(Value::to_string)),
        listed_tools: item
            .get("tools")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|tool| tool.get("name")?.as_str().map(str::to_owned))
            .collect(),
        output_present: present("output"),
        error: present("error").then(|| match &item["error"] {
            Value::String(error) => error.clone(),
            error => error.to_string(),
        }),
    }
}

/// The URL of each `url_citation` annotation. The Responses route puts the URL on the
/// annotation, and the Chat form nests it.
fn citations(message: &Value) -> impl Iterator<Item = String> + '_ {
    message
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|part| part.get("annotations")?.as_array())
        .flatten()
        .filter(|annotation| annotation["type"] == "url_citation")
        .filter_map(|annotation| {
            annotation
                .get("url")
                .or_else(|| annotation.pointer("/url_citation/url"))?
                .as_str()
                .map(str::to_owned)
        })
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
