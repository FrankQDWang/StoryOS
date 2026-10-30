use super::*;
use crate::command_sequence::tests::create_project_issue;
use storyos_application::{
    AuthorCommandAdmissionIds, CreateProjectChallengeBinding, CreateProjectCommand,
    EditorClientBinding, GetManuscriptTree, ProjectId, ProjectScope, UserId, create_project,
    get_manuscript_tree, issue_create_project_challenge,
};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const PROJECT_B: &str = "018f0000-0000-7001-8000-000000000102";

fn command(
    binding: CreateProjectChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
) -> CreateProjectCommand {
    let project_scope = ProjectScope::new(
        binding.owner_user_id.clone(),
        binding.prospective_project_id.clone(),
    );
    CreateProjectCommand {
        project_scope,
        client_binding: EditorClientBinding {
            binding_ref: binding.client_session_binding_digest.clone(),
            session_generation: binding.client_session_generation,
            client_contract_revision: binding.client_contract_revision.clone(),
            security_policy_revision: binding.security_policy_revision.clone(),
        },
        challenge_binding: binding,
        nonce_digest: nonce_digest.to_owned(),
        canonical_command_bytes: br#"{"title":"Empty Novel"}"#.to_vec(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{ids_suffix}"),
        title: "Empty Novel".to_owned(),
        ids: AuthorCommandAdmissionIds {
            command_id: format!("018f0000-0000-7001-8000-00000001{ids_suffix}"),
            author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{ids_suffix}"),
            receipt_id: format!("018f0000-0000-7001-8000-00000003{ids_suffix}"),
        },
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn empty_canonical_tree_is_scope_safe_and_snapshot_bound() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let issue = create_project_issue(USER_A, "018f0000-0000-7001-8000-000000000610", "0610");
    let issued = issue_create_project_challenge(&store, &issue)
        .await
        .unwrap();
    let mut binding = issue.binding.clone();
    binding.prospective_project_id = issued.prospective_project_id.clone();
    binding.canonical_command_digest = issued.canonical_command_digest.clone();
    create_project(
        &store,
        &command(binding.clone(), &issue.nonce_digest, "0611"),
    )
    .await
    .unwrap();
    let scope = ProjectScope::new(
        binding.owner_user_id.clone(),
        binding.prospective_project_id.clone(),
    );

    let GetManuscriptTree::Found(first) = get_manuscript_tree(&store, &scope).await.unwrap() else {
        panic!("empty active Project must return a Canonical Query");
    };
    assert_eq!(first.project_scope, scope);
    assert_eq!(first.volumes, Vec::new());

    let GetManuscriptTree::Found(second) = get_manuscript_tree(&store, &scope).await.unwrap()
    else {
        panic!("the Create Project Snapshot remains bound");
    };
    assert_eq!(second, first);

    assert_eq!(
        get_manuscript_tree(
            &store,
            &ProjectScope::new(UserId::new(USER_B), scope.project_id.clone()),
        )
        .await
        .unwrap(),
        GetManuscriptTree::Missing
    );
    assert_eq!(
        get_manuscript_tree(
            &store,
            &ProjectScope::new(UserId::new(USER_A), ProjectId::new(PROJECT_B)),
        )
        .await
        .unwrap(),
        GetManuscriptTree::Missing
    );

    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    admin
        .execute(
            "UPDATE storyos.projects SET lifecycle_state = 'archived'
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&USER_A, &scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    assert_eq!(
        get_manuscript_tree(&store, &scope).await.unwrap(),
        GetManuscriptTree::Missing
    );
}
