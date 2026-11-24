//! The shared test support from `agent_run_compaction_summary_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::model_gateway::RuntimeClock;
use crate::shared_tests::model_gateway::{Admission, claim_run, queued_run};
use crate::statement_cache::Client;
use storyos_application::{
    ConversationSelection, ModelProviderAdapter, NoContractFaults, complete_agent_run,
};
use storyos_core::DeploymentDestination;

/// Returns a test value that is not a real credential.
pub(crate) struct TestCredential;

impl storyos_application::CredentialResolver for TestCredential {
    async fn resolve(
        &self,
        _reference: &storyos_application::CredentialReference,
    ) -> Option<storyos_application::ResolvedCredential> {
        Some(storyos_application::ResolvedCredential::new(
            "storyos-test-credential".to_owned(),
        ))
    }
}

/// Answers each streamed Responses request on a local port with the next event list, in order,
/// and returns the endpoint.
pub(crate) async fn scripted_stream_endpoint(replies: Vec<Vec<serde_json::Value>>) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/api/plan/v3", listener.local_addr().unwrap());
    tokio::spawn(async move {
        for events in replies {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 4096];
            while !request
                .windows(/*size*/ 4)
                .any(|window| window == b"\r\n\r\n")
            {
                let read = stream.read(&mut chunk).await.unwrap();
                request.extend_from_slice(&chunk[..read]);
            }
            let mut body = String::from(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
            );
            for event in events {
                body.push_str(&format!("data: {event}\n\n"));
            }
            stream.write_all(body.as_bytes()).await.unwrap();
        }
    });
    endpoint
}

/// The streamed events of one complete response with one assistant message.
pub(crate) fn streamed(reference: &str, text: &str) -> Vec<serde_json::Value> {
    let message = serde_json::json!({"id": "msg_01", "type": "message", "role": "assistant",
        "status": "completed", "content": [{"type": "output_text", "text": text}]});
    vec![
        serde_json::json!({"type": "response.created",
                           "response": {"id": reference, "status": "in_progress"}}),
        serde_json::json!({"type": "response.output_item.added", "output_index": 0,
            "item": {"id": "msg_01", "type": "message", "role": "assistant",
                     "status": "in_progress", "content": []}}),
        serde_json::json!({"type": "response.output_text.delta", "output_index": 0,
                           "item_id": "msg_01", "delta": text}),
        serde_json::json!({"type": "response.output_item.done", "output_index": 0,
                           "item": message}),
        serde_json::json!({"type": "response.completed", "response": {
            "id": reference, "status": "completed", "output": [message],
            "usage": {"input_tokens": 40, "output_tokens": 12}}}),
    ]
}

pub(crate) const ADVICE: &str = "Keep it.";

/// The streamed events of one complete response with one `finish` call.
pub(crate) fn finish_events(reference: &str) -> Vec<serde_json::Value> {
    let call = serde_json::json!({"id": "fc_finish", "type": "function_call",
        "status": "completed", "call_id": format!("call_finish_{reference}"), "name": "finish",
        "arguments": "{}"});
    vec![
        serde_json::json!({"type": "response.created",
                           "response": {"id": reference, "status": "in_progress"}}),
        serde_json::json!({"type": "response.output_item.done", "output_index": 0,
                           "item": call}),
        serde_json::json!({"type": "response.completed", "response": {
            "id": reference, "status": "completed", "output": [call],
            "usage": {"input_tokens": 40, "output_tokens": 4}}}),
    ]
}

/// The events of `streamed`, with the reported usage that the terminal event gives.
pub(crate) fn with_usage(
    mut events: Vec<serde_json::Value>,
    input: u64,
    output: u64,
) -> Vec<serde_json::Value> {
    if let Some(terminal) = events.last_mut() {
        terminal["response"]["usage"] =
            serde_json::json!({"input_tokens": input, "output_tokens": output});
    }
    events
}

/// The responses of a first Run: a `finish` call, and the closing response whose reported usage
/// puts the next turn above the threshold. The threshold is (1,048,576 - 32,768) x 80 percent =
/// 812,646 tokens.
pub(crate) fn near_threshold() -> [Vec<serde_json::Value>; 2] {
    [
        finish_events("resp_finish"),
        with_usage(
            streamed("resp_decision", ADVICE),
            /*input*/ 812_500,
            /*output*/ 200,
        ),
    ]
}

/// Settles two AgentRuns of one Conversation as `compact_on_agent_plan` does. The second AgentRun
/// has the author message `message`.
pub(crate) async fn second_run_on_agent_plan(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    message: &str,
    replies: Vec<Vec<serde_json::Value>>,
) -> String {
    second_run_on_agent_plan_after(store, admin, prefixes, message, replies, "").await
}

/// Settles two AgentRuns as `second_run_on_agent_plan` does. The admin connection runs
/// `before_claim`, with `{run}` as the second AgentRun, before the claim of that AgentRun.
pub(crate) async fn second_run_on_agent_plan_after(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    message: &str,
    replies: Vec<Vec<serde_json::Value>>,
    before_claim: &str,
) -> String {
    let endpoint = scripted_stream_endpoint(replies).await;
    let destination = DeploymentDestination::VolcengineAgentPlan {
        qualification_authorization: None,
    };
    let (scope, chapter_id, first) = queued_run(store, prefixes[0], destination.clone()).await;
    admin
        .execute(
            "UPDATE storyos.processing_destination_identity_evidence_revisions
                SET endpoint = $2 WHERE project_id = $1::text::uuid AND endpoint IS NOT NULL",
            &[&scope.project_id.as_ref(), &endpoint],
        )
        .await
        .unwrap();
    let adapter = storyos_adapter_volcengine_responses::AgentPlanResponses {
        resolver: TestCredential,
    };
    let claim = claim_run(admin, scope.clone(), first).await;
    complete_agent_run(store, &adapter, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    let run_id = Admission {
        destination,
        message,
        conversation: ConversationSelection::Existing {
            conversation_id: format!("018f0000-0000-7001-8000-00000006{}6", prefixes[0]),
        },
        ..Default::default()
    }
    .admit(store, &scope, &chapter_id, prefixes[1])
    .await;
    admin
        .batch_execute(&before_claim.replace("{run}", &run_id))
        .await
        .unwrap();
    let claim = claim_run(admin, scope.clone(), run_id.clone()).await;
    complete_agent_run(store, &adapter, &NoContractFaults, &RuntimeClock, &claim)
        .await
        .unwrap();
    run_id
}

/// The Host fake, with one SQL statement that the admin connection runs at the first
/// preparation of the request that `at` selects. This is a concurrent change at that moment.
pub(crate) struct ChangingFake<'a> {
    pub(crate) admin: &'a Client,
    pub(crate) at: fn(&storyos_application::CreateRequest) -> bool,
    pub(crate) statement: String,
    pub(crate) done: std::sync::Mutex<bool>,
}

impl storyos_application::ModelProviderAdapter for ChangingFake<'_> {
    const ADAPTERS: &'static [storyos_core::ModelAdapter] = &[storyos_core::ModelAdapter::HostFake];
    type Prepared =
        <storyos_adapter_fake_destination::FakeDestination as ModelProviderAdapter>::Prepared;

    async fn prepare(
        &self,
        request: &storyos_application::DestinationRequest,
    ) -> Result<
        storyos_application::PreparedRequest<Self::Prepared>,
        storyos_application::PreDispatchRefusal,
    > {
        if let storyos_application::DestinationRequest::Create(create) = request
            && (self.at)(create)
            && !std::mem::replace(&mut *self.done.lock().unwrap(), /*src*/ true)
        {
            self.admin.batch_execute(&self.statement).await.unwrap();
        }
        storyos_adapter_fake_destination::FakeDestination
            .prepare(request)
            .await
    }

    async fn exchange(
        &self,
        prepared: Self::Prepared,
        sink: &mut impl storyos_application::ModelStreamSink,
    ) -> storyos_application::Observation {
        storyos_adapter_fake_destination::FakeDestination
            .exchange(prepared, sink)
            .await
    }
}
