use storyos_application::{UserId, issue_project_command_challenge};
use storyos_contracts::CommandKind;
use tokio_postgres::NoTls;

use super::*;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::message_counting_proxy::{SentMessages, message_counting_proxy};

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT_A: &str = "018f0000-0000-7001-8000-000000000002";
const PROJECT_B_OF_USER_B: &str = "018f0000-0000-7001-8000-000000000102";
const MISSING_PROJECT: &str = "018f0000-0000-7001-8000-0000001901ff";
const SESSION_GENERATION: u64 = 1_901;

fn store(database_url: String) -> PostgresProjectReader {
    PostgresProjectReader {
        challenge_rate_clock_unix_seconds: Some(1_901_000),
        ..PostgresProjectReader::new(database_url)
    }
}

fn request(project_id: &str, index: u64) -> IssueProjectCommandChallenge {
    IssueProjectCommandChallenge {
        binding: ProjectCommandChallengeBinding {
            project_scope: ProjectScope::new(UserId::new(USER_A), ProjectId::new(project_id)),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: SESSION_GENERATION,
            client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
            security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            challenge_rate_policy_revision:
                "storyos.project-command-challenge-rate.fixed-window.v1".to_owned(),
            method: "PATCH".to_owned(),
            route_template: "/api/v1/projects/{project_id}".to_owned(),
            command_schema: "storyos.command.update-project.request.v1".to_owned(),
            command_kind: CommandKind::UpdateProject.as_str().to_owned(),
            canonical_command_digest: format!(
                "sha256:storyos.command.updateProject.jcs.v1:{}",
                "a".repeat(64)
            ),
            idempotency_key: format!("018f0000-0000-7001-8000-0019010{index:05}"),
        },
        nonce: format!("opaque-project-proof-nonce-{index}"),
        nonce_digest: format!("sha256:project-proof-nonce-{index}"),
        earlier_revision_nonces: Vec::new(),
    }
}

/// The store calls of `create_project_command_challenge`: the Storage Activation proof read and
/// the issue. The separate Project read that the handler sent before the issue added 4.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_store_work_of_one_challenge_on_a_warm_connection_is_14_round_trips() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (proxy_url, messages) = message_counting_proxy(
        &std::env::var("STORYOS_TEST_DATABASE_URL")
            .expect("run through scripts/verify-project-scope.sh"),
    );
    let store = store(proxy_url);
    store
        .require_release1_storage_activation_proof()
        .await
        .unwrap();
    issue_project_command_challenge(&store, &request(PROJECT_A, 1))
        .await
        .unwrap();
    *messages.lock().unwrap() = SentMessages::default();

    store
        .require_release1_storage_activation_proof()
        .await
        .unwrap();
    issue_project_command_challenge(&store, &request(PROJECT_A, 2))
        .await
        .unwrap();

    assert_eq!(
        *messages.lock().unwrap(),
        SentMessages {
            round_trips: 14,
            simple_queries: 2,
            parses: 0,
            executes: 12,
        }
    );
}

/// The rows that an issue for one of these Projects can write, of each owner.
async fn challenge_rows(admin: &crate::statement_cache::Client) -> Vec<String> {
    admin
        .query(
            "SELECT 'idempotency ' || row_to_json(row)::text
               FROM storyos.command_idempotency AS row WHERE project_id = ANY($1::text[]::uuid[])
             UNION ALL
             SELECT 'challenge ' || row_to_json(row)::text
               FROM storyos.project_command_challenges AS row
              WHERE project_id = ANY($1::text[]::uuid[])
             UNION ALL
             SELECT 'rate guard ' || row_to_json(row)::text
               FROM storyos.project_command_challenge_rate_guards AS row
              WHERE project_id = ANY($1::text[]::uuid[])
             UNION ALL
             SELECT 'rate window ' || row_to_json(row)::text
               FROM storyos.project_command_challenge_rate_windows AS row
              WHERE project_id = ANY($1::text[]::uuid[])
             ORDER BY 1",
            &[&vec![PROJECT_B_OF_USER_B, MISSING_PROJECT]],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(0))
        .collect()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_issue_refuses_a_missing_project_and_a_project_of_another_user_with_no_row() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let store = store(
        std::env::var("STORYOS_TEST_DATABASE_URL")
            .expect("run through scripts/verify-project-scope.sh"),
    );
    let (admin, connection) = tokio_postgres::connect(
        &std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
            .expect("run through scripts/verify-project-scope.sh"),
        NoTls,
    )
    .await
    .unwrap();
    let admin = crate::statement_cache::Client::new(admin);
    tokio::spawn(async move { connection.await.unwrap() });
    let rows_before = challenge_rows(&admin).await;

    let mut refusals = Vec::new();
    for (index, project_id) in [(3, MISSING_PROJECT), (4, PROJECT_B_OF_USER_B)] {
        let result = issue_project_command_challenge(&store, &request(project_id, index)).await;
        refusals.push(matches!(
            result,
            Err(ProjectCommandChallengeError::MissingProject)
        ));
    }

    assert_eq!(refusals, [true, true]);
    assert_eq!(challenge_rows(&admin).await, rows_before);
}
