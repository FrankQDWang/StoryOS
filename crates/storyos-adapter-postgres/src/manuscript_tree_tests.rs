use super::*;
use storyos_application::{
    CanonicalManuscriptTree, ProjectId, ProjectScope, UserId, get_manuscript_tree,
};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const PROJECT_B: &str = "018f0000-0000-7001-8000-000000000102";
const EMPTY_TREE: &str = "018f0000-0000-7001-8000-000000000014";

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn empty_canonical_tree_is_scope_safe_and_snapshot_bound() {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let reader = PostgresProjectReader::new(runtime_url);
    let scope = ProjectScope::new(UserId::new(USER_A), ProjectId::new(EMPTY_TREE));
    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    admin
        .execute(
            "INSERT INTO storyos.projects (owner_user_id, project_id, title, current_chapter_id)
             VALUES ($1::text::uuid, $2::text::uuid, 'Empty Tree', NULL)",
            &[&USER_A, &EMPTY_TREE],
        )
        .await
        .unwrap();

    let Some(first) = get_manuscript_tree(&reader, &scope).await.unwrap() else {
        panic!("empty active Project must return a Canonical Query");
    };
    assert_eq!(
        first,
        CanonicalManuscriptTree {
            project_scope: scope.clone(),
            snapshot: first.snapshot.clone(),
            tree_revision: 1,
            volumes: Vec::new(),
        }
    );
    assert_eq!(first.snapshot.redaction_profile, "storyos.author.v1");
    assert_eq!(first.snapshot.schema_profile, "storyos.public.release.1");
    assert_eq!(first.snapshot.replay_generation, 1);
    assert_eq!(first.snapshot.floor_position, 0);

    let second = get_manuscript_tree(&reader, &scope)
        .await
        .unwrap()
        .expect("the issued Snapshot remains bound");
    assert_eq!(second, first);

    assert_eq!(
        get_manuscript_tree(
            &reader,
            &ProjectScope::new(UserId::new(USER_B), ProjectId::new(EMPTY_TREE)),
        )
        .await
        .unwrap(),
        None
    );
    assert_eq!(
        get_manuscript_tree(
            &reader,
            &ProjectScope::new(UserId::new(USER_A), ProjectId::new(PROJECT_B)),
        )
        .await
        .unwrap(),
        None
    );
}
