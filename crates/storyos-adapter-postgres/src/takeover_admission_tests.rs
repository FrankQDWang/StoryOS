use storyos_application::{
    AuthorCommandAdmissionIds, CanonicalSnapshot, EditorClientBinding, EditorSessionId,
    IssueProjectCommandChallenge, OpenEditorSession, ProjectCommandChallengeBinding,
    ProjectCommandEnvelope, ProjectCommandError, ProjectId, ProjectScope, SnapshotLookup,
    SnapshotStore, TakeOverProjectWriterInput, TakeOverProjectWriterSettlement, UserId,
    WriterTakeover, create_editor_session, issue_project_command_challenge,
};
use storyos_core::{TakeOverProjectWriterNoEffect, TransitionOutcome};
use tokio_postgres::{Client, NoTls};

use super::*;
use crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK;
use crate::command_sequence::tests::CommandCall;

const USER: &str = "018f0000-0000-7001-8000-000000000001";
const EMPTY_OBJECT_TAKEOVER_DIGEST: &str = "sha256:storyos.command.takeOverProjectWriter.jcs.v1:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";

#[derive(Debug)]
struct TakeoverFixture {
    project_id: String,
    chapter_id: String,
    payload_id: String,
    revision_id: String,
    writer_session_id: String,
    observer_one_session_id: String,
    observer_two_session_id: String,
    writer_snapshot_id: String,
    observer_one_snapshot_id: String,
    observer_two_snapshot_id: String,
}

impl TakeoverFixture {
    fn new(base: u32) -> Self {
        Self {
            project_id: test_uuid(base),
            chapter_id: test_uuid(base + 1),
            payload_id: test_uuid(base + 2),
            revision_id: test_uuid(base + 3),
            writer_session_id: test_uuid(base + 4),
            observer_one_session_id: test_uuid(base + 5),
            observer_two_session_id: test_uuid(base + 6),
            writer_snapshot_id: test_uuid(base + 7),
            observer_one_snapshot_id: test_uuid(base + 8),
            observer_two_snapshot_id: test_uuid(base + 9),
        }
    }

    fn scope(&self) -> ProjectScope {
        ProjectScope::new(UserId::new(USER), ProjectId::new(&self.project_id))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct DurableTakeoverState {
    generations: Vec<(String, String)>,
    admission_keys: Vec<String>,
    receipt_keys: Vec<String>,
    snapshot_count: i64,
    activity_count: i64,
    settlement_count: i64,
    /// The Author Action sequence, the Commit sequence, and the Activity position.
    counters: Option<[String; 3]>,
    pending_keys: Vec<String>,
    settled_keys: Vec<String>,
    consumed_keys: Vec<String>,
}

fn test_uuid(number: u32) -> String {
    format!("018f0000-0000-7001-8000-{number:012}")
}

async fn connect(database_url: &str) -> Client {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        let _connection_result = connection.await;
    });
    client
}

async fn seed_takeover_project(admin: &Client, fixture: &TakeoverFixture) {
    let project = &fixture.project_id;
    let chapter = &fixture.chapter_id;
    let payload = &fixture.payload_id;
    let revision = &fixture.revision_id;
    admin
        .batch_execute(&format!(
            "BEGIN;
             SET CONSTRAINTS ALL DEFERRED;
             INSERT INTO storyos.projects(owner_user_id, project_id, title, current_chapter_id)
             VALUES ('{USER}', '{project}', 'Takeover Admission Project', '{chapter}');
             INSERT INTO storyos.manuscript_objects
               (owner_user_id, project_id, manuscript_object_id, object_kind, title)
             VALUES ('{USER}', '{project}', '{chapter}', 'chapter',
                     'Takeover Admission Chapter');
             INSERT INTO storyos.authoritative_payloads
               (owner_user_id, project_id, payload_id, canonical_bytes)
             VALUES ('{USER}', '{project}', '{payload}',
                     convert_to('Takeover admission body', 'UTF8'));
             INSERT INTO storyos.authoritative_revisions
               (owner_user_id, project_id, manuscript_object_id, revision_id, payload_id)
             VALUES ('{USER}', '{project}', '{chapter}', '{revision}', '{payload}');
             INSERT INTO storyos.authoritative_heads
               (owner_user_id, project_id, manuscript_object_id, current_revision_id)
             VALUES ('{USER}', '{project}', '{chapter}', '{revision}');
             COMMIT;"
        ))
        .await
        .unwrap();
}

fn session_binding(
    fixture: &TakeoverFixture,
    binding_ref: &str,
    idempotency_key: &str,
) -> ProjectCommandChallengeBinding {
    ProjectCommandChallengeBinding {
        project_scope: fixture.scope(),
        client_session_binding_digest: binding_ref.to_owned(),
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
        canonical_command_digest: format!("sha256:session-{idempotency_key}"),
        idempotency_key: idempotency_key.to_owned(),
    }
}

async fn open_session(
    store: &PostgresProjectReader,
    fixture: &TakeoverFixture,
    binding_ref: &str,
    editor_session_id: &str,
    snapshot_id: &str,
    idempotency_key: &str,
) {
    let binding = session_binding(fixture, binding_ref, idempotency_key);
    let nonce_digest = format!("sha256:session-{idempotency_key}");
    issue_project_command_challenge(
        store,
        &IssueProjectCommandChallenge {
            binding: binding.clone(),
            nonce: format!("nonce:{idempotency_key}"),
            nonce_digest: nonce_digest.clone(),
            earlier_revision_nonces: Vec::new(),
        },
    )
    .await
    .unwrap();
    create_editor_session(
        store,
        &OpenEditorSession {
            project_scope: fixture.scope(),
            editor_session_id: EditorSessionId::new(editor_session_id),
            snapshot_id: snapshot_id.to_owned(),
            client_binding: EditorClientBinding {
                binding_ref: binding_ref.to_owned(),
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

async fn open_fixture_sessions(
    store: &PostgresProjectReader,
    fixture: &TakeoverFixture,
    key_base: u32,
) {
    let binding_ref = format!("binding:takeover-admission-{}", fixture.project_id);
    open_session(
        store,
        fixture,
        &binding_ref,
        &fixture.writer_session_id,
        &fixture.writer_snapshot_id,
        &test_uuid(key_base),
    )
    .await;
    open_session(
        store,
        fixture,
        &binding_ref,
        &fixture.observer_one_session_id,
        &fixture.observer_one_snapshot_id,
        &test_uuid(key_base + 1),
    )
    .await;
    open_session(
        store,
        fixture,
        &binding_ref,
        &fixture.observer_two_session_id,
        &fixture.observer_two_snapshot_id,
        &test_uuid(key_base + 2),
    )
    .await;
}

fn takeover_binding(
    fixture: &TakeoverFixture,
    binding_ref: &str,
    idempotency_key: &str,
) -> ProjectCommandChallengeBinding {
    ProjectCommandChallengeBinding {
        project_scope: fixture.scope(),
        client_session_binding_digest: binding_ref.to_owned(),
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
        canonical_command_digest: EMPTY_OBJECT_TAKEOVER_DIGEST.to_owned(),
        idempotency_key: idempotency_key.to_owned(),
    }
}

async fn prepare_takeover(
    store: &PostgresProjectReader,
    fixture: &TakeoverFixture,
    editor_session_id: &str,
    observed_writer_generation: u64,
    id_base: u32,
) -> CommandCall<TakeOverProjectWriterInput> {
    let binding_ref = format!("binding:takeover-admission-{}", fixture.project_id);
    let idempotency_key = test_uuid(id_base);
    let nonce_digest = format!("sha256:takeover-admission-{id_base}");
    let binding = takeover_binding(fixture, &binding_ref, &idempotency_key);
    issue_project_command_challenge(
        store,
        &IssueProjectCommandChallenge {
            binding: binding.clone(),
            nonce: format!("nonce:takeover-admission-{id_base}"),
            nonce_digest: nonce_digest.clone(),
            earlier_revision_nonces: Vec::new(),
        },
    )
    .await
    .unwrap();
    CommandCall {
        envelope: ProjectCommandEnvelope {
            project_scope: fixture.scope(),
            client_binding: EditorClientBinding {
                binding_ref,
                session_generation: 1,
                client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
                security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
            },
            challenge_binding: binding,
            nonce_digest,
            canonical_command_bytes: br#"{}"#.to_vec(),
            correlation_id: test_uuid(id_base + 1),
            ids: AuthorCommandAdmissionIds {
                command_id: test_uuid(id_base + 2),
                author_command_admission_id: test_uuid(id_base + 3),
                receipt_id: test_uuid(id_base + 4),
            },
        },
        input: TakeOverProjectWriterInput {
            editor_session_id: EditorSessionId::new(editor_session_id),
            observed_writer_generation,
            editor_contract_revision: "storyos.editor-contract.release-1.v3".to_owned(),
        },
    }
}

async fn take_over_project_writer(
    store: &PostgresProjectReader,
    call: &CommandCall<TakeOverProjectWriterInput>,
) -> Result<TakeOverProjectWriterSettlement, ProjectCommandError> {
    store
        .take_over_project_writer(&call.envelope, &call.input)
        .await
}

async fn idempotency_keys(admin: &Client, fixture: &TakeoverFixture, outcome: &str) -> Vec<String> {
    admin
        .query(
            "SELECT idempotency_key::text FROM storyos.command_idempotency
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'takeOverProjectWriter' AND outcome_kind = $3
              ORDER BY idempotency_key",
            &[&USER, &fixture.project_id.as_str(), &outcome],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get(0))
        .collect()
}

async fn durable_state(admin: &Client, fixture: &TakeoverFixture) -> DurableTakeoverState {
    let generations = admin
        .query(
            "SELECT writer_generation::text, current_editor_session_id::text
               FROM storyos.project_writer_generations
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              ORDER BY writer_generation",
            &[&USER, &fixture.project_id.as_str()],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect();
    let admission_keys = admin
        .query(
            "SELECT idempotency_key::text FROM storyos.author_command_admissions
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'takeOverProjectWriter'
              ORDER BY idempotency_key",
            &[&USER, &fixture.project_id.as_str()],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get(0))
        .collect();
    let receipt_keys = admin
        .query(
            "SELECT idempotency_key::text FROM storyos.domain_receipts
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'takeOverProjectWriter'
              ORDER BY idempotency_key",
            &[&USER, &fixture.project_id.as_str()],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get(0))
        .collect();
    let counts = admin
        .query_one(
            "SELECT
               (SELECT count(*) FROM storyos.project_snapshots
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
               (SELECT count(*) FROM storyos.project_activity_event_payloads
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                   AND event_kind = 'writer_takeover_applied'),
               (SELECT count(*) FROM storyos.author_command_admission_settlements
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
               (SELECT ARRAY[author_action_sequence::text, authoritative_commit_sequence::text,
                             project_activity_position::text]
                  FROM storyos.scope_counters
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid)",
            &[&USER, &fixture.project_id.as_str()],
        )
        .await
        .unwrap();
    let pending_keys = idempotency_keys(admin, fixture, "pending").await;
    let settled_keys = idempotency_keys(admin, fixture, "settled").await;
    let consumed_keys = admin
        .query(
            "SELECT idempotency_key::text FROM storyos.project_command_challenges
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'takeOverProjectWriter' AND consumed_at IS NOT NULL
              ORDER BY idempotency_key",
            &[&USER, &fixture.project_id.as_str()],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get(0))
        .collect();
    DurableTakeoverState {
        generations,
        admission_keys,
        receipt_keys,
        snapshot_count: counts.get(0),
        activity_count: counts.get(1),
        settlement_count: counts.get(2),
        counters: counts
            .get::<_, Option<Vec<String>>>(3)
            .map(|counters| counters.try_into().unwrap()),
        pending_keys,
        settled_keys,
        consumed_keys,
    }
}

async fn base_rows(admin: &Client, fixture: &TakeoverFixture) -> serde_json::Value {
    let row = admin
        .query_one(
            "SELECT jsonb_object_agg(editor_session_id::text, to_jsonb(snapshot))::text
               FROM storyos.editor_session_base_snapshots AS snapshot
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&USER, &fixture.project_id.as_str()],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get(0)).unwrap()
}

fn expected_initial_state(
    fixture: &TakeoverFixture,
    pending_keys: Vec<String>,
) -> DurableTakeoverState {
    DurableTakeoverState {
        generations: vec![("1".to_owned(), fixture.writer_session_id.clone())],
        admission_keys: Vec::new(),
        receipt_keys: Vec::new(),
        snapshot_count: 3,
        activity_count: 0,
        settlement_count: 0,
        counters: None,
        pending_keys,
        settled_keys: Vec::new(),
        consumed_keys: Vec::new(),
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_takeover_hands_the_writer_to_an_observer_and_refuses_a_stale_or_current_writer() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin = connect(&admin_url).await;
    let fixture = TakeoverFixture::new(950);
    seed_takeover_project(&admin, &fixture).await;
    let store = PostgresProjectReader::new(&runtime_url);
    open_fixture_sessions(&store, &fixture, 1000).await;
    let initial_bases = base_rows(&admin, &fixture).await;

    let same_session =
        prepare_takeover(&store, &fixture, &fixture.writer_session_id, 1, 1010).await;
    let stale_generation =
        prepare_takeover(&store, &fixture, &fixture.observer_one_session_id, 0, 1020).await;
    let refused_keys = [&same_session, &stale_generation]
        .map(|call| call.envelope.challenge_binding.idempotency_key.clone())
        .to_vec();
    for refused in [&same_session, &stale_generation] {
        assert!(matches!(
            take_over_project_writer(&store, refused).await,
            Err(ProjectCommandError::BindingConflict)
        ));
    }
    assert_eq!(
        durable_state(&admin, &fixture).await,
        expected_initial_state(&fixture, refused_keys.clone())
    );

    let successful =
        prepare_takeover(&store, &fixture, &fixture.observer_one_session_id, 1, 1030).await;
    let settlement = take_over_project_writer(&store, &successful).await.unwrap();
    let settled_bases = base_rows(&admin, &fixture).await;
    let base_snapshot_id = settled_bases[&fixture.observer_one_session_id]["snapshot_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        settlement,
        TakeOverProjectWriterSettlement {
            ids: successful.envelope.ids.clone(),
            receipt_created_at: settlement.receipt_created_at.clone(),
            outcome: TransitionOutcome::NoEffect(
                TakeOverProjectWriterNoEffect::WriterTakeoverApplied
            ),
            response: (),
            zero_authority_effect: Some(WriterTakeover {
                prior_editor_session_id: fixture.writer_session_id.clone(),
                prior_writer_generation: 1,
                resulting_editor_session_id: fixture.observer_one_session_id.clone(),
                resulting_writer_generation: 2,
                resulting_snapshot_id: base_snapshot_id.clone(),
                resulting_snapshot_activity_position: 1,
                resulting_head: fixture.revision_id.clone(),
            }),
        }
    );
    // Only the base of the new writer moves to the takeover Snapshot.
    let mut expected_bases = initial_bases.clone();
    let observer_base = &mut expected_bases[&fixture.observer_one_session_id];
    assert_ne!(observer_base["snapshot_id"], base_snapshot_id.as_str());
    observer_base["snapshot_id"] = serde_json::json!(base_snapshot_id);
    observer_base["created_at"] =
        settled_bases[&fixture.observer_one_session_id]["created_at"].clone();
    observer_base["project_activity_position"] = serde_json::json!(1);
    assert_eq!(settled_bases, expected_bases);
    let resumed = store
        .activity_resume(&SnapshotLookup {
            project_scope: fixture.scope(),
            snapshot_id: base_snapshot_id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(
        resumed,
        CanonicalSnapshot {
            snapshot_id: base_snapshot_id,
            project_activity_position: 1,
            replay_generation: 1,
            floor_position: 0,
            redaction_profile: "storyos.author.v1".to_owned(),
            schema_profile: "storyos.public.release.1".to_owned(),
            created_at: resumed.created_at.clone(),
            expires_at: None,
        }
    );
    let settled_key = successful
        .envelope
        .challenge_binding
        .idempotency_key
        .clone();
    assert_eq!(
        durable_state(&admin, &fixture).await,
        DurableTakeoverState {
            generations: vec![
                ("1".to_owned(), fixture.writer_session_id.clone()),
                ("2".to_owned(), fixture.observer_one_session_id.clone()),
            ],
            admission_keys: vec![settled_key.clone()],
            receipt_keys: vec![settled_key.clone()],
            snapshot_count: 4,
            activity_count: 1,
            settlement_count: 1,
            counters: Some(["0", "0", "1"].map(str::to_owned)),
            pending_keys: refused_keys,
            settled_keys: vec![settled_key.clone()],
            consumed_keys: vec![settled_key],
        }
    );

    // A takeover moves only the Activity position of an existing counter row.
    admin
        .execute(
            "UPDATE storyos.scope_counters
                SET author_action_sequence = 7, authoritative_commit_sequence = 11
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&USER, &fixture.project_id.as_str()],
        )
        .await
        .unwrap();
    let second =
        prepare_takeover(&store, &fixture, &fixture.observer_two_session_id, 2, 1300).await;
    let second_effect = take_over_project_writer(&store, &second)
        .await
        .unwrap()
        .zero_authority_effect
        .expect("a writer takeover records its writer effect");
    assert_eq!(
        second_effect,
        WriterTakeover {
            prior_editor_session_id: fixture.observer_one_session_id.clone(),
            prior_writer_generation: 2,
            resulting_editor_session_id: fixture.observer_two_session_id.clone(),
            resulting_writer_generation: 3,
            resulting_snapshot_id: second_effect.resulting_snapshot_id.clone(),
            resulting_snapshot_activity_position: 2,
            resulting_head: fixture.revision_id.clone(),
        }
    );
    assert_eq!(
        durable_state(&admin, &fixture).await.counters,
        Some(["7", "11", "2"].map(str::to_owned))
    );
}
