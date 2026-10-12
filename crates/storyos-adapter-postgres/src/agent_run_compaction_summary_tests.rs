use storyos_application::{
    ConversationSelection, ModelProviderAdapter, NoContractFaults, ProjectScope, complete_agent_run,
};
use storyos_core::{ACTIVE_COMPACTION_REQUEST_PREFIX, DeploymentDestination};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{admit_run_in, claim_run, queued_run, stores};

/// Settles the first AgentRun of a new Conversation, then settles one compaction request in the
/// same Conversation, and returns the identity of the second AgentRun.
async fn compact_after_first_run(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    destination: DeploymentDestination,
    adapter: &impl ModelProviderAdapter,
    before_compaction: &str,
) -> (ProjectScope, String) {
    let (scope, chapter_id, first) = queued_run(store, prefixes[0], destination.clone()).await;
    let claim = claim_run(admin, scope.clone(), first).await;
    complete_agent_run(store, adapter, &NoContractFaults, &claim)
        .await
        .unwrap();
    let run_id = admit_run_in(
        store,
        &scope,
        &chapter_id,
        prefixes[1],
        destination,
        ACTIVE_COMPACTION_REQUEST_PREFIX,
        ConversationSelection::Existing {
            conversation_id: format!("018f0000-0000-7001-8000-00000006{}6", prefixes[0]),
        },
    )
    .await;
    admin
        .batch_execute(&before_compaction.replace("{project}", scope.project_id.as_ref()))
        .await
        .unwrap();
    let claim = claim_run(admin, scope.clone(), run_id.clone()).await;
    complete_agent_run(store, adapter, &NoContractFaults, &claim)
        .await
        .unwrap();
    (scope, run_id)
}

/// The run status, the input mapping of the decision Attempt, the compaction Attempt count, and
/// the compaction record of one AgentRun.
async fn compaction_facts(admin: &Client, run_id: &str) -> [Option<String>; 6] {
    let row = admin
        .query_one(
            "SELECT run.status, decision.payload->'continuation'->>'input_mapping',
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
async fn a_summary_outside_the_pinned_grant_scope_leaves_the_ordinary_continuation() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // The pinned grant has the first scope, which does not name the compaction Purpose.
    let (_, run_id) = compact_after_first_run(
        &store,
        &admin,
        ["ec1", "ec2"],
        DeploymentDestination::HostFake,
        &storyos_adapter_fake_destination::FakeDestination,
        "SET session_replication_role = replica;
         UPDATE storyos.project_destination_grants
            SET grant_scope = 'project_writing_assistance_v1'
          WHERE project_id = '{project}';
         SET session_replication_role = DEFAULT;",
    )
    .await;

    assert_eq!(
        compaction_facts(&admin, &run_id).await,
        [
            Some("completed".to_owned()),
            Some("incremental".to_owned()),
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

const ADVICE: &str = r#"{"kind":"advisory","text":"Keep it."}"#;

/// Settles the first AgentRun of a new Conversation on a local scripted Agent Plan destination
/// that answers with `replies` in order. Then settles one compaction request in the same
/// Conversation, and returns the identity of the second AgentRun.
async fn compact_on_agent_plan(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    replies: Vec<Vec<serde_json::Value>>,
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
    complete_agent_run(store, &adapter, &NoContractFaults, &claim)
        .await
        .unwrap();
    let run_id = admit_run_in(
        store,
        &scope,
        &chapter_id,
        prefixes[1],
        destination,
        ACTIVE_COMPACTION_REQUEST_PREFIX,
        ConversationSelection::Existing {
            conversation_id: format!("018f0000-0000-7001-8000-00000006{}6", prefixes[0]),
        },
    )
    .await;
    let claim = claim_run(admin, scope.clone(), run_id.clone()).await;
    complete_agent_run(store, &adapter, &NoContractFaults, &claim)
        .await
        .unwrap();
    run_id
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_streamed_agent_plan_summary_installs_for_the_later_decision_request() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let summary = "The author keeps the rain scene and the open letter.";
    let run_id = compact_on_agent_plan(
        &store,
        &admin,
        ["ec3", "ec4"],
        vec![
            streamed("resp_decision", ADVICE),
            streamed("resp_summary", summary),
            streamed("resp_later", ADVICE),
        ],
    )
    .await;
    let later = admin
        .query_one(
            "SELECT summary.payload->>'response_reference',
                    (summary.payload->'wire'->>'serialized_payload')::jsonb
                      ->>'previous_response_id',
                    (decision.payload->'wire'->>'serialized_payload')::jsonb
                      ? 'previous_response_id',
                    strpos((decision.payload->'wire'->>'serialized_payload')::jsonb
                             #>> '{input,0,content,0,text}',
                           'Summary of the earlier conversation:') = 1,
                    record.installed_model_attempt_id = decision.model_attempt_id
                      AND record.installed_model_invocation_id = decision.model_invocation_id
                      AND record.installed_run_step_id IS NOT NULL,
                    assembly.manifest_role, assembly.payload->'selected'->0->>'projection_mode'
               FROM storyos.model_attempts AS summary
               JOIN storyos.model_attempts AS decision
                 ON decision.run_id = summary.run_id AND decision.attempt_role = 'decision'
               JOIN storyos.active_context_compactions AS record
                 ON record.run_id = summary.run_id
               JOIN storyos.context_assembly_manifests AS assembly
                 ON assembly.destination_context_manifest_id =
                    decision.destination_context_manifest_id
              WHERE summary.run_id = $1::text::uuid AND summary.attempt_role = 'compaction'",
            &[&run_id],
        )
        .await
        .unwrap();

    assert_eq!(
        (
            compaction_facts(&admin, &run_id).await,
            later.get::<_, Option<String>>(/*idx*/ 0),
            later.get::<_, Option<String>>(/*idx*/ 1),
            [2, 3, 4].map(|index| later.get::<_, bool>(index)),
            later.get::<_, String>(/*idx*/ 5),
            later.get::<_, Option<String>>(/*idx*/ 6),
        ),
        (
            [
                Some("completed".to_owned()),
                Some("new_transport".to_owned()),
                Some("1".to_owned()),
                Some("installed".to_owned()),
                Some("agent_plan_summary".to_owned()),
                Some(summary.to_owned()),
            ],
            Some("resp_summary".to_owned()),
            Some("resp_decision".to_owned()),
            [false, true, true],
            "later_request".to_owned(),
            Some("derived_summary".to_owned()),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_dropped_summary_stream_keeps_its_items_and_the_decision_continues_the_prior_response() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // The summary stream ends after its first delta, with no terminal event.
    let mut dropped = streamed("resp_dropped", "The author keeps the rain.");
    dropped.truncate(/*len*/ 3);
    let run_id = compact_on_agent_plan(
        &store,
        &admin,
        ["ec5", "ec6"],
        vec![
            streamed("resp_decision", ADVICE),
            dropped,
            streamed("resp_next", ADVICE),
        ],
    )
    .await;
    let summary_attempt = admin
        .query_one(
            "SELECT attempt.payload->>'response_reference', attempt.payload->'items'->0->>'text',
                    attempt.payload->>'observation', record.refusal_reason,
                    (decision.payload->'wire'->>'serialized_payload')::jsonb
                      ->>'previous_response_id'
               FROM storyos.model_attempts AS attempt
               JOIN storyos.active_context_compactions AS record ON record.run_id = attempt.run_id
               JOIN storyos.model_attempts AS decision
                 ON decision.run_id = attempt.run_id AND decision.attempt_role = 'decision'
              WHERE attempt.run_id = $1::text::uuid AND attempt.attempt_role = 'compaction'",
            &[&run_id],
        )
        .await
        .unwrap();

    assert_eq!(
        [0, 1, 2, 3, 4].map(|index| summary_attempt.get::<_, Option<String>>(index)),
        [
            Some("resp_dropped".to_owned()),
            Some("The author keeps the rain.".to_owned()),
            Some("outcome_unknown".to_owned()),
            Some("summary_unavailable".to_owned()),
            Some("resp_decision".to_owned()),
        ]
    );
}
