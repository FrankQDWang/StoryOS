use std::time::Duration;

use storyos_application::{
    AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId, IssueProjectCommandChallenge,
    OpenEditorSession, ProjectCommandChallengeBinding, ProjectId, ProjectScope,
    TakeOverProjectWriterCommand, TakeOverProjectWriterEffect, TakeOverProjectWriterError,
    TakeOverProjectWriterSettlement, UserId, create_editor_session,
    issue_project_command_challenge, take_over_project_writer,
};
use tokio_postgres::{Client, NoTls};

use super::*;
use crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK;

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
    activity_position: Option<String>,
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

async fn seed_project(admin: &Client, fixture: &TakeoverFixture) {
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
) -> TakeOverProjectWriterCommand {
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
        },
    )
    .await
    .unwrap();
    TakeOverProjectWriterCommand {
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
        editor_session_id: EditorSessionId::new(editor_session_id),
        observed_writer_generation,
        editor_contract_revision: "storyos.editor-contract.release-1.v2".to_owned(),
    }
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
               (SELECT project_activity_position::text FROM storyos.scope_counters
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
        activity_position: counts.get(3),
        pending_keys,
        settled_keys,
        consumed_keys,
    }
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
        activity_position: None,
        pending_keys,
        settled_keys: Vec::new(),
        consumed_keys: Vec::new(),
    }
}

fn assert_complete_settlement(
    settlement: &TakeOverProjectWriterSettlement,
    command: &TakeOverProjectWriterCommand,
    fixture: &TakeoverFixture,
) {
    let resulting_snapshot_id = match &settlement.effect {
        TakeOverProjectWriterEffect::TakeoverApplied {
            resulting_snapshot_id,
            ..
        } => resulting_snapshot_id.clone(),
    };
    assert_eq!(
        settlement,
        &TakeOverProjectWriterSettlement {
            ids: command.ids.clone(),
            receipt_created_at: settlement.receipt_created_at.clone(),
            expected_heads: vec![fixture.revision_id.clone()],
            prior_heads: vec![fixture.revision_id.clone()],
            resulting_heads: vec![fixture.revision_id.clone()],
            effect: TakeOverProjectWriterEffect::TakeoverApplied {
                prior_editor_session_id: fixture.writer_session_id.clone(),
                prior_writer_generation: 1,
                resulting_editor_session_id: command.editor_session_id.as_ref().to_owned(),
                resulting_writer_generation: 2,
                resulting_snapshot_id,
                resulting_snapshot_activity_position: 1,
            },
        }
    );
}

fn named_store(database_url: &str, application_name: &str) -> PostgresProjectReader {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    PostgresProjectReader::new(format!(
        "{database_url}{separator}application_name={application_name}"
    ))
}

async fn wait_for_generation_insert_gate(
    admin: &Client,
    application_names: &[String],
    blocker_pid: i32,
) -> Result<(), String> {
    let waited = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let blocked: i64 = admin
                .query_one(
                    "SELECT count(DISTINCT activity.pid)
                       FROM pg_catalog.pg_stat_activity AS activity
                      WHERE activity.application_name = ANY($1::text[])
                        AND activity.wait_event_type = 'Lock'
                        AND $2::int = ANY(pg_catalog.pg_blocking_pids(activity.pid))",
                    &[&application_names, &blocker_pid],
                )
                .await
                .map_err(|error| error.to_string())?
                .get(0);
            if blocked == application_names.len() as i64 {
                return Ok(());
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    match waited {
        Ok(result) => result,
        Err(_) => {
            let observations = admin
                .query(
                    "SELECT application_name, state, wait_event_type, wait_event,
                            left(query, 160), pg_catalog.pg_blocking_pids(pid)
                       FROM pg_catalog.pg_stat_activity
                      WHERE application_name = ANY($1::text[])
                      ORDER BY application_name",
                    &[&application_names],
                )
                .await
                .map_err(|error| error.to_string())?
                .into_iter()
                .map(|row| {
                    (
                        row.get::<_, String>(0),
                        row.get::<_, String>(1),
                        row.get::<_, Option<String>>(2),
                        row.get::<_, Option<String>>(3),
                        row.get::<_, String>(4),
                        row.get::<_, Vec<i32>>(5),
                    )
                })
                .collect::<Vec<_>>();
            Err(format!(
                "both Takeovers did not reach the generation gate before timeout: {observations:?}"
            ))
        }
    }
}

fn assert_database_race_failure(error: TakeOverProjectWriterError) {
    let TakeOverProjectWriterError::Unavailable(source) = error else {
        panic!("the losing Takeover must fail as unavailable: {error}");
    };
    let database_error = source
        .downcast_ref::<tokio_postgres::Error>()
        .expect("the losing Takeover must preserve the PostgreSQL error source");
    let sqlstate = database_error
        .code()
        .map(tokio_postgres::error::SqlState::code);
    assert!(
        matches!(sqlstate, Some("40001" | "23505")),
        "unexpected losing Takeover SQLSTATE: {sqlstate:?}"
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn admission_returns_the_prior_writer_and_rejections_leave_no_evidence() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin = connect(&admin_url).await;
    let fixture = TakeoverFixture::new(950);
    seed_project(&admin, &fixture).await;
    let store = PostgresProjectReader::new(&runtime_url);
    open_fixture_sessions(&store, &fixture, 1000).await;

    let same_session =
        prepare_takeover(&store, &fixture, &fixture.writer_session_id, 1, 1010).await;
    assert!(matches!(
        take_over_project_writer(&store, &same_session).await,
        Err(TakeOverProjectWriterError::BindingConflict)
    ));
    assert_eq!(
        durable_state(&admin, &fixture).await,
        expected_initial_state(
            &fixture,
            vec![same_session.challenge_binding.idempotency_key.clone()]
        )
    );

    let stale_generation =
        prepare_takeover(&store, &fixture, &fixture.observer_one_session_id, 0, 1020).await;
    assert!(matches!(
        take_over_project_writer(&store, &stale_generation).await,
        Err(TakeOverProjectWriterError::BindingConflict)
    ));
    assert_eq!(
        durable_state(&admin, &fixture).await,
        expected_initial_state(
            &fixture,
            vec![
                same_session.challenge_binding.idempotency_key.clone(),
                stale_generation.challenge_binding.idempotency_key.clone(),
            ]
        )
    );

    let successful =
        prepare_takeover(&store, &fixture, &fixture.observer_one_session_id, 1, 1030).await;
    let settlement = take_over_project_writer(&store, &successful).await.unwrap();
    assert_complete_settlement(&settlement, &successful, &fixture);
    assert_eq!(
        durable_state(&admin, &fixture).await,
        DurableTakeoverState {
            generations: vec![
                ("1".to_owned(), fixture.writer_session_id.clone()),
                ("2".to_owned(), fixture.observer_one_session_id.clone()),
            ],
            admission_keys: vec![successful.challenge_binding.idempotency_key.clone()],
            receipt_keys: vec![successful.challenge_binding.idempotency_key.clone()],
            snapshot_count: 4,
            activity_count: 1,
            settlement_count: 1,
            activity_position: Some("1".to_owned()),
            pending_keys: vec![
                same_session.challenge_binding.idempotency_key.clone(),
                stale_generation.challenge_binding.idempotency_key.clone(),
            ],
            settled_keys: vec![successful.challenge_binding.idempotency_key.clone()],
            consumed_keys: vec![successful.challenge_binding.idempotency_key.clone()],
        }
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn two_observers_race_with_one_complete_settlement_and_one_full_rollback() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin = connect(&admin_url).await;
    let monitor_client = connect(&admin_url).await;
    let fixture = TakeoverFixture::new(1200);
    seed_project(&admin, &fixture).await;
    let setup_store = PostgresProjectReader::new(&runtime_url);
    open_fixture_sessions(&setup_store, &fixture, 1250).await;

    let application_names = vec![
        "storyos_takeover_observer_a".to_owned(),
        "storyos_takeover_observer_b".to_owned(),
    ];
    let left_store = named_store(&runtime_url, &application_names[0]);
    let right_store = named_store(&runtime_url, &application_names[1]);
    let left_command = prepare_takeover(
        &left_store,
        &fixture,
        &fixture.observer_one_session_id,
        1,
        1300,
    )
    .await;
    let right_command = prepare_takeover(
        &right_store,
        &fixture,
        &fixture.observer_two_session_id,
        1,
        1320,
    )
    .await;
    let left_evidence = (
        left_command.editor_session_id.as_ref().to_owned(),
        left_command.challenge_binding.idempotency_key.clone(),
        left_command.clone(),
    );
    let right_evidence = (
        right_command.editor_session_id.as_ref().to_owned(),
        right_command.challenge_binding.idempotency_key.clone(),
        right_command.clone(),
    );

    admin
        .batch_execute("BEGIN; LOCK TABLE storyos.project_writer_generations IN SHARE MODE")
        .await
        .unwrap();
    let blocker_pid: i32 = admin
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);
    let monitor = async move {
        let gate =
            wait_for_generation_insert_gate(&monitor_client, &application_names, blocker_pid).await;
        admin
            .batch_execute(if gate.is_ok() { "COMMIT" } else { "ROLLBACK" })
            .await
            .unwrap();
        (admin, gate)
    };
    let (left_result, right_result, (admin, gate)) = tokio::join!(
        tokio::time::timeout(
            Duration::from_secs(20),
            take_over_project_writer(&left_store, &left_command)
        ),
        tokio::time::timeout(
            Duration::from_secs(20),
            take_over_project_writer(&right_store, &right_command)
        ),
        monitor,
    );
    if let Err(error) = gate {
        panic!("{error}");
    }
    let left_result = left_result.expect("the left Takeover must finish after the gate opens");
    let right_result = right_result.expect("the right Takeover must finish after the gate opens");
    let (winner_session, winner_key, loser_key, winner_command, settlement) =
        match (left_result, right_result) {
            (Ok(settlement), Err(error)) => {
                assert_database_race_failure(error);
                (
                    left_evidence.0,
                    left_evidence.1,
                    right_evidence.1,
                    left_evidence.2,
                    settlement,
                )
            }
            (Err(error), Ok(settlement)) => {
                assert_database_race_failure(error);
                (
                    right_evidence.0,
                    right_evidence.1,
                    left_evidence.1,
                    right_evidence.2,
                    settlement,
                )
            }
            (left_result, right_result) => panic!(
                "exactly one Takeover must settle: left={left_result:?}, right={right_result:?}"
            ),
        };
    assert_complete_settlement(&settlement, &winner_command, &fixture);
    assert_eq!(
        durable_state(&admin, &fixture).await,
        DurableTakeoverState {
            generations: vec![
                ("1".to_owned(), fixture.writer_session_id.clone()),
                ("2".to_owned(), winner_session),
            ],
            admission_keys: vec![winner_key.clone()],
            receipt_keys: vec![winner_key.clone()],
            snapshot_count: 4,
            activity_count: 1,
            settlement_count: 1,
            activity_position: Some("1".to_owned()),
            pending_keys: vec![loser_key],
            settled_keys: vec![winner_key.clone()],
            consumed_keys: vec![winner_key],
        }
    );
}
