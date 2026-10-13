use storyos_adapter_volcengine_responses::{AgentPlanExchange, AgentPlanResponses};
use storyos_application::{
    CompleteAgentRunError, CredentialReference, CredentialResolver, DestinationRequest,
    ModelProviderAdapter, ModelStreamSink, NoContractFaults, Observation, PreDispatchRefusal,
    PreparedRequest, ResolvedCredential, complete_agent_run,
};
use storyos_core::{DeploymentDestination, ModelAdapter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::{claim_run, queued_run, stores};

const ADVISORY: &str = r#"{\"kind\":\"advisory\",\"text\":\"Keep it.\"}"#;

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

/// Admits one Agent Plan AgentRun on the qualified head Registration and sends it to `base`.
pub(crate) async fn real_run(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    prefix: &str,
    base: &str,
) -> (storyos_application::ProjectScope, String) {
    let (scope, _, run_id) = queued_run(
        store,
        prefix,
        DeploymentDestination::VolcengineAgentPlan {
            qualification_authorization: None,
        },
    )
    .await;
    admin
        .execute(
            "UPDATE storyos.processing_destination_identity_evidence_revisions
                SET endpoint = $2 WHERE project_id = $1::text::uuid AND endpoint IS NOT NULL",
            &[&scope.project_id.as_ref(), &base],
        )
        .await
        .unwrap();
    (scope, run_id)
}

/// The status, settlement, and decision of the Run, and the retrieval facts of its Attempts.
pub(crate) async fn recovery(admin: &Client, run_id: &str) -> [Option<String>; 7] {
    let row = admin
        .query_one(
            "SELECT run.status, run.settlement::text,
                    decision.payload->'decision'->>'text',
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
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    [0, 1, 2, 3, 4, 5, 6].map(|index| row.get(index))
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_restarted_worker_retrieves_the_committed_reference_once_and_settles_the_result() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![
        broken_stream("resp_c41"),
        stored("resp_c41", "completed"),
    ])
    .await;
    let (scope, run_id) = real_run(&store, &admin, "c41", &base).await;
    let crash = AfterExchange {
        admin: &admin,
        statement: format!(
            "UPDATE storyos.agent_runs SET fence_token = fence_token + 1 WHERE run_id = '{run_id}'"
        ),
    };
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let crashed = complete_agent_run(&store, &crash, &NoContractFaults, &claim).await;
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &claim)
        .await
        .unwrap();

    assert_eq!(
        (
            crashed
                .err()
                .map(|error| matches!(error, CompleteAgentRunError::StaleFence)),
            recovery(&admin, &run_id).await,
            served.await.unwrap(),
        ),
        (
            Some(true),
            [
                Some("completed".to_owned()),
                None,
                Some("Keep it.".to_owned()),
                Some("settled".to_owned()),
                None,
                Some("true".to_owned()),
                Some("decision:true,retrieval:true".to_owned()),
            ],
            vec![
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
                "GET /api/plan/v3/responses/resp_c41 HTTP/1.1".to_owned(),
            ],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_route_that_is_no_longer_admitted_keeps_the_result_unknown_and_sends_no_retrieve() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![broken_stream("resp_c44")]).await;
    let (scope, run_id) = real_run(&store, &admin, "c44", &base).await;
    let off = AfterExchange {
        admin: &admin,
        statement: format!(
            "SET session_replication_role = replica;
             UPDATE storyos.project_policy_revisions SET availability = 'unavailable'
              WHERE project_id = '{}';
             SET session_replication_role = DEFAULT;",
            scope.project_id.as_ref()
        ),
    };
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    complete_agent_run(&store, &off, &NoContractFaults, &claim)
        .await
        .unwrap();

    assert_eq!(
        (recovery(&admin, &run_id).await, served.await.unwrap()),
        (
            [
                // The successor gate then pauses the Run, because assistance is off.
                Some("paused".to_owned()),
                Some(
                    r#"{"kind": "unknown_create_successor", "reason": "authority_unavailable"}"#
                        .to_owned()
                ),
                None,
                Some("kept_unknown".to_owned()),
                Some("destination_mismatch".to_owned()),
                Some("false".to_owned()),
                Some("decision:true".to_owned()),
            ],
            vec!["POST /api/plan/v3/responses HTTP/1.1".to_owned()],
        )
    );
}
