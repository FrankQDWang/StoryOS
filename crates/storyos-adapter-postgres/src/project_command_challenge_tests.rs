use std::time::Duration;

use super::*;
use storyos_application::{UserId, issue_project_command_challenge};

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT_A: &str = "018f0000-0000-7001-8000-000000000002";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const PROJECT_B: &str = "018f0000-0000-7001-8000-000000000102";

fn fixed_clock_store(database_url: String, unix_seconds: i64) -> PostgresProjectReader {
    PostgresProjectReader {
        database_url,
        challenge_rate_clock_unix_seconds: Some(unix_seconds),
        readable_export_lease_ttl: Duration::from_secs(30),
    }
}

fn numbered_request(index: u64, generation: u64) -> IssueProjectCommandChallenge {
    IssueProjectCommandChallenge {
        binding: ProjectCommandChallengeBinding {
            project_scope: ProjectScope::new(UserId::new(USER_A), ProjectId::new(PROJECT_A)),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: generation,
            client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
            security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            challenge_rate_policy_revision:
                "storyos.project-command-challenge-rate.fixed-window.v1".to_owned(),
            method: "PATCH".to_owned(),
            route_template: "/api/v1/projects/{project_id}".to_owned(),
            command_schema: "storyos.command.update-project.request.v1".to_owned(),
            command_kind: "updateProject".to_owned(),
            canonical_command_digest: format!(
                "sha256:storyos.command.updateProject.jcs.v1:{}",
                char::from_digit((index % 16) as u32, 16)
                    .unwrap()
                    .to_string()
                    .repeat(64)
            ),
            idempotency_key: format!("018f0000-0000-7001-8000-{index:012}"),
        },
        nonce: format!("opaque-numbered-nonce-{index}"),
        nonce_digest: format!("sha256:numbered-nonce-{index}"),
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn fixed_window_capacity_and_rollover_are_deterministic_under_concurrency() {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let last_second = fixed_clock_store(runtime_url.clone(), 119);
    let mut attempts = tokio::task::JoinSet::new();
    for index in 701..=711 {
        let store = last_second.clone();
        attempts.spawn(async move {
            (
                index,
                issue_project_command_challenge(&store, &numbered_request(index, 701)).await,
            )
        });
    }

    let mut accepted = Vec::new();
    let mut refused = Vec::new();
    while let Some(result) = attempts.join_next().await {
        let (index, result) = result.unwrap();
        match result {
            Ok(_) => accepted.push(index),
            Err(ProjectCommandChallengeError::RateLimited {
                retry_after_seconds,
            }) => {
                assert_eq!(retry_after_seconds, 1);
                refused.push(index);
            }
            Err(error) => panic!("unexpected challenge result: {error}"),
        }
    }
    assert_eq!(accepted.len(), 10);
    assert_eq!(refused.len(), 1);

    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    let counts = admin
        .query_one(
            "SELECT
               (SELECT count(*) FROM storyos.command_idempotency
                WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                  AND command_kind = 'updateProject'
                  AND idempotency_key::text LIKE '018f0000-0000-7001-8000-0000000007%'),
               (SELECT count(*) FROM storyos.project_command_challenges
                WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                  AND client_session_generation = 701),
               (SELECT issued_count FROM storyos.project_command_challenge_rate_windows
                WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                  AND client_session_generation = 701
                  AND window_started_at = to_timestamp(60))",
            &[&USER_A, &PROJECT_A],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            counts.get::<_, i64>(0),
            counts.get::<_, i64>(1),
            counts.get::<_, i16>(2)
        ),
        (10, 10, 10)
    );

    let next_window = fixed_clock_store(runtime_url, 120);
    issue_project_command_challenge(&next_window, &numbered_request(refused[0], 701))
        .await
        .unwrap();
    let windows = admin
        .query(
            "SELECT extract(epoch FROM window_started_at)::bigint, issued_count
             FROM storyos.project_command_challenge_rate_windows
             WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
               AND client_session_generation = 701
             ORDER BY window_started_at",
            &[&USER_A, &PROJECT_A],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.get::<_, i64>(0), row.get::<_, i16>(1)))
        .collect::<Vec<_>>();
    assert_eq!(windows, vec![(60, 10), (120, 1)]);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn exact_retry_uses_one_rate_unit_and_rate_rows_obey_forced_rls() {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = fixed_clock_store(runtime_url.clone(), 180);
    let request = numbered_request(801, 801);
    let (left, right) = tokio::join!(
        issue_project_command_challenge(&store, &request),
        issue_project_command_challenge(&store, &request)
    );
    assert_eq!(left.unwrap(), right.unwrap());

    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    assert_eq!(
        admin
            .query_one(
                "SELECT issued_count FROM storyos.project_command_challenge_rate_windows
                 WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                   AND client_session_generation = 801",
                &[&USER_A, &PROJECT_A],
            )
            .await
            .unwrap()
            .get::<_, i16>(0),
        1
    );

    let (mut runtime, connection) = tokio_postgres::connect(&runtime_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    let foreign_scope = runtime.transaction().await.unwrap();
    foreign_scope
        .execute(
            "SELECT set_config('storyos.user_id', $1, true),
                    set_config('storyos.owner_user_id', $1, true),
                    set_config('storyos.project_id', $2, true)",
            &[&USER_B, &PROJECT_B],
        )
        .await
        .unwrap();
    let hidden_rate_rows = foreign_scope
        .query_one(
            "SELECT
               (SELECT count(*) FROM storyos.project_command_challenge_rate_guards),
               (SELECT count(*) FROM storyos.project_command_challenge_rate_windows)",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            hidden_rate_rows.get::<_, i64>(0),
            hidden_rate_rows.get::<_, i64>(1)
        ),
        (0, 0)
    );
}
