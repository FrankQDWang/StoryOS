use super::*;
use crate::command_sequence::tests::create_project_issue;
use storyos_application::{
    AuthorCommandAdmissionIds, CreateProjectChallengeBinding, CreateProjectCommand,
    CreateProjectError, EditorClientBinding, ProjectScope, create_project,
    issue_create_project_challenge,
};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const TITLE: &str = "Empty Novel";

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
        title: TITLE.to_owned(),
        ids: AuthorCommandAdmissionIds {
            command_id: format!("018f0000-0000-7001-8000-00000001{ids_suffix}"),
            author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{ids_suffix}"),
            receipt_id: format!("018f0000-0000-7001-8000-00000003{ids_suffix}"),
        },
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn create_project_is_atomic_empty_replayable_and_fail_closed() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url.clone());
    let first_issue = create_project_issue(USER_A, "018f0000-0000-7001-8000-000000000401", "0401");
    let issued = issue_create_project_challenge(&store, &first_issue)
        .await
        .unwrap();
    let mut first_binding = first_issue.binding.clone();
    first_binding.prospective_project_id = issued.prospective_project_id.clone();
    first_binding.canonical_command_digest = issued.canonical_command_digest.clone();
    let first = create_project(
        &store,
        &command(first_binding.clone(), &first_issue.nonce_digest, "0402"),
    )
    .await
    .unwrap();
    assert_eq!(first.title, TITLE);
    assert_eq!(first.project_activity_position, 1);
    let replay = create_project(
        &store,
        &command(first_binding.clone(), &first_issue.nonce_digest, "0499"),
    )
    .await
    .unwrap();
    assert_eq!(replay, first);

    let mut changed_digest = first_binding.clone();
    changed_digest.canonical_command_digest =
        "sha256:storyos.command.createProject.jcs.v1:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
            .to_owned();
    assert!(matches!(
        create_project(
            &store,
            &command(changed_digest, &first_issue.nonce_digest, "0403")
        )
        .await,
        Err(CreateProjectError::InvalidChallenge)
    ));
    assert!(matches!(
        create_project(
            &store,
            &command(first_binding.clone(), "sha256:nonce-other", "0404")
        )
        .await,
        Err(CreateProjectError::InvalidChallenge)
    ));

    let existing_issue =
        create_project_issue(USER_A, "018f0000-0000-7001-8000-000000000405", "0405");
    let existing_issued = issue_create_project_challenge(&store, &existing_issue)
        .await
        .unwrap();
    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    admin
        .execute(
            "INSERT INTO storyos.projects (owner_user_id, project_id, title, current_chapter_id)
             VALUES ($1::text::uuid, $2::text::uuid, 'Existing', NULL)",
            &[&USER_A, &existing_issued.prospective_project_id.as_ref()],
        )
        .await
        .unwrap();
    let mut existing_binding = existing_issue.binding.clone();
    existing_binding.prospective_project_id = existing_issued.prospective_project_id.clone();
    existing_binding.canonical_command_digest = existing_issued.canonical_command_digest.clone();
    assert!(matches!(
        create_project(
            &store,
            &command(existing_binding, &existing_issue.nonce_digest, "0406")
        )
        .await,
        Err(CreateProjectError::ExistingProject)
    ));

    let project_id = first_binding.prospective_project_id.as_ref();
    let counts = admin
        .query_one(
            "SELECT
               (SELECT count(*) FROM storyos.projects WHERE project_id = $1::text::uuid),
               (SELECT current_chapter_id::text FROM storyos.projects WHERE project_id = $1::text::uuid),
               (SELECT count(*) FROM storyos.manuscript_objects WHERE project_id = $1::text::uuid),
               (SELECT count(*) FROM storyos.author_command_admissions WHERE project_id = $1::text::uuid),
               (SELECT count(*) FROM storyos.domain_receipts WHERE project_id = $1::text::uuid),
               (SELECT count(*) FROM storyos.project_activity_event_payloads WHERE project_id = $1::text::uuid),
               (SELECT count(*) FROM storyos.project_activity_events WHERE project_id = $1::text::uuid),
               (SELECT count(*) FROM storyos.authoritative_commits WHERE project_id = $1::text::uuid)",
            &[&project_id],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            counts.get::<_, i64>(0),
            counts.get::<_, Option<String>>(1),
            counts.get::<_, i64>(2),
            counts.get::<_, i64>(3),
            counts.get::<_, i64>(4),
            counts.get::<_, i64>(5),
            counts.get::<_, i64>(6),
            counts.get::<_, i64>(7)
        ),
        (1, None, 0, 1, 1, 1, 0, 0)
    );

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
               (SELECT count(*) FROM storyos.projects WHERE project_id = $1::text::uuid),
               (SELECT count(*) FROM storyos.create_project_challenges
                 WHERE idempotency_key = '018f0000-0000-7001-8000-000000000401'::uuid)",
            &[&project_id],
        )
        .await
        .unwrap();
    assert_eq!((hidden.get::<_, i64>(0), hidden.get::<_, i64>(1)), (0, 0));
    foreign.rollback().await.unwrap();

    let payloads = "SELECT count(*) FROM storyos.project_activity_event_payloads
                     WHERE project_id = $1::text::uuid AND event_kind = 'project_created'";
    admin
        .batch_execute(&format!(
            "BEGIN; SET LOCAL session_replication_role = replica;
             UPDATE storyos.project_activity_event_payloads
                SET payload = jsonb_set(payload, '{{title}}', '\"Damaged original title\"'::jsonb)
              WHERE project_id = '{project_id}'::uuid AND event_kind = 'project_created';
             COMMIT"
        ))
        .await
        .unwrap();
    assert!(matches!(
        create_project(
            &store,
            &command(first_binding.clone(), &first_issue.nonce_digest, "0498")
        )
        .await,
        Err(CreateProjectError::Unavailable(_))
    ));
    let damaged = admin.query_one(payloads, &[&project_id]).await.unwrap();
    assert_eq!(damaged.get::<_, i64>(0), 1);
    admin
        .batch_execute(&format!(
            "BEGIN; SET LOCAL session_replication_role = replica;
             DELETE FROM storyos.project_activity_event_payloads
              WHERE project_id = '{project_id}'::uuid AND event_kind = 'project_created';
             COMMIT"
        ))
        .await
        .unwrap();
    assert!(matches!(
        create_project(
            &store,
            &command(first_binding.clone(), &first_issue.nonce_digest, "0497")
        )
        .await,
        Err(CreateProjectError::BindingConflict)
    ));
    let removed = admin.query_one(payloads, &[&project_id]).await.unwrap();
    assert_eq!(removed.get::<_, i64>(0), 0);
}
