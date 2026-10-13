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

struct TestCredential;

impl CredentialResolver for TestCredential {
    async fn resolve(&self, _reference: &CredentialReference) -> Option<ResolvedCredential> {
        Some(ResolvedCredential::new(
            "storyos-test-credential".to_owned(),
        ))
    }
}

fn adapter() -> AgentPlanResponses<TestCredential> {
    AgentPlanResponses {
        resolver: TestCredential,
    }
}

/// The real adapter. After its exchange, the Worker stops before it records the observation, as
/// a crash and the expiry of its lease do.
struct CrashAfterExchange<'a> {
    admin: &'a Client,
    run_id: String,
}

impl ModelProviderAdapter for CrashAfterExchange<'_> {
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
        self.admin
            .execute(
                "UPDATE storyos.agent_runs SET fence_token = fence_token + 1
                  WHERE run_id = $1::text::uuid",
                &[&self.run_id],
            )
            .await
            .unwrap();
        observation
    }
}

/// Serves one connection for each reply, in order. Returns the base URL and the request lines.
async fn destination(replies: Vec<String>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
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
fn broken_stream(reference: &str) -> String {
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
fn stored(reference: &str, status: &str) -> String {
    let body = format!(
        r#"{{"id":"{reference}","status":"{status}","output":[{{"id":"msg_01","type":"message","role":"assistant","status":"{status}","content":[{{"type":"output_text","text":"{ADVISORY}"}}]}}],"usage":{{"input_tokens":40,"output_tokens":12}}}}"#
    );
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Admits one Agent Plan AgentRun on the qualified head Registration and sends it to `base`.
async fn real_run(
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
async fn recovery(admin: &Client, run_id: &str) -> [Option<String>; 7] {
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
    let crash = CrashAfterExchange {
        admin: &admin,
        run_id: run_id.clone(),
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
async fn a_dropped_stream_retrieves_once_and_keeps_an_unfinished_result_unknown() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![
        broken_stream("resp_c42"),
        stored("resp_c42", "in_progress"),
    ])
    .await;
    let (scope, run_id) = real_run(&store, &admin, "c42", &base).await;
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &claim)
        .await
        .unwrap();

    assert_eq!(
        (recovery(&admin, &run_id).await, served.await.unwrap()),
        (
            [
                Some("waiting".to_owned()),
                Some(r#"{"kind": "outcome_unknown", "reason": "unknown_result"}"#.to_owned()),
                None,
                Some("kept_unknown".to_owned()),
                Some("unknown_result".to_owned()),
                Some("false".to_owned()),
                Some("decision:true,retrieval:true".to_owned()),
            ],
            vec![
                "POST /api/plan/v3/responses HTTP/1.1".to_owned(),
                "GET /api/plan/v3/responses/resp_c42 HTTP/1.1".to_owned(),
            ],
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_registration_without_qualified_retrieval_keeps_the_reference_and_sends_no_retrieve() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (base, served) = destination(vec![broken_stream("resp_c43")]).await;
    let (scope, run_id) = real_run(&store, &admin, "c43", &base).await;
    let crash = CrashAfterExchange {
        admin: &admin,
        run_id: run_id.clone(),
    };
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let _crashed = complete_agent_run(&store, &crash, &NoContractFaults, &claim).await;
    // The Run pins the Registration before retrieval qualified.
    let earlier = &storyos_core::AGENT_PLAN_THRESHOLD_REGISTRATION;
    admin
        .batch_execute(&format!(
            "INSERT INTO storyos.model_capability_profiles (capability_profile_revision, profile)
             VALUES ('{profile_revision}', '{profile}'::jsonb) ON CONFLICT DO NOTHING;
             INSERT INTO storyos.model_registration_revisions
               (model_registration_revision, model_kind, api_surface, provider_model_id,
                capability_profile_revision)
             VALUES ('{revision}', 'volcengine_agent_plan_responses', '{surface}',
                     'doubao-seed-2.1-pro', '{profile_revision}') ON CONFLICT DO NOTHING;
             SET session_replication_role = replica;
             UPDATE storyos.agent_runs SET model_registration_revision = '{revision}'
              WHERE run_id = '{run_id}';
             SET session_replication_role = DEFAULT;",
            profile_revision = earlier.capability_profile.revision,
            profile = serde_json::to_string(earlier.capability_profile).unwrap(),
            revision = earlier.revision,
            surface = earlier.api_surface,
        ))
        .await
        .unwrap();
    let claim = claim_run(&admin, scope, run_id.clone()).await;
    complete_agent_run(&store, &adapter(), &NoContractFaults, &claim)
        .await
        .unwrap();
    let reference: Option<String> = admin
        .query_one(
            "SELECT payload->'original_result_retrieval'->>'reference_id'
               FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(0);

    assert_eq!(
        (
            recovery(&admin, &run_id).await,
            reference,
            served.await.unwrap()
        ),
        (
            [
                Some("waiting".to_owned()),
                Some(
                    r#"{"kind": "outcome_unknown", "reason": "unsupported_retrieval"}"#.to_owned()
                ),
                None,
                Some("kept_unknown".to_owned()),
                Some("unsupported_retrieval".to_owned()),
                Some("false".to_owned()),
                Some("decision:true".to_owned()),
            ],
            Some("resp_c43".to_owned()),
            vec!["POST /api/plan/v3/responses HTTP/1.1".to_owned()],
        )
    );
}
