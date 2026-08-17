use storyos_application::{
    ApplyAuthorEditOutcome, ApplyAuthorEditRejectionReason, ApplyAuthorEditUnknownObservation,
    EditorClientBinding, IssueProjectCommandChallenge, ProjectCommandChallengeBinding,
    ProjectCommandChallengeUse, ProjectId, ProjectScope, ReadApplyAuthorEditOutcome, UserId,
    consume_project_command_challenge, get_apply_author_edit_outcome,
    issue_project_command_challenge,
};

use super::*;

const USER_ID: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT_ID: &str = "018f0000-0000-7001-8000-000000000002";

fn scope() -> ProjectScope {
    ProjectScope::new(UserId::new(USER_ID), ProjectId::new(PROJECT_ID))
}

fn challenge_binding(idempotency_key: &str) -> ProjectCommandChallengeBinding {
    ProjectCommandChallengeBinding {
        project_scope: scope(),
        client_session_binding_digest: "binding:outcome-reader".to_owned(),
        client_session_generation: 109,
        client_contract_revision: "storyos.client.release-1.v1".to_owned(),
        security_policy_revision: "storyos.security.release-1.v1".to_owned(),
        limit_profile_revision: "storyos.limit.release-1.v1".to_owned(),
        challenge_rate_policy_revision:
            storyos_application::PROJECT_COMMAND_CHALLENGE_RATE_POLICY_REVISION.to_owned(),
        method: "POST".to_owned(),
        route_template: "/api/v1/projects/{project_id}/manuscript/author-edits".to_owned(),
        command_schema: "storyos.command.apply-author-edit.request.v1".to_owned(),
        command_kind: "applyAuthorEdit".to_owned(),
        canonical_command_digest:
            "sha256:storyos.command.applyAuthorEdit.jcs.v1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
        idempotency_key: idempotency_key.to_owned(),
    }
}

fn outcome_query(
    binding: &ProjectCommandChallengeBinding,
    nonce_digest: &str,
) -> ReadApplyAuthorEditOutcome {
    ReadApplyAuthorEditOutcome {
        project_scope: binding.project_scope.clone(),
        client_binding: EditorClientBinding {
            binding_ref: binding.client_session_binding_digest.clone(),
            session_generation: binding.client_session_generation,
            client_contract_revision: binding.client_contract_revision.clone(),
            security_policy_revision: binding.security_policy_revision.clone(),
        },
        limit_profile_revision: binding.limit_profile_revision.clone(),
        idempotency_key: binding.idempotency_key.clone(),
        nonce_digest: nonce_digest.to_owned(),
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn unexpired_challenge_is_unknown_and_the_query_does_not_consume_it() {
    let database_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(database_url);
    let binding = challenge_binding("018f0000-0000-7001-8000-000000000109");
    let nonce_digest = "sha256:outcome-reader-nonce-109";
    let challenge = issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: binding.clone(),
            nonce: "nonce-109".to_owned(),
            nonce_digest: nonce_digest.to_owned(),
        },
    )
    .await
    .unwrap();

    let outcome = get_apply_author_edit_outcome(&store, &outcome_query(&binding, nonce_digest))
        .await
        .unwrap();

    assert_eq!(
        outcome,
        ApplyAuthorEditOutcome::StillUnknown {
            observation: ApplyAuthorEditUnknownObservation::ChallengeIssued {
                expires_at: challenge.expires_at,
            },
        }
    );
    let mut command_transaction = store
        .begin_project_command_transaction(&scope())
        .await
        .unwrap();
    assert_eq!(
        consume_project_command_challenge(&mut command_transaction, &binding, nonce_digest)
            .await
            .unwrap(),
        ProjectCommandChallengeUse::FirstUse
    );
    command_transaction.rollback().await.unwrap();
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn expired_unconsumed_challenge_without_admission_is_rejected() {
    let database_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(database_url);
    let binding = challenge_binding("018f0000-0000-7001-8000-000000000110");
    let nonce_digest = "sha256:outcome-reader-nonce-110";
    issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: binding.clone(),
            nonce: "nonce-110".to_owned(),
            nonce_digest: nonce_digest.to_owned(),
        },
    )
    .await
    .unwrap();
    let client = store.connect_challenge().await.unwrap();
    client.batch_execute("BEGIN").await.unwrap();
    set_challenge_scope_on_client(&client, &binding.project_scope)
        .await
        .unwrap();
    client
        .execute(
            "UPDATE storyos.project_command_challenges
                SET expires_at = clock_timestamp() - interval '1 second'
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND command_kind = 'applyAuthorEdit'
                AND idempotency_key = $3::text::uuid",
            &[
                &binding.project_scope.owner_user_id.as_ref(),
                &binding.project_scope.project_id.as_ref(),
                &binding.idempotency_key,
            ],
        )
        .await
        .unwrap();
    client.batch_execute("COMMIT").await.unwrap();

    let outcome = get_apply_author_edit_outcome(&store, &outcome_query(&binding, nonce_digest))
        .await
        .unwrap();

    assert_eq!(
        outcome,
        ApplyAuthorEditOutcome::Rejected {
            reason: ApplyAuthorEditRejectionReason::ChallengeExpiredUnconsumed,
        }
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn missing_foreign_and_changed_proof_bindings_share_one_unavailable_oracle() {
    let database_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(database_url);
    let binding = challenge_binding("018f0000-0000-7001-8000-000000000111");
    let nonce_digest = "sha256:outcome-reader-nonce-111";
    issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: binding.clone(),
            nonce: "nonce-111".to_owned(),
            nonce_digest: nonce_digest.to_owned(),
        },
    )
    .await
    .unwrap();

    let base = outcome_query(&binding, nonce_digest);
    let mut cases = Vec::new();
    let mut missing = base.clone();
    missing.idempotency_key = "018f0000-0000-7001-8000-000000000112".to_owned();
    cases.push(missing);
    let mut foreign = base.clone();
    foreign.project_scope = ProjectScope::new(
        UserId::new("018f0000-0000-7001-8000-000000000101"),
        ProjectId::new("018f0000-0000-7001-8000-000000000102"),
    );
    cases.push(foreign);
    let mut wrong_nonce = base.clone();
    wrong_nonce.nonce_digest = "sha256:wrong".to_owned();
    cases.push(wrong_nonce);
    let mut wrong_binding = base.clone();
    wrong_binding.client_binding.binding_ref = "binding:wrong".to_owned();
    cases.push(wrong_binding);
    let mut wrong_generation = base.clone();
    wrong_generation.client_binding.session_generation += 1;
    cases.push(wrong_generation);
    let mut wrong_client_contract = base.clone();
    wrong_client_contract
        .client_binding
        .client_contract_revision = "client:wrong".to_owned();
    cases.push(wrong_client_contract);
    let mut wrong_security_policy = base.clone();
    wrong_security_policy
        .client_binding
        .security_policy_revision = "security:wrong".to_owned();
    cases.push(wrong_security_policy);
    let mut wrong_limit_profile = base;
    wrong_limit_profile.limit_profile_revision = "limit:wrong".to_owned();
    cases.push(wrong_limit_profile);

    for case in cases {
        let error = get_apply_author_edit_outcome(&store, &case)
            .await
            .expect_err("changed or invisible proof must fail closed");
        assert_eq!(
            error.to_string(),
            "Apply Author Edit outcome read is unavailable"
        );
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn outcome_read_waits_for_consume_and_cannot_report_a_false_rejection() {
    let database_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(database_url);
    let binding = challenge_binding("018f0000-0000-7001-8000-000000000113");
    let nonce_digest = "sha256:outcome-reader-nonce-113";
    issue_project_command_challenge(
        &store,
        &IssueProjectCommandChallenge {
            binding: binding.clone(),
            nonce: "nonce-113".to_owned(),
            nonce_digest: nonce_digest.to_owned(),
        },
    )
    .await
    .unwrap();
    let mut command_transaction = store
        .begin_project_command_transaction(&scope())
        .await
        .unwrap();
    assert_eq!(
        consume_project_command_challenge(&mut command_transaction, &binding, nonce_digest)
            .await
            .unwrap(),
        ProjectCommandChallengeUse::FirstUse
    );
    command_transaction
        .client
        .execute(
            "UPDATE storyos.project_command_challenges
                SET expires_at = clock_timestamp() - interval '1 second'
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND command_kind = 'applyAuthorEdit'
                AND idempotency_key = $3::text::uuid",
            &[
                &binding.project_scope.owner_user_id.as_ref(),
                &binding.project_scope.project_id.as_ref(),
                &binding.idempotency_key,
            ],
        )
        .await
        .unwrap();

    let query_store = store.clone();
    let query = outcome_query(&binding, nonce_digest);
    let read =
        tokio::spawn(async move { get_apply_author_edit_outcome(&query_store, &query).await });
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    assert!(
        !read.is_finished(),
        "the outcome read must wait on the command arbiter rows"
    );

    command_transaction.commit().await.unwrap();
    let error = read
        .await
        .unwrap()
        .expect_err("consumed in-progress state without Admission is not Rejected");
    assert_eq!(
        error.to_string(),
        "Apply Author Edit outcome read is unavailable"
    );
}
