use super::*;
use storyos_application::{
    AuthorCommandAdmissionIds, CreateProjectChallengeBinding, CreateProjectCommand,
    CreateVolumeCommand, CreateVolumeSettlementEffect, EditorClientBinding,
    IssueCreateProjectChallenge, IssueProjectCommandChallenge, ProjectCommandChallengeBinding,
    ProjectId, ProjectScope, UserId, create_project, create_volume, issue_create_project_challenge,
    issue_project_command_challenge,
};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const CLIENT: &str = "storyos.web-client.release-1.v3";
const SECURITY: &str = "storyos.web-security-policy.release-1.v1";
const TITLE: &str = "Volume A";
const COMMAND_BYTES: &[u8] = br#"{"expected_tree_revision":"1","title":"Volume A"}"#;
const COMMAND_DIGEST: &str = "sha256:storyos.command.createVolume.jcs.v1:2b02ae40bec5ed5ccf7ec412519d2178186dc80e3829100851514a6f3422cedf";

fn create_project_issue(
    idempotency_key: &str,
    project_suffix: &str,
) -> IssueCreateProjectChallenge {
    let digest = format!("sha256:storyos.command.createProject.jcs.v1:{project_suffix:0<64}");
    IssueCreateProjectChallenge {
        binding: CreateProjectChallengeBinding {
            owner_user_id: UserId::new(USER_A),
            prospective_project_id: ProjectId::new(format!(
                "018f0000-0000-7001-8000-00000000{project_suffix}"
            )),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: 1,
            client_contract_revision: CLIENT.to_owned(),
            security_policy_revision: SECURITY.to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            method: "POST".to_owned(),
            route_template: "/api/v1/projects".to_owned(),
            command_schema: "storyos.command.create-project.request.v1".to_owned(),
            command_kind: "createProject".to_owned(),
            create_input_digest: format!("{project_suffix:0<64}"),
            canonical_command_digest: digest,
            idempotency_key: idempotency_key.to_owned(),
        },
        nonce_digest: format!("sha256:nonce-{project_suffix}"),
    }
}

fn create_project_command(
    binding: CreateProjectChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
) -> CreateProjectCommand {
    CreateProjectCommand {
        project_scope: ProjectScope::new(
            binding.owner_user_id.clone(),
            binding.prospective_project_id.clone(),
        ),
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

fn volume_issue(scope: &ProjectScope, idempotency_suffix: &str) -> IssueProjectCommandChallenge {
    IssueProjectCommandChallenge {
        binding: ProjectCommandChallengeBinding {
            project_scope: scope.clone(),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: 1,
            client_contract_revision: CLIENT.to_owned(),
            security_policy_revision: SECURITY.to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            challenge_rate_policy_revision:
                "storyos.project-command-challenge-rate.fixed-window.v1".to_owned(),
            method: "POST".to_owned(),
            route_template: "/api/v1/projects/{project_id}/volumes".to_owned(),
            command_schema: "storyos.command.create-volume.request.v1".to_owned(),
            command_kind: "createVolume".to_owned(),
            canonical_command_digest: COMMAND_DIGEST.to_owned(),
            idempotency_key: format!("018f0000-0000-7001-8000-00000000{idempotency_suffix}"),
        },
        nonce: format!("opaque-nonce-{idempotency_suffix}"),
        nonce_digest: format!("sha256:nonce-{idempotency_suffix}"),
    }
}

fn volume_command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
    expected_tree_revision: u64,
) -> CreateVolumeCommand {
    CreateVolumeCommand {
        project_scope: binding.project_scope.clone(),
        client_binding: EditorClientBinding {
            binding_ref: binding.client_session_binding_digest.clone(),
            session_generation: binding.client_session_generation,
            client_contract_revision: binding.client_contract_revision.clone(),
            security_policy_revision: binding.security_policy_revision.clone(),
        },
        challenge_binding: binding,
        nonce_digest: nonce_digest.to_owned(),
        canonical_command_bytes: COMMAND_BYTES.to_vec(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{ids_suffix}"),
        title: TITLE.to_owned(),
        expected_tree_revision,
        ids: AuthorCommandAdmissionIds {
            command_id: format!("018f0000-0000-7001-8000-00000001{ids_suffix}"),
            author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{ids_suffix}"),
            receipt_id: format!("018f0000-0000-7001-8000-00000003{ids_suffix}"),
        },
    }
}

async fn open_admin() -> tokio_postgres::Client {
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    admin
}

async fn seed_project_with_volume(
    store: &PostgresProjectReader,
    project_suffix: &str,
    volume_suffix: &str,
) -> (ProjectScope, String) {
    let issue = create_project_issue(
        &format!("018f0000-0000-7001-8000-00000000{project_suffix}"),
        project_suffix,
    );
    let issued = issue_create_project_challenge(store, &issue).await.unwrap();
    let mut project_binding = issue.binding.clone();
    project_binding.prospective_project_id = issued.prospective_project_id.clone();
    project_binding.canonical_command_digest = issued.canonical_command_digest.clone();
    create_project(
        store,
        &create_project_command(project_binding.clone(), &issue.nonce_digest, project_suffix),
    )
    .await
    .unwrap();
    let scope = ProjectScope::new(
        project_binding.owner_user_id.clone(),
        project_binding.prospective_project_id.clone(),
    );
    let first_issue = volume_issue(&scope, volume_suffix);
    issue_project_command_challenge(store, &first_issue)
        .await
        .unwrap();
    let first = create_volume(
        store,
        &volume_command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            volume_suffix,
            1,
        ),
    )
    .await
    .unwrap();
    let CreateVolumeSettlementEffect::Applied { volume_id, .. } = first.effect else {
        panic!("Create Volume on an empty active Project must apply");
    };
    (scope, volume_id)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn authority_history_floor_exists_without_rewriting_structure_activity() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let (scope, _) = seed_project_with_volume(&store, "0810", "0812").await;
    let admin = open_admin().await;
    let row = admin
        .query_one(
            "SELECT floor.floor_activity_position::text,
                    floor.snapshot_id::text,
                    snapshot.replay_generation::text,
                    snapshot.project_activity_position::text,
                    (SELECT count(*) FROM storyos.replay_generations
                      WHERE project_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.authoritative_commits
                      WHERE project_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.author_action_entries
                      WHERE project_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.project_activity_event_payloads
                      WHERE project_id = $1::text::uuid AND event_kind = 'volume_created'),
                    (SELECT payload::text FROM storyos.project_activity_event_payloads
                      WHERE project_id = $1::text::uuid AND event_kind = 'volume_created')
               FROM storyos.authority_history_floors AS floor
               JOIN storyos.project_snapshots AS snapshot
                 ON (snapshot.owner_user_id, snapshot.project_id, snapshot.snapshot_id) =
                    (floor.owner_user_id, floor.project_id, floor.snapshot_id)
              WHERE floor.project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let payload: serde_json::Value = serde_json::from_str(&row.get::<_, String>(8)).unwrap();
    assert_eq!(row.get::<_, String>(2), "1");
    assert_eq!(row.get::<_, i64>(4), 1);
    assert_eq!(row.get::<_, i64>(5), 0);
    assert_eq!(row.get::<_, i64>(6), 0);
    assert_eq!(row.get::<_, i64>(7), 1);
    assert_eq!(row.get::<_, String>(0), row.get::<_, String>(3));
    assert_eq!(
        payload.get("kind").and_then(serde_json::Value::as_str),
        Some("volume_created")
    );
    assert!(payload.get("authoritative_commit_id").is_none());
    assert!(payload.get("author_action_sequence").is_none());
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn applied_structure_receipt_may_bind_empty_pair_commit_and_author_action() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let (scope, volume_id) = seed_project_with_volume(&store, "0820", "0822").await;
    let admin = open_admin().await;
    let receipt = admin
        .query_one(
            "SELECT receipt_id::text, author_command_admission_id::text
               FROM storyos.domain_receipts
              WHERE project_id = $1::text::uuid
                AND command_kind = 'createVolume'
                AND result_kind = 'authoritative_applied'",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let receipt_id: String = receipt.get(0);
    let admission_id: String = receipt.get(1);
    let commit_id = "018f0000-0000-7001-8000-000000000823";
    admin.batch_execute("BEGIN").await.unwrap();
    admin
        .execute(
            "INSERT INTO storyos.authoritative_commits
               (owner_user_id, project_id, authoritative_commit_id, authoritative_commit_sequence,
                author_command_admission_id, receipt_id, receipt_result_kind,
                prior_manuscript_tree_revision, resulting_manuscript_tree_revision,
                affected_volume_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 1,
                     $4::text::uuid, $5::text::uuid, 'authoritative_applied',
                     1, 2, $6::text::uuid)",
            &[
                &USER_A,
                &scope.project_id.as_ref(),
                &commit_id,
                &admission_id,
                &receipt_id,
                &volume_id,
            ],
        )
        .await
        .unwrap();
    admin
        .execute(
            "INSERT INTO storyos.author_action_entries
               (owner_user_id, project_id, author_action_sequence, disposition,
                authoritative_commit_id, receipt_id, receipt_result_kind)
             VALUES ($1::text::uuid, $2::text::uuid, 1, 'forward',
                     $3::text::uuid, $4::text::uuid, 'authoritative_applied')",
            &[&USER_A, &scope.project_id.as_ref(), &commit_id, &receipt_id],
        )
        .await
        .unwrap();
    admin
        .execute(
            "UPDATE storyos.domain_receipts
                SET authoritative_commit_ids = ARRAY[$3::text::uuid]
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND receipt_id = $4::text::uuid",
            &[&USER_A, &scope.project_id.as_ref(), &commit_id, &receipt_id],
        )
        .await
        .unwrap();
    admin.batch_execute("COMMIT").await.unwrap();
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn unsuccessful_structure_receipt_still_rejects_commit_ids() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let (scope, _) = seed_project_with_volume(&store, "0830", "0832").await;
    let stale_issue = volume_issue(&scope, "0834");
    issue_project_command_challenge(&store, &stale_issue)
        .await
        .unwrap();
    let stale = create_volume(
        &store,
        &volume_command(
            stale_issue.binding.clone(),
            &stale_issue.nonce_digest,
            "0835",
            1,
        ),
    )
    .await
    .unwrap();
    assert!(matches!(
        stale.effect,
        CreateVolumeSettlementEffect::Conflicted { .. }
    ));
    let admin = open_admin().await;
    let error = admin
        .execute(
            "UPDATE storyos.domain_receipts
                SET authoritative_commit_ids = ARRAY[$3::text::uuid]
              WHERE project_id = $1::text::uuid
                AND receipt_id = $2::text::uuid",
            &[
                &scope.project_id.as_ref(),
                &stale.ids.receipt_id,
                &"018f0000-0000-7001-8000-000000000836",
            ],
        )
        .await
        .expect_err("a conflicted structure Receipt must keep zero Commit identities");
    assert!(error.code().is_some());
}
