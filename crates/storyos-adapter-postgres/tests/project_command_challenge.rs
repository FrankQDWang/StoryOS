use storyos_adapter_postgres::PostgresProjectReader;
use storyos_application::{
    IssueProjectCommandChallenge, ProjectCommandChallengeBinding, ProjectCommandChallengeUse,
    ProjectId, ProjectScope, UserId, consume_project_command_challenge,
    issue_project_command_challenge,
};

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT_A: &str = "018f0000-0000-7001-8000-000000000002";

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
}
