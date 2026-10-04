use super::*;
use storyos_application::{UserId, issue_project_command_challenge};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT_A: &str = "018f0000-0000-7001-8000-000000000002";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const PROJECT_B: &str = "018f0000-0000-7001-8000-000000000102";

fn fixed_clock_store(database_url: String, unix_seconds: i64) -> PostgresProjectReader {
    PostgresProjectReader {
        challenge_rate_clock_unix_seconds: Some(unix_seconds),
        ..PostgresProjectReader::new(database_url)
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
               (SELECT count(*) FROM storyos.project_command_challenge_rate_guards
                WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                  AND client_session_generation = 801),
               (SELECT count(*) FROM storyos.project_command_challenge_rate_windows
                WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                  AND client_session_generation = 801)",
            &[&USER_A, &PROJECT_A],
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

fn author_edit_request(index: u64, generation: u64) -> IssueProjectCommandChallenge {
    let mut request = numbered_request(index, generation);
    let binding = &mut request.binding;
    binding.challenge_rate_policy_revision =
        "storyos.project-command-challenge-rate.author-edit.fixed-window.v1".to_owned();
    binding.method = "POST".to_owned();
    binding.route_template = "/api/v1/projects/{project_id}/manuscript/author-edits".to_owned();
    binding.command_schema = "storyos.command.apply-author-edit.request.v1".to_owned();
    binding.command_kind = "applyAuthorEdit".to_owned();
    binding.canonical_command_digest = binding
        .canonical_command_digest
        .replace("updateProject", "applyAuthorEdit");
    request
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_edit_and_shared_challenges_use_separate_rate_budgets() {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = fixed_clock_store(runtime_url, /*unix_seconds*/ 30_015);
    let mut shared = Vec::new();
    for index in 8_901..=8_911 {
        shared.push(
            issue_project_command_challenge(&store, &numbered_request(index, /*generation*/ 901))
                .await,
        );
    }
    let mut author_edits = Vec::new();
    for index in 9_001..=9_121 {
        author_edits.push(
            issue_project_command_challenge(
                &store,
                &author_edit_request(index, /*generation*/ 901),
            )
            .await,
        );
    }
    let shared_after_author_edits = issue_project_command_challenge(
        &store,
        &numbered_request(/*index*/ 8_912, /*generation*/ 901),
    )
    .await;

    let admitted = |results: &[Result<_, ProjectCommandChallengeError>]| {
        results.iter().filter(|result| result.is_ok()).count()
    };
    let retry_after = |result: &Result<_, ProjectCommandChallengeError>| match result {
        Err(ProjectCommandChallengeError::RateLimited {
            retry_after_seconds,
        }) => Some(*retry_after_seconds),
        Ok(_) | Err(_) => None,
    };
    assert_eq!(
        (
            admitted(&shared),
            retry_after(shared.last().unwrap()),
            admitted(&author_edits),
            retry_after(author_edits.last().unwrap()),
            retry_after(&shared_after_author_edits),
        ),
        (10, Some(45), 120, Some(45), Some(45))
    );
}

fn undo_request(index: u64, generation: u64) -> IssueProjectCommandChallenge {
    let mut request = numbered_request(index, generation);
    let binding = &mut request.binding;
    binding.command_kind = "undoLatestAuthorAction".to_owned();
    binding.challenge_rate_policy_revision =
        ChallengeRateClass::for_command_kind(&binding.command_kind)
            .policy_revision()
            .to_owned();
    binding.method = "POST".to_owned();
    binding.route_template = "/api/v1/projects/{project_id}/author-actions/undo".to_owned();
    binding.command_schema = "storyos.command.undo-latest-author-action.request.v1".to_owned();
    binding.canonical_command_digest = binding
        .canonical_command_digest
        .replace("updateProject", "undoLatestAuthorAction");
    request
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_and_author_edit_share_the_author_edit_rate_budget_for_one_project() {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = fixed_clock_store(runtime_url, /*unix_seconds*/ 30_015);
    let mut shared = Vec::new();
    for index in 9_401..=9_410 {
        shared.push(
            issue_project_command_challenge(&store, &numbered_request(index, /*generation*/ 902))
                .await,
        );
    }
    let mut writing = Vec::new();
    for index in 0..60 {
        writing.push(
            issue_project_command_challenge(
                &store,
                &author_edit_request(9_501 + index, /*generation*/ 902),
            )
            .await,
        );
        writing.push(
            issue_project_command_challenge(
                &store,
                &undo_request(9_601 + index, /*generation*/ 902),
            )
            .await,
        );
    }
    let undo_over_capacity =
        issue_project_command_challenge(&store, &undo_request(/*index*/ 9_661, /*generation*/ 902))
            .await;
    let exact_retry =
        issue_project_command_challenge(&store, &undo_request(/*index*/ 9_601, /*generation*/ 902))
            .await;
    let mut other_project = undo_request(/*index*/ 9_701, /*generation*/ 902);
    other_project.binding.project_scope =
        ProjectScope::new(UserId::new(USER_B), ProjectId::new(PROJECT_B));
    let other_project = issue_project_command_challenge(&store, &other_project).await;

    let admitted = |results: &[Result<_, ProjectCommandChallengeError>]| {
        results.iter().filter(|result| result.is_ok()).count()
    };
    let retry_after = |result: &Result<_, ProjectCommandChallengeError>| match result {
        Err(ProjectCommandChallengeError::RateLimited {
            retry_after_seconds,
        }) => Some(*retry_after_seconds),
        Ok(_) | Err(_) => None,
    };
    assert_eq!(
        (
            admitted(&shared),
            admitted(&writing),
            retry_after(&undo_over_capacity),
            exact_retry.as_ref().ok(),
            other_project.is_ok(),
        ),
        (10, 120, Some(45), writing[1].as_ref().ok(), true)
    );
}
