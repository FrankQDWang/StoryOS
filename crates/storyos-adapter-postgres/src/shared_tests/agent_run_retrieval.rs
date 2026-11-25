//! The shared test support from `agent_run_retrieval_tests.rs`.

use crate::shared_tests::model_gateway::queued_run_offering;
use crate::statement_cache::Client;
use storyos_adapter_volcengine_responses::{AgentPlanExchange, AgentPlanResponses};
use storyos_application::{
    CredentialReference, CredentialResolver, DestinationRequest, ModelProviderAdapter,
    ModelStreamSink, Observation, PreDispatchRefusal, PreparedRequest, ResolvedCredential,
};
use storyos_core::{DeploymentDestination, ModelAdapter, RESEARCH_TOOL_OFFER, ResearchToolOffer};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(crate) const ADVISORY: &str = r#"{\"kind\":\"advisory\",\"text\":\"Keep it.\"}"#;

pub(crate) struct TestCredential;

impl CredentialResolver for TestCredential {
    async fn resolve(&self, _reference: &CredentialReference) -> Option<ResolvedCredential> {
        Some(ResolvedCredential::new(
            "storyos-test-credential".to_owned(),
        ))
    }
}

pub(crate) fn adapter() -> AgentPlanResponses<TestCredential> {
    AgentPlanResponses {
        resolver: TestCredential,
    }
}

/// The real adapter. After its exchange, `statement` changes the disposable database, for
/// example as a crash and the expiry of the lease do.
pub(crate) struct AfterExchange<'a> {
    pub admin: &'a Client,
    pub statement: String,
}

impl ModelProviderAdapter for AfterExchange<'_> {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::VolcengineAgentPlanResponses];
    type Prepared = AgentPlanExchange;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<AgentPlanExchange>, PreDispatchRefusal> {
        adapter().prepare(request).await
    }

    async fn exchange(
        &self,
        prepared: AgentPlanExchange,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        let observation = adapter().exchange(prepared, sink).await;
        self.admin.batch_execute(&self.statement).await.unwrap();
        observation
    }
}

/// Serves one connection for each reply, in order. Returns the base URL and the request lines.
pub(crate) async fn destination(
    replies: Vec<String>,
) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/api/plan/v3", listener.local_addr().unwrap());
    let served = tokio::spawn(async move {
        let mut lines = Vec::new();
        for reply in replies {
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
            let text = String::from_utf8_lossy(&request).into_owned();
            lines.push(text.lines().next().unwrap_or_default().to_owned());
            stream.write_all(reply.as_bytes()).await.unwrap();
        }
        lines
    });
    (base, served)
}

/// A stream that reports `reference` and part of the text, and then ends with no terminal event.
pub(crate) fn broken_stream(reference: &str) -> String {
    let events = [
        format!(
            r#"{{"type":"response.created","response":{{"id":"{reference}","status":"in_progress","output":[]}}}}"#
        ),
        r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"msg_01","type":"message","role":"assistant","status":"in_progress","content":[]}}"#.to_owned(),
        r#"{"type":"response.output_text.delta","output_index":0,"delta":"{\"kind\""}"#.to_owned(),
    ];
    let mut reply = String::from(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
    );
    for event in events {
        reply.push_str(&format!("data: {event}\n\n"));
    }
    reply
}

/// The stored response that a Retrieve reads, with `status`.
pub(crate) fn stored(reference: &str, status: &str) -> String {
    let body = format!(
        r#"{{"id":"{reference}","status":"{status}","output":[{{"id":"msg_01","type":"message","role":"assistant","status":"{status}","content":[{{"type":"output_text","text":"{ADVISORY}"}}]}}],"usage":{{"input_tokens":40,"output_tokens":12}}}}"#
    );
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// The answer of the route to a Retrieve of a stream that broke before its terminal event
/// (ADR 0057).
pub(crate) fn not_found() -> String {
    let body = r#"{"error":{"code":"ResourceNotFound","message":"The response is not found."}}"#;
    format!(
        "HTTP/1.1 404 Not Found\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\
         connection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Admits one Agent Plan AgentRun on the qualified head Registration and sends it to `base`,
/// in a deployment that offers no research destination.
pub(crate) async fn real_run(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    base: &str,
) -> (storyos_application::ProjectScope, String) {
    agent_plan_run(store, admin, prefix, base, None).await
}

/// Admits one Agent Plan AgentRun as `real_run` does, in a deployment that also offers the
/// research destination. The first Run of the Project binds it (ADR 0090).
pub(crate) async fn research_run(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    base: &str,
) -> (storyos_application::ProjectScope, String) {
    agent_plan_run(store, admin, prefix, base, Some(&RESEARCH_TOOL_OFFER)).await
}

pub(crate) async fn agent_plan_run(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    base: &str,
    research: Option<&'static ResearchToolOffer>,
) -> (storyos_application::ProjectScope, String) {
    let (scope, _, run_id) = queued_run_offering(
        store,
        prefix,
        DeploymentDestination::VolcengineAgentPlan {
            qualification_authorization: None,
        },
        research,
    )
    .await;
    admin
        .execute(
            "UPDATE storyos.processing_destination_identity_evidence_revisions
                SET endpoint = $2
              WHERE project_id = $1::text::uuid AND evidence_kind = 'volcengine_agent_plan_boundary'",
            &[&scope.project_id.as_ref(), &base],
        )
        .await
        .unwrap();
    (scope, run_id)
}

/// The status, settlement, and final answer of the Run, and the retrieval facts of its first
/// RunStep and of its Attempts.
pub(crate) async fn recovery(admin: &Client, run_id: &str) -> [Option<String>; 7] {
    let row = admin
        .query_one(
            "SELECT run.status, run.settlement::text,
                    (SELECT last.payload->'decision'->>'text' FROM storyos.model_attempts AS last
                      WHERE last.run_id = run.run_id AND last.attempt_role = 'decision'
                      ORDER BY last.decision_position DESC LIMIT 1),
                    decision.payload->'original_result_retrieval'->>'disposition',
                    decision.payload->'original_result_retrieval'->>'keep_reason',
                    decision.payload->'reservation'->>'released',
                    (SELECT string_agg(attempt.attempt_role || ':' ||
                                       (attempt.outbound_disclosure_event_id IS NOT NULL)::text,
                                       ',' ORDER BY attempt.model_attempt_id)
                       FROM storyos.model_attempts AS attempt
                      WHERE attempt.run_id = run.run_id)
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS decision
                 ON decision.run_id = run.run_id AND decision.attempt_role = 'decision'
                AND decision.decision_position = 0
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    [0, 1, 2, 3, 4, 5, 6].map(|index| row.get(index))
}
