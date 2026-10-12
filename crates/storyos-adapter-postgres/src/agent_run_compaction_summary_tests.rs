use storyos_application::{NoContractFaults, complete_agent_run};
use storyos_core::{ACTIVE_COMPACTION_REQUEST_PREFIX, DeploymentDestination};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::{admit_run, claim_run, queued_run, stores};

/// The run status, the refusal mark of the decision Attempt, the compaction Attempt count, and
/// the compaction record of one AgentRun.
async fn compaction_facts(admin: &Client, run_id: &str) -> [Option<String>; 6] {
    let row = admin
        .query_one(
            "SELECT run.status, decision.payload->>'compaction_summary_refused',
                    (SELECT count(*)::text FROM storyos.model_attempts AS summary
                      WHERE summary.run_id = run.run_id AND summary.attempt_role = 'compaction'),
                    record.install_state, record.producer, record.output_text
               FROM storyos.agent_runs AS run
               JOIN storyos.model_attempts AS decision
                 ON decision.run_id = run.run_id AND decision.attempt_role = 'decision'
               LEFT JOIN storyos.active_context_compactions AS record
                 ON record.run_id = run.run_id
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    [0, 1, 2, 3, 4, 5].map(|index| row.get(index))
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_summary_outside_the_pinned_grant_scope_refuses_once_and_the_run_completes() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, _) = queued_run(&store, "ec1", DeploymentDestination::HostFake).await;
    let run_id = admit_run(
        &store,
        &scope,
        &chapter_id,
        "ec2",
        DeploymentDestination::HostFake,
        ACTIVE_COMPACTION_REQUEST_PREFIX,
    )
    .await;
    // The pinned grant has the first scope, which does not name the compaction Purpose.
    admin
        .batch_execute(&format!(
            "SET session_replication_role = replica;
             UPDATE storyos.project_destination_grants
                SET grant_scope = 'project_writing_assistance_v1'
              WHERE project_id = '{project}';
             SET session_replication_role = DEFAULT;",
            project = scope.project_id.as_ref(),
        ))
        .await
        .unwrap();
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    complete_agent_run(
        &store,
        &storyos_adapter_fake_destination::FakeDestination,
        &NoContractFaults,
        &claim,
    )
    .await
    .unwrap();

    assert_eq!(
        compaction_facts(&admin, &run_id).await,
        [
            Some("completed".to_owned()),
            Some("disclosure_outside_grant".to_owned()),
            Some("0".to_owned()),
            None,
            None,
            None,
        ]
    );
}

/// Returns a test value that is not a real credential.
struct TestCredential;

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
async fn scripted_stream_endpoint(replies: Vec<Vec<serde_json::Value>>) -> String {
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
fn streamed(reference: &str, text: &str) -> Vec<serde_json::Value> {
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

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_streamed_agent_plan_summary_commits_its_events_and_stages_its_text() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let destination = DeploymentDestination::VolcengineAgentPlan {
        qualification_authorization: None,
    };
    let (scope, chapter_id, _) = queued_run(&store, "ec3", destination.clone()).await;
    let run_id = admit_run(
        &store,
        &scope,
        &chapter_id,
        "ec4",
        destination,
        ACTIVE_COMPACTION_REQUEST_PREFIX,
    )
    .await;
    let summary = "The author keeps the rain scene and the open letter.";
    let endpoint = scripted_stream_endpoint(vec![
        streamed("resp_decision", r#"{"kind":"advisory","text":"Keep it."}"#),
        streamed("resp_summary", summary),
    ])
    .await;
    admin
        .execute(
            "UPDATE storyos.processing_destination_identity_evidence_revisions
                SET endpoint = $2 WHERE project_id = $1::text::uuid AND endpoint IS NOT NULL",
            &[&scope.project_id.as_ref(), &endpoint],
        )
        .await
        .unwrap();
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    complete_agent_run(
        &store,
        &storyos_adapter_volcengine_responses::AgentPlanResponses {
            resolver: TestCredential,
        },
        &NoContractFaults,
        &claim,
    )
    .await
    .unwrap();
    let summary_attempt = admin
        .query_one(
            "SELECT payload->>'response_reference', jsonb_array_length(payload->'items'),
                    (payload->'wire'->>'serialized_payload')::jsonb->>'previous_response_id'
               FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND attempt_role = 'compaction'",
            &[&run_id],
        )
        .await
        .unwrap();

    assert_eq!(
        (
            compaction_facts(&admin, &run_id).await,
            summary_attempt.get::<_, Option<String>>(/*idx*/ 0),
            summary_attempt.get::<_, Option<i32>>(/*idx*/ 1),
            summary_attempt.get::<_, Option<String>>(/*idx*/ 2),
        ),
        (
            [
                Some("completed".to_owned()),
                None,
                Some("1".to_owned()),
                Some("installed".to_owned()),
                Some("agent_plan_summary".to_owned()),
                Some(summary.to_owned()),
            ],
            Some("resp_summary".to_owned()),
            Some(1),
            Some("resp_decision".to_owned()),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_dropped_summary_stream_keeps_its_committed_items_and_reference() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let destination = DeploymentDestination::VolcengineAgentPlan {
        qualification_authorization: None,
    };
    let (scope, chapter_id, _) = queued_run(&store, "ec5", destination.clone()).await;
    let run_id = admit_run(
        &store,
        &scope,
        &chapter_id,
        "ec6",
        destination,
        ACTIVE_COMPACTION_REQUEST_PREFIX,
    )
    .await;
    // The summary stream ends after its first delta, with no terminal event.
    let mut dropped = streamed("resp_dropped", "The author keeps the rain.");
    dropped.truncate(/*len*/ 3);
    let endpoint = scripted_stream_endpoint(vec![
        streamed("resp_decision", r#"{"kind":"advisory","text":"Keep it."}"#),
        dropped,
    ])
    .await;
    admin
        .execute(
            "UPDATE storyos.processing_destination_identity_evidence_revisions
                SET endpoint = $2 WHERE project_id = $1::text::uuid AND endpoint IS NOT NULL",
            &[&scope.project_id.as_ref(), &endpoint],
        )
        .await
        .unwrap();
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    complete_agent_run(
        &store,
        &storyos_adapter_volcengine_responses::AgentPlanResponses {
            resolver: TestCredential,
        },
        &NoContractFaults,
        &claim,
    )
    .await
    .unwrap();
    let summary_attempt = admin
        .query_one(
            "SELECT payload->>'response_reference', payload->'items'->0->>'text',
                    payload->>'observation', record.refusal_reason
               FROM storyos.model_attempts AS attempt
               JOIN storyos.active_context_compactions AS record ON record.run_id = attempt.run_id
              WHERE attempt.run_id = $1::text::uuid AND attempt.attempt_role = 'compaction'",
            &[&run_id],
        )
        .await
        .unwrap();

    assert_eq!(
        [0, 1, 2, 3].map(|index| summary_attempt.get::<_, Option<String>>(index)),
        [
            Some("resp_dropped".to_owned()),
            Some("The author keeps the rain.".to_owned()),
            Some("outcome_unknown".to_owned()),
            Some("summary_unavailable".to_owned()),
        ]
    );
}
