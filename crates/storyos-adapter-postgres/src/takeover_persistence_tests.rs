use storyos_application::{
    EditorClientBinding, EditorSessionId, IssueProjectCommandChallenge, OpenEditorSession,
    ProjectCommandChallengeBinding, ProjectId, ProjectScope, UserId, create_editor_session,
    issue_project_command_challenge,
};
use tokio_postgres::NoTls;

use super::*;
use crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK;

const USER: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT: &str = "018f0000-0000-7001-8000-000000000902";
const REVISION: &str = "018f0000-0000-7001-8000-000000000905";
const WRITER_SESSION: &str = "018f0000-0000-7001-8000-000000000906";
const OBSERVER_SESSION: &str = "018f0000-0000-7001-8000-000000000907";
const WRITER_SNAPSHOT: &str = "018f0000-0000-7001-8000-000000000908";
const OBSERVER_SNAPSHOT: &str = "018f0000-0000-7001-8000-000000000909";
const TAKEOVER_KEY: &str = "018f0000-0000-7001-8000-000000000910";
const TAKEOVER_ADMISSION: &str = "018f0000-0000-7001-8000-000000000912";
const TAKEOVER_RECEIPT: &str = "018f0000-0000-7001-8000-000000000913";
const TAKEOVER_EVENT: &str = "018f0000-0000-7001-8000-000000000914";
const TAKEOVER_SNAPSHOT: &str = "018f0000-0000-7001-8000-000000000915";

fn takeover_binding() -> ProjectCommandChallengeBinding {
    ProjectCommandChallengeBinding {
        project_scope: ProjectScope::new(UserId::new(USER), ProjectId::new(PROJECT)),
        client_session_binding_digest: "binding:takeover".to_owned(),
        client_session_generation: 1,
        client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
        security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
        limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
        challenge_rate_policy_revision: "storyos.project-command-challenge-rate.fixed-window.v1"
            .to_owned(),
        method: "POST".to_owned(),
        route_template:
            "/api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers".to_owned(),
        command_schema: "storyos.command.take-over-project-writer.request.v1".to_owned(),
        command_kind: "takeOverProjectWriter".to_owned(),
        canonical_command_digest: "sha256:storyos.command.takeOverProjectWriter.jcs.v1:aa"
            .to_owned(),
        idempotency_key: TAKEOVER_KEY.to_owned(),
    }
}

fn session_binding(key: &str) -> ProjectCommandChallengeBinding {
    ProjectCommandChallengeBinding {
        project_scope: ProjectScope::new(UserId::new(USER), ProjectId::new(PROJECT)),
        client_session_binding_digest: "binding:takeover".to_owned(),
        client_session_generation: 1,
        client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
        security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
        limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
        challenge_rate_policy_revision: "storyos.project-command-challenge-rate.fixed-window.v1"
            .to_owned(),
        method: "POST".to_owned(),
        route_template: "/api/v1/projects/{project_id}/editor-sessions".to_owned(),
        command_schema: "storyos.command.create-editor-session.request.v1".to_owned(),
        command_kind: "createEditorSession".to_owned(),
        canonical_command_digest: format!("sha256:storyos.test:{key}"),
        idempotency_key: key.to_owned(),
    }
}

async fn open_session(
    store: &PostgresProjectReader,
    session_id: &str,
    snapshot_id: &str,
    key: &str,
) {
    let binding = session_binding(key);
    let nonce_digest = format!("sha256:session-{key}");
    issue_project_command_challenge(
        store,
        &IssueProjectCommandChallenge {
            binding: binding.clone(),
            nonce: format!("nonce-{key}"),
            nonce_digest: nonce_digest.clone(),
        },
    )
    .await
    .unwrap();
    create_editor_session(
        store,
        &OpenEditorSession {
            project_scope: ProjectScope::new(UserId::new(USER), ProjectId::new(PROJECT)),
            editor_session_id: EditorSessionId::new(session_id),
            snapshot_id: snapshot_id.to_owned(),
            client_binding: EditorClientBinding {
                binding_ref: "binding:takeover".to_owned(),
                session_generation: 1,
                client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
                security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
            },
            challenge_binding: binding,
            nonce_digest,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn observer_takeover_settles_no_effect_activity_without_manuscript_authority() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    admin
        .batch_execute(
            "
            BEGIN;
            SET CONSTRAINTS ALL DEFERRED;
            INSERT INTO storyos.projects(owner_user_id, project_id, title, current_chapter_id)
            VALUES ('018f0000-0000-7001-8000-000000000001',
                    '018f0000-0000-7001-8000-000000000902', 'Project Takeover',
                    '018f0000-0000-7001-8000-000000000903');
            INSERT INTO storyos.manuscript_objects
              (owner_user_id, project_id, manuscript_object_id, object_kind, title)
            VALUES ('018f0000-0000-7001-8000-000000000001',
                    '018f0000-0000-7001-8000-000000000902',
                    '018f0000-0000-7001-8000-000000000903', 'chapter', 'Chapter Takeover');
            INSERT INTO storyos.authoritative_payloads
              (owner_user_id, project_id, payload_id, canonical_bytes)
            VALUES ('018f0000-0000-7001-8000-000000000001',
                    '018f0000-0000-7001-8000-000000000902',
                    '018f0000-0000-7001-8000-000000000904', convert_to('Takeover body', 'UTF8'));
            INSERT INTO storyos.authoritative_revisions
              (owner_user_id, project_id, manuscript_object_id, revision_id, payload_id)
            VALUES ('018f0000-0000-7001-8000-000000000001',
                    '018f0000-0000-7001-8000-000000000902',
                    '018f0000-0000-7001-8000-000000000903',
                    '018f0000-0000-7001-8000-000000000905',
                    '018f0000-0000-7001-8000-000000000904');
            INSERT INTO storyos.authoritative_heads
              (owner_user_id, project_id, manuscript_object_id, current_revision_id)
            VALUES ('018f0000-0000-7001-8000-000000000001',
                    '018f0000-0000-7001-8000-000000000902',
                    '018f0000-0000-7001-8000-000000000903',
                    '018f0000-0000-7001-8000-000000000905');
            COMMIT;
            ",
        )
        .await
        .unwrap();

    let store = PostgresProjectReader::new(&runtime_url);
    open_session(
        &store,
        WRITER_SESSION,
        WRITER_SNAPSHOT,
        "018f0000-0000-7001-8000-000000000917",
    )
    .await;
    open_session(
        &store,
        OBSERVER_SESSION,
        OBSERVER_SNAPSHOT,
        "018f0000-0000-7001-8000-000000000918",
    )
    .await;

    let binding = takeover_binding();
    issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: binding.clone(),
            nonce: "takeover-nonce".to_owned(),
            nonce_digest: "sha256:takeover-nonce".to_owned(),
        },
    )
    .await
    .unwrap();

    let (mut runtime, connection) = tokio_postgres::connect(&runtime_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    let transaction = runtime.transaction().await.unwrap();
    transaction
        .batch_execute(
            "SET LOCAL storyos.owner_user_id = '018f0000-0000-7001-8000-000000000001';
             SET LOCAL storyos.project_id = '018f0000-0000-7001-8000-000000000902';",
        )
        .await
        .unwrap();
    transaction
        .batch_execute(
            "
            UPDATE storyos.project_command_challenges
               SET consumed_at = clock_timestamp()
             WHERE command_kind = 'takeOverProjectWriter'
               AND idempotency_key = '018f0000-0000-7001-8000-000000000910';
            UPDATE storyos.command_idempotency
               SET outcome_kind = 'in_progress'
             WHERE command_kind = 'takeOverProjectWriter'
               AND idempotency_key = '018f0000-0000-7001-8000-000000000910';
            INSERT INTO storyos.author_command_admissions
              (owner_user_id, project_id, author_command_admission_id, command_id,
               editor_session_id, writer_generation, client_session_binding_ref,
               client_session_generation, client_contract_revision, security_policy_revision,
               action_class, method, route_template, command_schema, command_kind,
               canonical_command_digest, idempotency_key, challenge_consumed_at,
               challenge_expires_at, correlation_id, chapter_object_id,
               expected_authoritative_revision_id, expected_proposal_head_revision_ids,
               target_refs, observed_ownership_partition, editor_contract_revision,
               undo_group_id, completed_intent_record_id, local_intent_sequence, command_payload)
            SELECT owner_user_id, project_id,
                   '018f0000-0000-7001-8000-000000000912',
                   '018f0000-0000-7001-8000-000000000911',
                   '018f0000-0000-7001-8000-000000000907', 1,
                   client_session_binding_digest, client_session_generation,
                   client_contract_revision, security_policy_revision,
                   'explicit_editor_command', method, route_template, command_schema,
                   command_kind, canonical_command_digest, idempotency_key,
                   consumed_at, expires_at, '018f0000-0000-7001-8000-000000000916',
                   NULL, NULL, '{}', '{}', NULL,
                   'storyos.editor-contract.release-1.v2', NULL, NULL, NULL,
                   jsonb_build_object(
                     'command_schema', command_schema,
                     'editor_session_id', '018f0000-0000-7001-8000-000000000907',
                     'observed_writer_generation', '1')
              FROM storyos.project_command_challenges
             WHERE command_kind = 'takeOverProjectWriter'
               AND idempotency_key = '018f0000-0000-7001-8000-000000000910';
            INSERT INTO storyos.domain_receipts
              (owner_user_id, project_id, receipt_id, author_command_admission_id, command_id,
               command_kind, command_digest, idempotency_key, producer_cause, expected_heads,
               prior_heads, resulting_heads, authoritative_revision_ids, proposal_revision_ids,
               authoritative_commit_ids, draft_artifact_refs, artifact_lifecycle_event_refs,
               condition_refs, result_kind, result_payload)
            VALUES (
              '018f0000-0000-7001-8000-000000000001',
              '018f0000-0000-7001-8000-000000000902',
              '018f0000-0000-7001-8000-000000000913',
              '018f0000-0000-7001-8000-000000000912',
              '018f0000-0000-7001-8000-000000000911',
              'takeOverProjectWriter',
              'sha256:storyos.command.takeOverProjectWriter.jcs.v1:aa',
              '018f0000-0000-7001-8000-000000000910',
              'author_command_admission',
              ARRAY['018f0000-0000-7001-8000-000000000905'::uuid],
              ARRAY['018f0000-0000-7001-8000-000000000905'::uuid],
              ARRAY['018f0000-0000-7001-8000-000000000905'::uuid],
              ARRAY[]::uuid[], ARRAY[]::uuid[], ARRAY[]::uuid[], ARRAY[]::text[],
              ARRAY[]::text[], ARRAY[]::text[], 'no_effect',
              '{\"reason\":\"writer_takeover_applied\"}'::jsonb);
            INSERT INTO storyos.author_command_admission_settlements
              (owner_user_id, project_id, author_command_admission_id, settlement_kind, receipt_id)
            VALUES (
              '018f0000-0000-7001-8000-000000000001',
              '018f0000-0000-7001-8000-000000000902',
              '018f0000-0000-7001-8000-000000000912',
              'receipt_settled',
              '018f0000-0000-7001-8000-000000000913');
            UPDATE storyos.command_idempotency
               SET outcome_kind = 'settled',
                   result_reference = '018f0000-0000-7001-8000-000000000913'
             WHERE command_kind = 'takeOverProjectWriter'
               AND idempotency_key = '018f0000-0000-7001-8000-000000000910';
            INSERT INTO storyos.scope_counters (owner_user_id, project_id)
            VALUES ('018f0000-0000-7001-8000-000000000001',
                    '018f0000-0000-7001-8000-000000000902')
            ON CONFLICT DO NOTHING;
            UPDATE storyos.scope_counters
               SET project_activity_position = project_activity_position + 1
             WHERE owner_user_id = '018f0000-0000-7001-8000-000000000001'
               AND project_id = '018f0000-0000-7001-8000-000000000902';
            INSERT INTO storyos.project_writer_generations
              (owner_user_id, project_id, writer_generation, current_editor_session_id)
            VALUES (
              '018f0000-0000-7001-8000-000000000001',
              '018f0000-0000-7001-8000-000000000902',
              2, '018f0000-0000-7001-8000-000000000907');
            INSERT INTO storyos.project_snapshots
              (owner_user_id, project_id, snapshot_id, replay_generation,
               project_activity_position, snapshot_kind, redaction_profile, schema_profile)
            VALUES (
              '018f0000-0000-7001-8000-000000000001',
              '018f0000-0000-7001-8000-000000000902',
              '018f0000-0000-7001-8000-000000000915', 1, 1,
              'canonical', 'storyos.author.v1', 'storyos.public.release.1');
            INSERT INTO storyos.project_activity_event_payloads
              (owner_user_id, project_id, project_activity_position, project_activity_event_id,
               event_kind, receipt_id, receipt_result_kind, payload)
            VALUES (
              '018f0000-0000-7001-8000-000000000001',
              '018f0000-0000-7001-8000-000000000902',
              1, '018f0000-0000-7001-8000-000000000914',
              'writer_takeover_applied', '018f0000-0000-7001-8000-000000000913', 'no_effect',
              jsonb_build_object(
                'kind', 'takeover_applied',
                'prior_editor_session_id', '018f0000-0000-7001-8000-000000000906',
                'prior_writer_generation', '1',
                'resulting_editor_session_id', '018f0000-0000-7001-8000-000000000907',
                'resulting_writer_generation', '2',
                'resulting_snapshot_id', '018f0000-0000-7001-8000-000000000915',
                'resulting_snapshot_activity_position', '1',
                'resulting_heads', jsonb_build_array('018f0000-0000-7001-8000-000000000905')));
            ",
        )
        .await
        .unwrap();
    transaction.commit().await.unwrap();

    let snapshot: serde_json::Value = serde_json::from_str(
        &admin
            .query_one(
                "SELECT jsonb_build_object(
               'generations', (SELECT jsonb_agg(jsonb_build_object(
                    'writer_generation', writer_generation::text,
                    'current_editor_session_id', current_editor_session_id::text)
                  ORDER BY writer_generation)
                 FROM storyos.project_writer_generations
                WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
               'admission', (SELECT jsonb_build_object(
                    'command_kind', command_kind,
                    'action_class', action_class,
                    'editor_session_id', editor_session_id::text,
                    'writer_generation', writer_generation::text)
                 FROM storyos.author_command_admissions
                WHERE author_command_admission_id = $3::text::uuid),
               'receipt', (SELECT jsonb_build_object(
                    'command_kind', command_kind,
                    'result_kind', result_kind,
                    'reason', result_payload->>'reason',
                    'authoritative_revision_ids', cardinality(authoritative_revision_ids),
                    'authoritative_commit_ids', cardinality(authoritative_commit_ids))
                 FROM storyos.domain_receipts
                WHERE receipt_id = $4::text::uuid),
               'author_edit_activity', (SELECT count(*) FROM storyos.project_activity_events
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
               'takeover_activity', (SELECT jsonb_build_object(
                    'event_kind', event_kind,
                    'receipt_result_kind', receipt_result_kind,
                    'payload', payload)
                 FROM storyos.project_activity_event_payloads
                WHERE project_activity_event_id = $5::text::uuid),
               'actions', (SELECT count(*) FROM storyos.author_action_entries
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
               'commits', (SELECT count(*) FROM storyos.authoritative_commits
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
               'envelopes', (SELECT count(*) FROM storyos.authoritative_revision_envelopes
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid)
            )::text",
                &[
                    &USER,
                    &PROJECT,
                    &TAKEOVER_ADMISSION,
                    &TAKEOVER_RECEIPT,
                    &TAKEOVER_EVENT,
                ],
            )
            .await
            .unwrap()
            .get::<_, String>(0),
    )
    .unwrap();
    assert_eq!(
        snapshot,
        serde_json::json!({
            "generations": [
                {
                    "writer_generation": "1",
                    "current_editor_session_id": WRITER_SESSION
                },
                {
                    "writer_generation": "2",
                    "current_editor_session_id": OBSERVER_SESSION
                }
            ],
            "admission": {
                "command_kind": "takeOverProjectWriter",
                "action_class": "explicit_editor_command",
                "editor_session_id": OBSERVER_SESSION,
                "writer_generation": "1"
            },
            "receipt": {
                "command_kind": "takeOverProjectWriter",
                "result_kind": "no_effect",
                "reason": "writer_takeover_applied",
                "authoritative_revision_ids": 0,
                "authoritative_commit_ids": 0
            },
            "author_edit_activity": 0,
            "takeover_activity": {
                "event_kind": "writer_takeover_applied",
                "receipt_result_kind": "no_effect",
                "payload": {
                    "kind": "takeover_applied",
                    "prior_editor_session_id": WRITER_SESSION,
                    "prior_writer_generation": "1",
                    "resulting_editor_session_id": OBSERVER_SESSION,
                    "resulting_writer_generation": "2",
                    "resulting_snapshot_id": TAKEOVER_SNAPSHOT,
                    "resulting_snapshot_activity_position": "1",
                    "resulting_heads": [REVISION]
                }
            },
            "actions": 0,
            "commits": 0,
            "envelopes": 0
        })
    );

    let forbidden = runtime.transaction().await.unwrap();
    assert!(
        forbidden
            .batch_execute(
                "SET LOCAL storyos.owner_user_id = '018f0000-0000-7001-8000-000000000001';
             SET LOCAL storyos.project_id = '018f0000-0000-7001-8000-000000000902';
             INSERT INTO storyos.project_activity_events
               (owner_user_id, project_id, project_activity_position, project_activity_event_id,
                event_kind, receipt_id, receipt_result_kind, authoritative_commit_id,
                resulting_revision_id, author_action_sequence)
             VALUES (
               '018f0000-0000-7001-8000-000000000001',
               '018f0000-0000-7001-8000-000000000902',
               2, '018f0000-0000-7001-8000-000000000919',
               'authoritative_author_edit_applied',
               '018f0000-0000-7001-8000-000000000913', 'no_effect',
               '018f0000-0000-7001-8000-000000000905',
               '018f0000-0000-7001-8000-000000000905', 1);",
            )
            .await
            .is_err()
    );
}
