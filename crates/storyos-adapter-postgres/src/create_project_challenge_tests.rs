use super::*;
use storyos_application::{
    CreateProjectChallengeBinding, IssueCreateProjectChallenge, ProjectCommandChallengeError,
    ProjectId, UserId, issue_create_project_challenge,
};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const INPUT_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const INPUT_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn issue_request(
    idempotency_key: &str,
    input_digest: &str,
    project_suffix: &str,
) -> IssueCreateProjectChallenge {
    IssueCreateProjectChallenge {
        binding: CreateProjectChallengeBinding {
            owner_user_id: UserId::new(USER_A),
            prospective_project_id: ProjectId::new(format!(
                "018f0000-0000-7001-8000-00000000{project_suffix}"
            )),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: 1,
            client_contract_revision: "storyos.web-client.release-1.v3".to_owned(),
            security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            method: "POST".to_owned(),
            route_template: "/api/v1/projects".to_owned(),
            command_schema: "storyos.command.create-project.request.v1".to_owned(),
            command_kind: "createProject".to_owned(),
            create_input_digest: input_digest.to_owned(),
            canonical_command_digest: format!(
                "sha256:storyos.command.createProject.jcs.v1:{input_digest}"
            ),
            idempotency_key: idempotency_key.to_owned(),
        },
        nonce_digest: format!("sha256:nonce-{project_suffix}"),
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn create_project_challenge_replays_conflicts_and_stays_user_isolated() {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url.clone());
    let first_request = issue_request("018f0000-0000-7001-8000-000000000321", INPUT_A, "0321");
    let first = issue_create_project_challenge(&store, &first_request)
        .await
        .unwrap();
    let retry = issue_create_project_challenge(&store, &first_request)
        .await
        .unwrap();
    assert_eq!(retry, first);
    assert_eq!(
        first.prospective_project_id,
        first_request.binding.prospective_project_id
    );

    let changed = issue_request("018f0000-0000-7001-8000-000000000321", INPUT_B, "0322");
    assert!(matches!(
        issue_create_project_challenge(&store, &changed).await,
        Err(ProjectCommandChallengeError::BindingConflict)
    ));

    let (mut runtime, connection) = tokio_postgres::connect(&runtime_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        connection.await.unwrap();
    });
    let foreign = runtime.transaction().await.unwrap();
    foreign
        .execute("SELECT set_config('storyos.user_id', $1, true)", &[&USER_B])
        .await
        .unwrap();
    let hidden = foreign
        .query_one(
            "SELECT
               (SELECT count(*) FROM storyos.create_project_challenges),
               (SELECT count(*) FROM storyos.create_project_idempotency),
               (SELECT count(*) FROM storyos.projects),
               (SELECT count(*) FROM storyos.users)",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            hidden.get::<_, i64>(0),
            hidden.get::<_, i64>(1),
            hidden.get::<_, i64>(2),
            hidden.get::<_, i64>(3)
        ),
        (0, 0, 0, 1)
    );
    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    let project_id = first.prospective_project_id.as_ref();
    let effect = admin
        .query_one(
            "SELECT
               (SELECT count(*) FROM storyos.projects WHERE project_id = $1::text::uuid),
               (SELECT count(*) FROM storyos.author_command_admissions WHERE project_id = $1::text::uuid),
               (SELECT count(*) FROM storyos.domain_receipts WHERE project_id = $1::text::uuid),
               (SELECT count(*) FROM storyos.project_activity_events WHERE project_id = $1::text::uuid),
               (SELECT count(*) FROM storyos.manuscript_objects WHERE project_id = $1::text::uuid)",
            &[&project_id],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            effect.get::<_, i64>(0),
            effect.get::<_, i64>(1),
            effect.get::<_, i64>(2),
            effect.get::<_, i64>(3),
            effect.get::<_, i64>(4)
        ),
        (0, 0, 0, 0, 0)
    );
}
