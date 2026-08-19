use storyos_adapter_postgres::PostgresProjectReader;
use storyos_application::{
    Chapter, ChapterId, Project, ProjectId, ProjectReader, ProjectScope, RevisionId, UserId,
    open_current_chapter, open_project,
};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const PROJECT_A: &str = "018f0000-0000-7001-8000-000000000002";
const CHAPTER_A: &str = "018f0000-0000-7001-8000-000000000003";
const REVISION_A: &str = "018f0000-0000-7001-8000-000000000005";
const STALE_CHAPTER_A: &str = "018f0000-0000-7001-8000-000000000006";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const PROJECT_B: &str = "018f0000-0000-7001-8000-000000000102";

#[tokio::test]
async fn database_error_keeps_its_cause_without_exposing_it_in_display() {
    let reader = PostgresProjectReader::new("postgres://[");
    let scope = ProjectScope::new(UserId::new(USER_A), ProjectId::new(PROJECT_A));

    let error = reader.read_project(&scope).await.unwrap_err();

    assert_eq!(error.to_string(), "Project read is unavailable");
    let source = std::error::Error::source(&error).expect("PostgreSQL cause is retained");
    assert!(!source.to_string().is_empty());
    assert_ne!(source.to_string(), error.to_string());
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn two_users_and_projects_are_isolated_at_application_and_rls_boundaries() {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let reader = PostgresProjectReader::new(&runtime_url);
    let scope_a = ProjectScope::new(UserId::new(USER_A), ProjectId::new(PROJECT_A));
    let scope_b = ProjectScope::new(UserId::new(USER_B), ProjectId::new(PROJECT_B));

    assert_eq!(
        open_project(&reader, &scope_a).await.unwrap(),
        Some(Project {
            project_id: ProjectId::new(PROJECT_A),
            title: "Project A".to_owned(),
            current_chapter_id: ChapterId::new(CHAPTER_A),
        })
    );
    assert_eq!(
        open_current_chapter(&reader, &scope_a, &ChapterId::new(CHAPTER_A))
            .await
            .unwrap(),
        Some(Chapter {
            chapter_id: ChapterId::new(CHAPTER_A),
            title: "Chapter A".to_owned(),
            revision_id: RevisionId::new(REVISION_A),
            body: "Authoritative A".to_owned(),
            project_activity_position: 0,
        })
    );
    assert_eq!(
        reader
            .read_project(&ProjectScope::new(
                UserId::new(USER_A),
                ProjectId::new(PROJECT_B),
            ))
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        reader
            .read_project(&ProjectScope::new(
                UserId::new(USER_B),
                ProjectId::new(PROJECT_A),
            ))
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        open_current_chapter(&reader, &scope_a, &ChapterId::new(STALE_CHAPTER_A))
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        open_current_chapter(&reader, &scope_b, &ChapterId::new(CHAPTER_A))
            .await
            .unwrap(),
        None
    );

    let (mut client, connection) = tokio_postgres::connect(&runtime_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    let transaction = client.transaction().await.unwrap();
    transaction
        .execute(
            "SELECT set_config('storyos.user_id', $1, true), \
             set_config('storyos.owner_user_id', $1, true), \
             set_config('storyos.project_id', $2, true)",
            &[&USER_A, &PROJECT_A],
        )
        .await
        .unwrap();
    let visible_titles = transaction
        .query("SELECT title FROM storyos.projects ORDER BY title", &[])
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get::<_, String>(0))
        .collect::<Vec<_>>();
    assert_eq!(visible_titles, vec!["Project A"]);
    let posture = transaction
        .query_one(
            "SELECT rolsuper, rolbypassrls FROM pg_roles WHERE rolname = current_user",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        (posture.get::<_, bool>(0), posture.get::<_, bool>(1)),
        (false, false)
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn composite_identity_rejects_a_foreign_revision_reference() {
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (mut client, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    let transaction = client.transaction().await.unwrap();
    transaction
        .execute(
            "INSERT INTO storyos.manuscript_objects \
             (owner_user_id, project_id, manuscript_object_id, object_kind, title) \
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 'chapter', 'Composite probe')",
            &[&USER_A, &PROJECT_A, &"018f0000-0000-7001-8000-000000000099"],
        )
        .await
        .unwrap();
    let result = transaction
        .execute(
            "INSERT INTO storyos.authoritative_heads \
             (owner_user_id, project_id, manuscript_object_id, current_revision_id) \
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid)",
            &[
                &USER_A,
                &PROJECT_A,
                &"018f0000-0000-7001-8000-000000000099",
                &"018f0000-0000-7001-8000-000000000105",
            ],
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn editor_session_tables_force_scope_and_composite_references() {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (mut client, connection) = tokio_postgres::connect(&runtime_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    let transaction = client.transaction().await.unwrap();
    transaction
        .execute(
            "SELECT set_config('storyos.user_id', $1, true), \
             set_config('storyos.owner_user_id', $1, true), \
             set_config('storyos.project_id', $2, true)",
            &[&USER_A, &PROJECT_A],
        )
        .await
        .unwrap();
    let session_id = "018f0000-0000-7001-8000-000000000081";
    transaction
        .execute(
            "INSERT INTO storyos.editor_sessions
             (owner_user_id, project_id, editor_session_id, client_session_binding_ref,
              client_session_generation, client_contract_revision, security_policy_revision)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 'binding', 1, 'client', 'security')",
            &[&USER_A, &PROJECT_A, &session_id],
        )
        .await
        .unwrap();
    transaction
        .batch_execute("SAVEPOINT wrong_scope")
        .await
        .unwrap();
    assert!(
        transaction
            .execute(
                "INSERT INTO storyos.editor_sessions
                 (owner_user_id, project_id, editor_session_id, client_session_binding_ref,
                  client_session_generation, client_contract_revision, security_policy_revision)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 'binding', 1, 'client', 'security')",
                &[&USER_B, &PROJECT_B, &"018f0000-0000-7001-8000-000000000181"],
            )
            .await
            .is_err()
    );
    transaction
        .batch_execute("ROLLBACK TO SAVEPOINT wrong_scope")
        .await
        .unwrap();
    transaction
        .batch_execute("SAVEPOINT foreign_revision")
        .await
        .unwrap();
    assert!(
        transaction
            .execute(
                "INSERT INTO storyos.editor_session_base_snapshots
                 (owner_user_id, project_id, snapshot_id, editor_session_id,
                  chapter_object_id, authoritative_revision_id)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid, $6::text::uuid)",
                &[
                    &USER_A,
                    &PROJECT_A,
                    &"018f0000-0000-7001-8000-000000000082",
                    &session_id,
                    &CHAPTER_A,
                    &"018f0000-0000-7001-8000-000000000105",
                ],
            )
            .await
            .is_err()
    );
    transaction
        .batch_execute("ROLLBACK TO SAVEPOINT foreign_revision")
        .await
        .unwrap();

    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (client, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    let posture = client
        .query(
            "SELECT relname, relrowsecurity, relforcerowsecurity
             FROM pg_class WHERE relname IN
               ('editor_session_base_snapshots', 'editor_sessions', 'project_snapshots',
                'project_writer_generations', 'replay_floors', 'replay_generations')
             ORDER BY relname",
            &[],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| {
            (
                row.get::<_, String>(0),
                row.get::<_, bool>(1),
                row.get::<_, bool>(2),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        posture,
        [
            "editor_session_base_snapshots",
            "editor_sessions",
            "project_snapshots",
            "project_writer_generations",
            "replay_floors",
            "replay_generations",
        ]
        .map(|name| (name.to_owned(), true, true))
    );
}
