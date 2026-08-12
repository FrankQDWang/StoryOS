use storyos_adapter_postgres::PostgresProjectReader;
use storyos_application::{
    IssueProjectCommandChallenge, ProjectCommandChallengeBinding, ProjectCommandChallengeError,
    ProjectCommandChallengeUse, ProjectId, ProjectScope, UserId, consume_project_command_challenge,
    issue_project_command_challenge,
};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT_A: &str = "018f0000-0000-7001-8000-000000000002";

fn numbered_challenge_request(index: u64, session_generation: u64) -> IssueProjectCommandChallenge {
    let digest_byte = char::from_digit((index % 16) as u32, 16).unwrap();
    let mut request = challenge_request(digest_byte);
    request.binding.client_session_generation = session_generation;
    request.binding.idempotency_key = format!("018f0000-0000-7001-8000-{index:012}");
    request.nonce = format!("opaque-numbered-nonce-{index}");
    request.nonce_digest = format!("sha256:numbered-nonce-{index}");
    request
}

fn challenge_request(digest_byte: char) -> IssueProjectCommandChallenge {
    let canonical_command_digest = format!(
        "sha256:storyos.command.updateProject.jcs.v1:{}",
        digest_byte.to_string().repeat(64)
    );
    let idempotency_key = if digest_byte == 'a' {
        "018f0000-0000-7001-8000-000000000004"
    } else {
        "018f0000-0000-7001-8000-000000000005"
    };
    IssueProjectCommandChallenge {
        binding: ProjectCommandChallengeBinding {
            project_scope: ProjectScope::new(UserId::new(USER_A), ProjectId::new(PROJECT_A)),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: 7,
            client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
            security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            challenge_rate_policy_revision:
                "storyos.project-command-challenge-rate.fixed-window.v1".to_owned(),
            method: "PATCH".to_owned(),
            route_template: "/api/v1/projects/{project_id}".to_owned(),
            command_schema: "storyos.command.update-project.request.v1".to_owned(),
            command_kind: "updateProject".to_owned(),
            canonical_command_digest,
            idempotency_key: idempotency_key.to_owned(),
        },
        nonce: format!("opaque-nonce-{digest_byte}"),
        nonce_digest: format!("sha256:nonce-{digest_byte}"),
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn issue_and_consume_are_exact_retry_safe_under_forced_rls() {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let request = challenge_request('a');

    let first = issue_project_command_challenge(&store, &request)
        .await
        .unwrap();
    let retry = issue_project_command_challenge(&store, &request)
        .await
        .unwrap();
    assert_eq!(first, retry);
    assert_eq!(first.nonce, "opaque-nonce-a");
    let mut first_transaction = store
        .begin_project_command_transaction(&request.binding.project_scope)
        .await
        .unwrap();
    assert_eq!(
        consume_project_command_challenge(
            &mut first_transaction,
            &request.binding,
            &request.nonce_digest
        )
        .await
        .unwrap(),
        ProjectCommandChallengeUse::FirstUse,
    );
    first_transaction.commit().await.unwrap();
    let mut retry_transaction = store
        .begin_project_command_transaction(&request.binding.project_scope)
        .await
        .unwrap();
    assert_eq!(
        consume_project_command_challenge(
            &mut retry_transaction,
            &request.binding,
            &request.nonce_digest
        )
        .await
        .unwrap(),
        ProjectCommandChallengeUse::ExactRetryInProgress,
    );
    retry_transaction.rollback().await.unwrap();
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    admin
        .execute(
            "UPDATE storyos.project_command_challenges SET expires_at = clock_timestamp() - interval '1 second'
             WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
               AND command_kind = $3 AND idempotency_key = $4::text::uuid",
            &[&USER_A, &PROJECT_A, &request.binding.command_kind, &request.binding.idempotency_key],
        )
        .await
        .unwrap();
    admin
        .execute(
            "UPDATE storyos.command_idempotency
             SET outcome_kind = 'settled', result_reference = 'receipt:018f0000-0000-7001-8000-000000000099'
             WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
               AND command_kind = $3 AND idempotency_key = $4::text::uuid",
            &[&USER_A, &PROJECT_A, &request.binding.command_kind, &request.binding.idempotency_key],
        )
        .await
        .unwrap();
    let mut settled_transaction = store
        .begin_project_command_transaction(&request.binding.project_scope)
        .await
        .unwrap();
    assert_eq!(
        consume_project_command_challenge(
            &mut settled_transaction,
            &request.binding,
            &request.nonce_digest
        )
        .await
        .unwrap(),
        ProjectCommandChallengeUse::ExactRetrySettled {
            result_reference: "receipt:018f0000-0000-7001-8000-000000000099".to_owned(),
        },
    );
    settled_transaction.rollback().await.unwrap();

    let nonce_columns = admin
        .query(
            "SELECT column_name FROM information_schema.columns
             WHERE table_schema = 'storyos' AND table_name = 'project_command_challenges'
               AND column_name IN ('nonce', 'nonce_digest') ORDER BY column_name",
            &[],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get::<_, String>(0))
        .collect::<Vec<_>>();
    assert_eq!(nonce_columns, vec!["nonce_digest"]);
    let durable_rate_binding = admin
        .query_one(
            "SELECT challenge.challenge_rate_policy_revision,
                    challenge.challenge_rate_window_started_at = rate.window_started_at
             FROM storyos.project_command_challenges AS challenge
             JOIN storyos.project_command_challenge_rate_windows AS rate
               ON rate.owner_user_id = challenge.owner_user_id
              AND rate.project_id = challenge.project_id
              AND rate.client_session_generation = challenge.client_session_generation
              AND rate.policy_revision = challenge.challenge_rate_policy_revision
              AND rate.window_started_at = challenge.challenge_rate_window_started_at
             WHERE challenge.owner_user_id = $1::text::uuid
               AND challenge.project_id = $2::text::uuid
               AND challenge.command_kind = $3
               AND challenge.idempotency_key = $4::text::uuid",
            &[
                &USER_A,
                &PROJECT_A,
                &request.binding.command_kind,
                &request.binding.idempotency_key,
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        durable_rate_binding.get::<_, String>(0),
        "storyos.project-command-challenge-rate.fixed-window.v1"
    );
    assert!(durable_rate_binding.get::<_, bool>(1));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn changed_binding_and_wrong_scope_fail_closed_without_consumption() {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let request = numbered_challenge_request(201, 201);
    issue_project_command_challenge(&store, &request)
        .await
        .unwrap();

    let mut changed_bindings = Vec::new();
    for mutate in [
        |value: &mut ProjectCommandChallengeBinding| {
            value.client_contract_revision = "storyos.web-client.release-0.v1".to_owned()
        },
        |value: &mut ProjectCommandChallengeBinding| {
            value.client_session_binding_digest = "sha256:session-stolen".to_owned()
        },
        |value: &mut ProjectCommandChallengeBinding| value.client_session_generation += 1,
        |value: &mut ProjectCommandChallengeBinding| {
            value.security_policy_revision = "storyos.web-security-policy.release-0.v1".to_owned()
        },
        |value: &mut ProjectCommandChallengeBinding| {
            value.limit_profile_revision = "storyos.foundation.absolute.v0".to_owned()
        },
        |value: &mut ProjectCommandChallengeBinding| {
            value.challenge_rate_policy_revision =
                "storyos.project-command-challenge-rate.fixed-window.v0".to_owned()
        },
        |value: &mut ProjectCommandChallengeBinding| value.method = "POST".to_owned(),
        |value: &mut ProjectCommandChallengeBinding| {
            value.route_template = "/api/v1/projects/{project_id}/archival".to_owned()
        },
        |value: &mut ProjectCommandChallengeBinding| {
            value.command_schema = "storyos.command.archive-project.request.v1".to_owned()
        },
        |value: &mut ProjectCommandChallengeBinding| {
            value.command_kind = "archiveProject".to_owned()
        },
        |value: &mut ProjectCommandChallengeBinding| {
            value.canonical_command_digest = format!(
                "sha256:storyos.command.updateProject.jcs.v1:{}",
                "f".repeat(64)
            )
        },
        |value: &mut ProjectCommandChallengeBinding| {
            value.idempotency_key = "018f0000-0000-7001-8000-000000000099".to_owned()
        },
    ] {
        let mut changed = request.binding.clone();
        mutate(&mut changed);
        changed_bindings.push(changed);
    }
    for changed_binding in changed_bindings {
        let mut transaction = store
            .begin_project_command_transaction(&changed_binding.project_scope)
            .await
            .unwrap();
        assert!(matches!(
            consume_project_command_challenge(
                &mut transaction,
                &changed_binding,
                &request.nonce_digest
            )
            .await,
            Err(ProjectCommandChallengeError::InvalidOrExpired)
        ));
        transaction.rollback().await.unwrap();
    }

    let mut wrong_nonce = store
        .begin_project_command_transaction(&request.binding.project_scope)
        .await
        .unwrap();
    assert!(matches!(
        consume_project_command_challenge(
            &mut wrong_nonce,
            &request.binding,
            "sha256:nonce-from-another-record"
        )
        .await,
        Err(ProjectCommandChallengeError::InvalidOrExpired)
    ));
    wrong_nonce.rollback().await.unwrap();

    let mut wrong_scope = request.binding.clone();
    wrong_scope.project_scope = ProjectScope::new(
        UserId::new("018f0000-0000-7001-8000-000000000101"),
        ProjectId::new("018f0000-0000-7001-8000-000000000102"),
    );
    let mut wrong_scope_transaction = store
        .begin_project_command_transaction(&wrong_scope.project_scope)
        .await
        .unwrap();
    assert!(matches!(
        consume_project_command_challenge(
            &mut wrong_scope_transaction,
            &wrong_scope,
            &request.nonce_digest
        )
        .await,
        Err(ProjectCommandChallengeError::InvalidOrExpired)
    ));
    wrong_scope_transaction.rollback().await.unwrap();

    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    let unchanged = admin
        .query_one(
            "SELECT challenge.consumed_at IS NULL, idempotency.outcome_kind
             FROM storyos.project_command_challenges AS challenge
             JOIN storyos.command_idempotency AS idempotency USING
               (owner_user_id, project_id, command_kind, idempotency_key)
             WHERE challenge.owner_user_id = $1::text::uuid
               AND challenge.project_id = $2::text::uuid
               AND challenge.command_kind = $3
               AND challenge.idempotency_key = $4::text::uuid",
            &[
                &USER_A,
                &PROJECT_A,
                &request.binding.command_kind,
                &request.binding.idempotency_key,
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        (unchanged.get::<_, bool>(0), unchanged.get::<_, String>(1)),
        (true, "pending".to_owned())
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn consumption_rolls_back_with_the_caller_owned_transaction() {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let request = numbered_challenge_request(301, 301);
    issue_project_command_challenge(&store, &request)
        .await
        .unwrap();

    let mut rolled_back = store
        .begin_project_command_transaction(&request.binding.project_scope)
        .await
        .unwrap();
    assert_eq!(
        consume_project_command_challenge(
            &mut rolled_back,
            &request.binding,
            &request.nonce_digest
        )
        .await
        .unwrap(),
        ProjectCommandChallengeUse::FirstUse,
    );
    rolled_back.rollback().await.unwrap();

    let mut committed = store
        .begin_project_command_transaction(&request.binding.project_scope)
        .await
        .unwrap();
    assert_eq!(
        consume_project_command_challenge(&mut committed, &request.binding, &request.nonce_digest)
            .await
            .unwrap(),
        ProjectCommandChallengeUse::FirstUse,
    );
    committed.commit().await.unwrap();
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_expired_unconsumed_challenge_cannot_enter_the_command_transaction() {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let request = numbered_challenge_request(601, 601);
    issue_project_command_challenge(&store, &request)
        .await
        .unwrap();
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    admin
        .execute(
            "UPDATE storyos.project_command_challenges SET expires_at = clock_timestamp() - interval '1 second'
             WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
               AND command_kind = $3 AND idempotency_key = $4::text::uuid",
            &[
                &USER_A,
                &PROJECT_A,
                &request.binding.command_kind,
                &request.binding.idempotency_key,
            ],
        )
        .await
        .unwrap();

    let mut transaction = store
        .begin_project_command_transaction(&request.binding.project_scope)
        .await
        .unwrap();
    assert!(matches!(
        consume_project_command_challenge(
            &mut transaction,
            &request.binding,
            &request.nonce_digest
        )
        .await,
        Err(ProjectCommandChallengeError::InvalidOrExpired)
    ));
    transaction.rollback().await.unwrap();
}
