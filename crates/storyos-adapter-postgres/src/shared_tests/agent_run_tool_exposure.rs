//! The shared test support from `agent_run_tool_exposure_tests.rs`.

use crate::shared_tests::agent_run_retrieval::adapter;
use crate::statement_cache::Client;
use serde_json::json;
use storyos_adapter_volcengine_responses::AgentPlanExchange;
use storyos_application::{
    DestinationRequest, ModelProviderAdapter, ModelStreamSink, Observation, PreDispatchRefusal,
    PreparedRequest,
};
use storyos_core::{ExposureFacts, ModelAdapter, ToolExposure};

/// The final answer of a closing response.
pub(crate) const ADVISORY: &str = "Keep it.";

pub(crate) const ARGUMENTS: &str = r#"{"query":"tide tables of 1890","result_count":null}"#;

pub(crate) fn sse(events: &[serde_json::Value]) -> String {
    let mut reply = String::from(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
    );
    for event in events {
        reply.push_str(&format!("data: {event}\n\n"));
    }
    reply
}

/// A stream that reports `reference` and ends with one complete final answer.
pub(crate) fn advisory_stream(reference: &str) -> String {
    let message = json!({"id": "msg_01", "type": "message", "role": "assistant",
        "status": "completed", "content": [{"type": "output_text", "text": ADVISORY}]});
    sse(&[
        json!({"type": "response.created",
               "response": {"id": reference, "status": "in_progress", "output": []}}),
        json!({"type": "response.output_item.done", "output_index": 0, "item": message}),
        json!({"type": "response.completed", "response": {
            "id": reference, "status": "completed", "output": [message],
            "usage": {"input_tokens": 40, "output_tokens": 12}}}),
    ])
}

/// A stream that reports `reference` and ends with one complete `finish` call.
pub(crate) fn finish_stream(reference: &str) -> String {
    let call = json!({"id": "fc_finish", "type": "function_call", "status": "completed",
        "call_id": format!("call_finish_{reference}"), "name": "finish", "arguments": "{}"});
    sse(&[
        json!({"type": "response.created",
               "response": {"id": reference, "status": "in_progress", "output": []}}),
        json!({"type": "response.output_item.done", "output_index": 0, "item": call}),
        json!({"type": "response.completed", "response": {
            "id": reference, "status": "completed", "output": [call],
            "usage": {"input_tokens": 40, "output_tokens": 4}}}),
    ])
}

/// The two streams of a model that ends its Run: a `finish` call in `reference`, then the
/// closing Advisory in `reference` with `_closing`.
pub(crate) fn finish_then_advisory(reference: &str) -> [String; 2] {
    [
        finish_stream(reference),
        advisory_stream(&format!("{reference}_closing")),
    ]
}

/// One function call item with `arguments` and `status`.
pub(crate) fn call(arguments: &str, status: &str) -> serde_json::Value {
    json!({"id": "fc_01", "type": "function_call", "status": status, "call_id": "call_01",
           "name": "web_search", "arguments": arguments})
}

/// A stream that reports `reference`, streams the arguments of one function call, and ends
/// with the complete call.
pub(crate) fn function_call_stream(reference: &str) -> String {
    sse(&[
        json!({"type": "response.created",
               "response": {"id": reference, "status": "in_progress", "output": []}}),
        json!({"type": "response.output_item.added", "output_index": 0,
               "item": call("", "in_progress")}),
        json!({"type": "response.function_call_arguments.delta", "output_index": 0,
               "item_id": "fc_01", "delta": &ARGUMENTS[..20]}),
        json!({"type": "response.function_call_arguments.delta", "output_index": 0,
               "item_id": "fc_01", "delta": &ARGUMENTS[20..]}),
        json!({"type": "response.output_item.done", "output_index": 0,
               "item": call(ARGUMENTS, "completed")}),
        json!({"type": "response.completed", "response": {
            "id": reference, "status": "completed", "output": [call(ARGUMENTS, "completed")],
            "usage": {"input_tokens": 90, "output_tokens": 30}}}),
    ])
}

/// The Exposure of the Host Tools alone on the current Agent Plan Registration, for a Run with
/// Memory use.
pub(crate) fn host_exposure() -> ToolExposure {
    ToolExposure::compute(&ExposureFacts {
        bound: &[],
        model_registration_revision: storyos_core::AGENT_PLAN_REGISTRATION.revision,
        skill_catalog: false,
        memory_use: true,
        hybrid_search: false,
        run_kind: storyos_core::AgentRunKind::Author,
    })
}

/// The real adapter. Before the claim of the first Create that is a successor when `successor`
/// is set, and that is not a successor when it is not, a Tool Contract Drift quarantines the
/// research revision. Each prepared Create keeps its Exposure Snapshot and the `tools` field of
/// its body.
pub(crate) struct QuarantineBeforeClaim<'a> {
    pub admin: &'a Client,
    pub successor: bool,
    pub quarantined: std::sync::atomic::AtomicBool,
    pub prepared: std::sync::Mutex<Vec<(serde_json::Value, Option<serde_json::Value>)>>,
}

impl ModelProviderAdapter for QuarantineBeforeClaim<'_> {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = AgentPlanExchange;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<AgentPlanExchange>, PreDispatchRefusal> {
        let prepared = adapter().prepare(request).await?;
        if let DestinationRequest::Create(create) = request {
            let body: serde_json::Value =
                serde_json::from_str(prepared.projection.serialized_payload.as_deref().unwrap())
                    .unwrap();
            self.prepared
                .lock()
                .unwrap()
                .push((create.tool_exposure.snapshot(), body.get("tools").cloned()));
            if create.successor_of.is_some() == self.successor
                && !self
                    .quarantined
                    .swap(/*val*/ true, std::sync::atomic::Ordering::Relaxed)
            {
                self.admin
                    .batch_execute(
                        &crate::shared_tests::agent_run_tool_call::quarantine_statement(),
                    )
                    .await
                    .unwrap();
            }
        }
        Ok(prepared)
    }

    async fn exchange(
        &self,
        prepared: AgentPlanExchange,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        adapter().exchange(prepared, sink).await
    }
}
