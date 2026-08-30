use storyos_application::{
    AppendAuthorCommandOutcomeUnknown, AuthorCommandOutcomeUnknownBoundary,
    AuthorCommandOutcomeUnknownError, AuthorCommandOutcomeUnknownObservation,
    AuthorCommandOutcomeUnknownReason, EditorClientBinding, EditorSessionId,
    IssueProjectCommandChallenge, OpenEditorSession, ProjectId, ProjectScope,
    ReconciliationRequired, UserId, append_author_command_outcome_unknown, create_editor_session,
    issue_project_command_challenge,
};
use tokio_postgres::NoTls;

use super::tests::{AUTHOR_EDIT_TEST_LOCK, author_command, binding};
use super::*;

const USER: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT: &str = "018f0000-0000-7001-8000-000000000002";

async fn lock_arbiter(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    idempotency_key: &str,
) -> (PostgresProjectCommandTransaction, i32) {
    let transaction = store
        .begin_project_command_transaction(scope)
        .await
        .unwrap();
    let owner = scope.owner_user_id.as_ref();
    let project = scope.project_id.as_ref();
    let pid = transaction
        .client
        .query_one(
            "SELECT pg_backend_pid() FROM storyos.command_idempotency
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = 'applyAuthorEdit' AND idempotency_key = $3::text::uuid
              FOR UPDATE",
            &[&owner, &project, &idempotency_key],
        )
        .await
        .unwrap()
        .get(0);
    (transaction, pid)
}

async fn wait_for_blocked(admin: &tokio_postgres::Client, names: &[String], blocker_pid: i32) {
    let names = names.to_vec();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let blocked = admin
                .query_one(
                    "SELECT count(*), COALESCE(bool_or(
                              $2::int = ANY(pg_catalog.pg_blocking_pids(activity.pid))), false)
                       FROM pg_catalog.pg_stat_activity AS activity
                      WHERE activity.application_name = ANY($1::text[])
                        AND activity.wait_event_type = 'Lock'
                        AND activity.query LIKE '%author_command_admissions AS admission%'",
                    &[&names, &blocker_pid],
                )
                .await
                .unwrap();
            if blocked.get::<_, i64>(0) == names.len() as i64 && blocked.get::<_, bool>(1) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("every append must reach the command arbiter lock");
}

async fn command_state(
    admin: &tokio_postgres::Client,
    command: &storyos_application::ApplyAuthorEditCommand,
) -> String {
    let owner = command.project_scope.owner_user_id.as_ref();
    let project = command.project_scope.project_id.as_ref();
    admin
        .query_one(
            "SELECT json_build_array(
               (SELECT current_revision_id::text FROM storyos.authoritative_heads
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid AND manuscript_object_id = $4::text::uuid),
               (SELECT concat(author_action_sequence, '/', authoritative_commit_sequence,
                              '/', project_activity_position) FROM storyos.scope_counters
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
               (SELECT concat(outcome_kind, '/', COALESCE(result_reference::text, ''))
                  FROM storyos.command_idempotency
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid AND command_kind = 'applyAuthorEdit' AND idempotency_key = $3::text::uuid))::text",
            &[&owner, &project, &command.challenge_binding.idempotency_key, &command.chapter_id],
        )
        .await
        .unwrap()
        .get(0)
}

async fn set_sequence(admin: &tokio_postgres::Client, observation_id: &str, sequence: &str) {
    admin
        .execute(
            "UPDATE storyos.author_command_admission_outcome_unknown_observations
                SET observation_sequence = $4::text::numeric
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND observation_id = $3::text::uuid",
            &[&USER, &PROJECT, &observation_id, &sequence],
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn append_is_exact_serialized_and_has_zero_authority_effect() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let database_url = std::env::var("STORYOS_TEST_DATABASE_URL").unwrap();
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL").unwrap();
    let store = PostgresProjectReader::new(&database_url);
    let scope = ProjectScope::new(UserId::new(USER), ProjectId::new(PROJECT));
    let session_key = "018f0000-0000-7001-8000-000000000801";
    let mut session_binding = binding(
        &scope,
        "createEditorSession",
        "/api/v1/projects/{project_id}/editor-sessions",
        "storyos.command.create-editor-session.request.v1",
        session_key,
    );
    session_binding.client_session_generation = 81;
    let session_nonce_digest = "sha256:unknown-session-81";
    issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: session_binding.clone(),
            nonce: "unknown-session-nonce-81".to_owned(),
            nonce_digest: session_nonce_digest.to_owned(),
        },
    )
    .await
    .unwrap();
    let editor_session_id = "018f0000-0000-7001-8000-000000000802";
    create_editor_session(
        &store,
        &OpenEditorSession {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(editor_session_id),
            snapshot_id: "018f0000-0000-7001-8000-000000000803".to_owned(),
            client_binding: EditorClientBinding {
                binding_ref: "binding:author-edit".to_owned(),
                session_generation: 81,
                client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
                security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
            },
            challenge_binding: session_binding,
            nonce_digest: session_nonce_digest.to_owned(),
        },
    )
    .await
    .unwrap();
    let mut command = author_command(
        &scope,
        editor_session_id,
        "018f0000-0000-7001-8000-000000000811",
        "sha256:unknown-command-81",
        "81",
    );
    command.client_binding.session_generation = 81;
    command.challenge_binding.client_session_generation = 81;
    issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: command.challenge_binding.clone(),
            nonce: "unknown-command-nonce-81".to_owned(),
            nonce_digest: command.nonce_digest.clone(),
        },
    )
    .await
    .unwrap();
    store
        .apply_author_edit_with_fault(&command, AuthorEditFault::AfterAdmissionBeforeCore)
        .await
        .unwrap_err();
    let (admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        let _connection_result = connection.await;
    });
    let state_before = command_state(&admin, &command).await;
    let append = AppendAuthorCommandOutcomeUnknown {
        project_scope: scope.clone(),
        author_command_admission_id: command.ids.author_command_admission_id.clone(),
        observation_id: "018f0000-0000-7001-8000-000000000813".to_owned(),
        last_provable_boundary: AuthorCommandOutcomeUnknownBoundary::AdmissionCommitted,
        reason: AuthorCommandOutcomeUnknownReason::AcknowledgementMissing,
    };
    let names = [
        "storyos_unknown_exact_probe_81_a".to_owned(),
        "storyos_unknown_exact_probe_81_b".to_owned(),
    ];
    let (blocker, blocker_pid) =
        lock_arbiter(&store, &scope, &command.challenge_binding.idempotency_key).await;
    let separator = if database_url.contains('?') { '&' } else { '?' };
    let exact_store = |name: &str| {
        PostgresProjectReader::new(format!("{database_url}{separator}application_name={name}"))
    };
    let append_a = append.clone();
    let append_b = append.clone();
    let store_a = exact_store(&names[0]);
    let store_b = exact_store(&names[1]);
    let first_task =
        tokio::spawn(
            async move { append_author_command_outcome_unknown(&store_a, &append_a).await },
        );
    let retry_task =
        tokio::spawn(
            async move { append_author_command_outcome_unknown(&store_b, &append_b).await },
        );
    wait_for_blocked(&admin, &names, blocker_pid).await;
    blocker.rollback().await.unwrap();
    let first = first_task.await.unwrap().unwrap();
    assert_eq!(retry_task.await.unwrap().unwrap(), first);
    assert_eq!(
        first,
        AuthorCommandOutcomeUnknownObservation {
            project_scope: scope.clone(),
            observation_id: append.observation_id.clone(),
            observation_sequence: 1,
            author_command_admission_id: command.ids.author_command_admission_id.clone(),
            command_id: command.ids.command_id.clone(),
            command_kind: "applyAuthorEdit".to_owned(),
            canonical_command_digest: command.challenge_binding.canonical_command_digest.clone(),
            idempotency_key: command.challenge_binding.idempotency_key.clone(),
            last_provable_boundary: AuthorCommandOutcomeUnknownBoundary::AdmissionCommitted,
            reason: AuthorCommandOutcomeUnknownReason::AcknowledgementMissing,
            reconciliation_required: ReconciliationRequired,
            observed_at: first.observed_at.clone(),
        }
    );
    let mut second_append = append.clone();
    second_append.observation_id = "018f0000-0000-7001-8000-000000000814".to_owned();
    let second = append_author_command_outcome_unknown(&store, &second_append)
        .await
        .unwrap();
    assert_eq!(
        second,
        AuthorCommandOutcomeUnknownObservation {
            observation_id: second_append.observation_id.clone(),
            observation_sequence: 2,
            observed_at: second.observed_at.clone(),
            ..first.clone()
        }
    );
    let mut mismatch = append.clone();
    mismatch.author_command_admission_id = "018f0000-0000-7001-8000-000000000899".to_owned();
    let mut foreign = append.clone();
    foreign.project_scope = ProjectScope::new(
        UserId::new("018f0000-0000-7001-8000-000000000101"),
        ProjectId::new("018f0000-0000-7001-8000-000000000102"),
    );
    for invalid in [&mismatch, &foreign] {
        assert!(matches!(
            append_author_command_outcome_unknown(&store, invalid).await,
            Err(AuthorCommandOutcomeUnknownError::BindingConflict)
        ));
    }
    assert_eq!(command_state(&admin, &command).await, state_before);

    set_sequence(&admin, &second.observation_id, "18446744073709551615").await;
    let mut new_append = append.clone();
    new_append.observation_id = "018f0000-0000-7001-8000-000000000815".to_owned();
    assert!(
        append_author_command_outcome_unknown(&store, &new_append)
            .await
            .is_err()
    );
    assert_eq!(
        append_author_command_outcome_unknown(&store, &append)
            .await
            .unwrap(),
        first
    );
    set_sequence(&admin, &second.observation_id, "2").await;
    let (settlement, blocker_pid) =
        lock_arbiter(&store, &scope, &command.challenge_binding.idempotency_key).await;
    let names = ["storyos_unknown_settlement_probe_81".to_owned()];
    let blocked_store = exact_store(&names[0]);
    let blocked_append = new_append.clone();
    let blocked = tokio::spawn(async move {
        append_author_command_outcome_unknown(&blocked_store, &blocked_append).await
    });
    wait_for_blocked(&admin, &names, blocker_pid).await;
    let receipt = crate::author_edit_settlement::persist_author_edit_settlement(
        &settlement.client,
        &command,
        "018f0000-0000-7001-8000-000000000005",
        crate::author_edit::ClassifiedAuthorEdit {
            result: storyos_core::ApplyAuthorEditResult::NoEffect {
                reason: storyos_core::AuthorEditNoEffect::ContentUnchanged,
            },
            successor_blocks: None,
        },
    )
    .await
    .unwrap();
    settlement.commit().await.unwrap();
    assert!(matches!(
        blocked.await.unwrap(),
        Err(AuthorCommandOutcomeUnknownError::BindingConflict)
    ));
    assert_eq!(
        append_author_command_outcome_unknown(&store, &append)
            .await
            .unwrap(),
        first
    );
    let counts: Vec<i64> = admin
        .query_one(
            "SELECT ARRAY[
               (SELECT count(*) FROM storyos.author_command_admission_outcome_unknown_observations
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
               (SELECT count(*) FROM storyos.author_command_admission_settlements WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
               (SELECT count(*) FROM storyos.domain_receipts WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
               (SELECT count(*) FROM storyos.project_activity_events WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
               (SELECT count(*) FROM storyos.authoritative_revision_envelopes WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
               (SELECT count(*) FROM storyos.authoritative_commits WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid),
               (SELECT count(*) FROM storyos.author_action_entries WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid)]::bigint[]",
            &[&USER, &PROJECT],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(counts, vec![2, 1, 1, 0, 0, 0, 0]);
    admin
        .batch_execute(&format!(
            "BEGIN; SET CONSTRAINTS ALL DEFERRED;
             DELETE FROM storyos.author_command_admission_outcome_unknown_observations WHERE owner_user_id = '{USER}'::uuid AND project_id = '{PROJECT}'::uuid AND author_command_admission_id = '{admission}'::uuid;
             DELETE FROM storyos.author_command_admission_settlements WHERE owner_user_id = '{USER}'::uuid AND project_id = '{PROJECT}'::uuid AND author_command_admission_id = '{admission}'::uuid;
             DELETE FROM storyos.domain_receipts WHERE owner_user_id = '{USER}'::uuid AND project_id = '{PROJECT}'::uuid AND receipt_id = '{receipt}'::uuid;
             DELETE FROM storyos.author_command_admissions WHERE owner_user_id = '{USER}'::uuid AND project_id = '{PROJECT}'::uuid AND author_command_admission_id = '{admission}'::uuid;
             DELETE FROM storyos.editor_session_base_snapshots WHERE owner_user_id = '{USER}'::uuid AND project_id = '{PROJECT}'::uuid AND editor_session_id = '{session}'::uuid;
             DELETE FROM storyos.project_writer_generations WHERE owner_user_id = '{USER}'::uuid AND project_id = '{PROJECT}'::uuid AND current_editor_session_id = '{session}'::uuid;
             DELETE FROM storyos.editor_sessions WHERE owner_user_id = '{USER}'::uuid AND project_id = '{PROJECT}'::uuid AND editor_session_id = '{session}'::uuid;
             DELETE FROM storyos.project_command_challenges WHERE owner_user_id = '{USER}'::uuid AND project_id = '{PROJECT}'::uuid AND idempotency_key IN ('{session_key}'::uuid, '{command_key}'::uuid);
             DELETE FROM storyos.command_idempotency WHERE owner_user_id = '{USER}'::uuid AND project_id = '{PROJECT}'::uuid AND idempotency_key IN ('{session_key}'::uuid, '{command_key}'::uuid);
             DELETE FROM storyos.project_command_challenge_rate_windows WHERE owner_user_id = '{USER}'::uuid AND project_id = '{PROJECT}'::uuid AND client_session_generation = 81;
             DELETE FROM storyos.project_command_challenge_rate_guards WHERE owner_user_id = '{USER}'::uuid AND project_id = '{PROJECT}'::uuid AND client_session_generation = 81;
             DELETE FROM storyos.scope_counters WHERE owner_user_id = '{USER}'::uuid AND project_id = '{PROJECT}'::uuid;
             COMMIT;",
            admission = command.ids.author_command_admission_id,
            receipt = receipt.ids.receipt_id,
            session = editor_session_id,
            command_key = command.challenge_binding.idempotency_key,
        ))
        .await
        .unwrap();
}
