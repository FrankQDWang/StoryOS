use super::*;
use storyos_application::{
    AuthorCommandAdmissionIds, ChapterId, CreateChapterCommand, CreateChapterSettlementEffect,
    CreateProjectChallengeBinding, CreateProjectCommand, CreateVolumeCommand,
    CreateVolumeSettlementEffect, EditorClientBinding, EditorSessionId,
    IssueCreateProjectChallenge, IssueProjectCommandChallenge, OpenChapter, OpenEditorSession,
    ProjectCommandChallengeBinding, ProjectId, ProjectScope, SetCurrentChapterCommand,
    SetCurrentChapterSettlementEffect, UserId, create_chapter, create_editor_session,
    create_project, create_volume, issue_create_project_challenge, issue_project_command_challenge,
    open_chapter, set_current_chapter,
};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const CLIENT: &str = "storyos.web-client.release-1.v3";
const SECURITY: &str = "storyos.web-security-policy.release-1.v1";
const TITLE: &str = "Volume A";
const COMMAND_BYTES: &[u8] = br#"{"expected_tree_revision":"1","title":"Volume A"}"#;
const COMMAND_DIGEST: &str = "sha256:storyos.command.createVolume.jcs.v1:2b02ae40bec5ed5ccf7ec412519d2178186dc80e3829100851514a6f3422cedf";
const CHAPTER_A_BYTES: &[u8] = br#"{"expected_tree_revision":"2","title":"Chapter A"}"#;
const CHAPTER_A_DIGEST: &str = "sha256:storyos.command.createChapter.jcs.v1:fdee6f3020b94d08f6e75dfab8be7caf86d1a8c84c521254b03e6a45153a1de2";
const CHAPTER_B_BYTES: &[u8] = br#"{"expected_tree_revision":"3","title":"Chapter B"}"#;
const CHAPTER_B_DIGEST: &str = "sha256:storyos.command.createChapter.jcs.v1:23a99998cce1f35b113794d41235dae8fae7d8ae9f234ba67161a2f9e8e86e65";
const CURRENT_BYTES: &[u8] = b"{}";
const CURRENT_DIGEST: &str = "sha256:storyos.command.setCurrentChapter.jcs.v1:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";

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

fn command_issue(
    scope: &ProjectScope,
    idempotency_suffix: &str,
    method: &str,
    route: &str,
    schema: &str,
    kind: &str,
    digest: &str,
) -> IssueProjectCommandChallenge {
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
            method: method.to_owned(),
            route_template: route.to_owned(),
            command_schema: schema.to_owned(),
            command_kind: kind.to_owned(),
            canonical_command_digest: digest.to_owned(),
            idempotency_key: format!("018f0000-0000-7001-8000-00000000{idempotency_suffix}"),
        },
        nonce: format!("opaque-nonce-{idempotency_suffix}"),
        nonce_digest: format!("sha256:nonce-{idempotency_suffix}"),
    }
}

fn chapter_command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
    volume_id: &str,
    title: &str,
    expected_tree_revision: u64,
    bytes: &[u8],
) -> CreateChapterCommand {
    CreateChapterCommand {
        project_scope: binding.project_scope.clone(),
        client_binding: EditorClientBinding {
            binding_ref: binding.client_session_binding_digest.clone(),
            session_generation: binding.client_session_generation,
            client_contract_revision: binding.client_contract_revision.clone(),
            security_policy_revision: binding.security_policy_revision.clone(),
        },
        challenge_binding: binding,
        nonce_digest: nonce_digest.to_owned(),
        canonical_command_bytes: bytes.to_vec(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{ids_suffix}"),
        volume_id: volume_id.to_owned(),
        title: title.to_owned(),
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
            /*expected_tree_revision*/ 1,
        ),
    )
    .await
    .unwrap();
    let CreateVolumeSettlementEffect::Applied { volume_id, .. } = first.effect else {
        panic!("Create Volume on an empty active Project must apply");
    };
    (scope, volume_id)
}

struct SeedChapter<'a> {
    suffix: &'a str,
    title: &'a str,
    expected_tree_revision: u64,
    bytes: &'a [u8],
    digest: &'a str,
}

async fn seed_chapter(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    volume_id: &str,
    chapter: SeedChapter<'_>,
) -> String {
    let issue = command_issue(
        scope,
        chapter.suffix,
        "POST",
        "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
        "storyos.command.create-chapter.request.v1",
        "createChapter",
        chapter.digest,
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    let created = create_chapter(
        store,
        &chapter_command(
            issue.binding,
            &issue.nonce_digest,
            chapter.suffix,
            volume_id,
            chapter.title,
            chapter.expected_tree_revision,
            chapter.bytes,
        ),
    )
    .await
    .unwrap();
    let CreateChapterSettlementEffect::Applied { chapter_id, .. } = created.effect else {
        panic!("Create Chapter on an active Volume must apply");
    };
    chapter_id
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
    let (scope, volume_id) = seed_project_with_volume(&store, "0810", "0812").await;
    let admin = open_admin().await;
    let observed: serde_json::Value = serde_json::from_str(
        &admin
            .query_one(
                "SELECT jsonb_build_object(
                          'replay_generation', snapshot.replay_generation,
                          'floor_matches_snapshot',
                            floor.floor_activity_position
                              = snapshot.project_activity_position,
                          'replay_generation_count', (
                            SELECT count(*) FROM storyos.replay_generations
                             WHERE project_id = $1::text::uuid
                          ),
                          'commit_count', (
                            SELECT count(*) FROM storyos.authoritative_commits
                             WHERE project_id = $1::text::uuid
                          ),
                          'author_action_count', (
                            SELECT count(*) FROM storyos.author_action_entries
                             WHERE project_id = $1::text::uuid
                          ),
                          'volume_created_payload', (
                            SELECT payload
                              FROM storyos.project_activity_event_payloads
                             WHERE project_id = $1::text::uuid
                               AND event_kind = 'volume_created'
                          )
                        )::text
                   FROM storyos.authority_history_floors AS floor
                   JOIN storyos.project_snapshots AS snapshot
                     ON (snapshot.owner_user_id, snapshot.project_id,
                         snapshot.snapshot_id) =
                        (floor.owner_user_id, floor.project_id, floor.snapshot_id)
                  WHERE floor.project_id = $1::text::uuid",
                &[&scope.project_id.as_ref()],
            )
            .await
            .unwrap()
            .get::<_, String>(0),
    )
    .unwrap();
    assert_eq!(
        observed,
        serde_json::json!({
            "replay_generation": 1,
            "floor_matches_snapshot": true,
            "replay_generation_count": 1,
            "commit_count": 1,
            "author_action_count": 1,
            "volume_created_payload": {
                "kind": "volume_created",
                "volume_id": volume_id,
                "title": TITLE,
                "tree_revision": "2",
                "order": "1",
            },
        })
    );
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
    let bound: serde_json::Value = serde_json::from_str(
        &admin
            .query_one(
                "SELECT jsonb_build_object(
                          'commit_id', authoritative_commit.authoritative_commit_id,
                          'prior_tree',
                            authoritative_commit.prior_manuscript_tree_revision,
                          'resulting_tree',
                            authoritative_commit.resulting_manuscript_tree_revision,
                          'affected_volume_id',
                            authoritative_commit.affected_volume_id,
                          'manuscript_object_id',
                            authoritative_commit.manuscript_object_id,
                          'prior_revision_id',
                            authoritative_commit.prior_revision_id,
                          'resulting_revision_id',
                            authoritative_commit.resulting_revision_id,
                          'action_commit_id', action.authoritative_commit_id,
                          'receipt_commit_ids', receipt.authoritative_commit_ids
                        )::text
                   FROM storyos.authoritative_commits AS authoritative_commit
                   JOIN storyos.author_action_entries AS action
                     ON (action.owner_user_id, action.project_id,
                         action.authoritative_commit_id) =
                        (authoritative_commit.owner_user_id,
                         authoritative_commit.project_id,
                         authoritative_commit.authoritative_commit_id)
                   JOIN storyos.domain_receipts AS receipt
                     ON (receipt.owner_user_id, receipt.project_id,
                         receipt.receipt_id) =
                        (authoritative_commit.owner_user_id,
                         authoritative_commit.project_id,
                         authoritative_commit.receipt_id)
                  WHERE authoritative_commit.project_id = $1::text::uuid",
                &[&scope.project_id.as_ref()],
            )
            .await
            .unwrap()
            .get::<_, String>(0),
    )
    .unwrap();
    let commit_id = bound["commit_id"].as_str().expect("commit id").to_owned();
    assert_eq!(
        bound,
        serde_json::json!({
            "commit_id": commit_id,
            "prior_tree": 1,
            "resulting_tree": 2,
            "affected_volume_id": volume_id,
            "manuscript_object_id": null,
            "prior_revision_id": null,
            "resulting_revision_id": null,
            "action_commit_id": commit_id,
            "receipt_commit_ids": [commit_id],
        })
    );
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
            /*expected_tree_revision*/ 1,
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
    assert_eq!(
        error.code(),
        Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn applied_create_chapter_receipt_may_bind_genesis_revision_pair() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let (scope, volume_id) = seed_project_with_volume(&store, "0840", "0842").await;
    let chapter_id = seed_chapter(
        &store,
        &scope,
        &volume_id,
        SeedChapter {
            suffix: "0844",
            title: "Chapter A",
            expected_tree_revision: 2,
            bytes: CHAPTER_A_BYTES,
            digest: CHAPTER_A_DIGEST,
        },
    )
    .await;
    let admin = open_admin().await;
    let bound: serde_json::Value = serde_json::from_str(
        &admin
            .query_one(
                "SELECT jsonb_build_object(
                          'prior_tree',
                            authoritative_commit.prior_manuscript_tree_revision,
                          'resulting_tree',
                            authoritative_commit.resulting_manuscript_tree_revision,
                          'affected_chapter_id',
                            authoritative_commit.affected_chapter_id,
                          'manuscript_object_id',
                            authoritative_commit.manuscript_object_id,
                          'prior_revision_id',
                            authoritative_commit.prior_revision_id,
                          'resulting_revision_id',
                            authoritative_commit.resulting_revision_id,
                          'head_revision_id', head.current_revision_id,
                          'action_commit_id', action.authoritative_commit_id,
                          'receipt_commit_ids', receipt.authoritative_commit_ids
                        )::text
                   FROM storyos.authoritative_commits AS authoritative_commit
                   JOIN storyos.author_action_entries AS action
                     ON (action.owner_user_id, action.project_id,
                         action.authoritative_commit_id) =
                        (authoritative_commit.owner_user_id,
                         authoritative_commit.project_id,
                         authoritative_commit.authoritative_commit_id)
                   JOIN storyos.domain_receipts AS receipt
                     ON (receipt.owner_user_id, receipt.project_id,
                         receipt.receipt_id) =
                        (authoritative_commit.owner_user_id,
                         authoritative_commit.project_id,
                         authoritative_commit.receipt_id)
                   JOIN storyos.authoritative_heads AS head
                     ON (head.owner_user_id, head.project_id,
                         head.manuscript_object_id) =
                        (authoritative_commit.owner_user_id,
                         authoritative_commit.project_id,
                         authoritative_commit.affected_chapter_id)
                  WHERE authoritative_commit.project_id = $1::text::uuid
                    AND authoritative_commit.affected_chapter_id = $2::text::uuid
                    AND receipt.command_kind = 'createChapter'",
                &[&scope.project_id.as_ref(), &chapter_id],
            )
            .await
            .unwrap()
            .get::<_, String>(0),
    )
    .unwrap();
    let resulting_revision_id = bound["resulting_revision_id"].clone();
    let commit_id = bound["action_commit_id"].clone();
    assert_eq!(
        bound,
        serde_json::json!({
            "prior_tree": 2,
            "resulting_tree": 3,
            "affected_chapter_id": chapter_id,
            "manuscript_object_id": chapter_id,
            "prior_revision_id": null,
            "resulting_revision_id": resulting_revision_id,
            "head_revision_id": resulting_revision_id,
            "action_commit_id": commit_id,
            "receipt_commit_ids": [commit_id],
        })
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn applied_set_current_chapter_receipt_may_bind_author_action_without_commit() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let (scope, volume_id) = seed_project_with_volume(&store, "0850", "0852").await;
    let chapter_a = seed_chapter(
        &store,
        &scope,
        &volume_id,
        SeedChapter {
            suffix: "0854",
            title: "Chapter A",
            expected_tree_revision: 2,
            bytes: CHAPTER_A_BYTES,
            digest: CHAPTER_A_DIGEST,
        },
    )
    .await;
    let chapter_b = seed_chapter(
        &store,
        &scope,
        &volume_id,
        SeedChapter {
            suffix: "0856",
            title: "Chapter B",
            expected_tree_revision: 3,
            bytes: CHAPTER_B_BYTES,
            digest: CHAPTER_B_DIGEST,
        },
    )
    .await;
    let session_issue = command_issue(
        &scope,
        "0858",
        "POST",
        "/api/v1/projects/{project_id}/editor-sessions",
        "storyos.command.create-editor-session.request.v1",
        "createEditorSession",
        "sha256:storyos.test:0858",
    );
    issue_project_command_challenge(&store, &session_issue)
        .await
        .unwrap();
    let editor_session_id = "018f0000-0000-7001-8000-000000000859";
    create_editor_session(
        &store,
        &OpenEditorSession {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(editor_session_id),
            snapshot_id: "018f0000-0000-7001-8000-00000000085a".to_owned(),
            client_binding: EditorClientBinding {
                binding_ref: session_issue.binding.client_session_binding_digest.clone(),
                session_generation: session_issue.binding.client_session_generation,
                client_contract_revision: session_issue.binding.client_contract_revision.clone(),
                security_policy_revision: session_issue.binding.security_policy_revision.clone(),
            },
            challenge_binding: session_issue.binding,
            nonce_digest: session_issue.nonce_digest,
        },
    )
    .await
    .unwrap();
    let OpenChapter::Found(opened_b) =
        open_chapter(&store, &scope, &ChapterId::new(chapter_b.clone()))
            .await
            .unwrap()
    else {
        panic!("Chapter B must open");
    };
    let switch_issue = command_issue(
        &scope,
        "085b",
        "PUT",
        "/api/v1/projects/{project_id}/current-chapter",
        "storyos.command.set-current-chapter.request.v1",
        "setCurrentChapter",
        CURRENT_DIGEST,
    );
    issue_project_command_challenge(&store, &switch_issue)
        .await
        .unwrap();
    let switched = set_current_chapter(
        &store,
        &SetCurrentChapterCommand {
            project_scope: switch_issue.binding.project_scope.clone(),
            client_binding: EditorClientBinding {
                binding_ref: switch_issue.binding.client_session_binding_digest.clone(),
                session_generation: switch_issue.binding.client_session_generation,
                client_contract_revision: switch_issue.binding.client_contract_revision.clone(),
                security_policy_revision: switch_issue.binding.security_policy_revision.clone(),
            },
            challenge_binding: switch_issue.binding.clone(),
            nonce_digest: switch_issue.nonce_digest.clone(),
            canonical_command_bytes: CURRENT_BYTES.to_vec(),
            correlation_id: "018f0000-0000-7001-8000-00000000085c".to_owned(),
            ids: AuthorCommandAdmissionIds {
                command_id: "018f0000-0000-7001-8000-00000001085c".to_owned(),
                author_command_admission_id: "018f0000-0000-7001-8000-00000002085c".to_owned(),
                receipt_id: "018f0000-0000-7001-8000-00000003085c".to_owned(),
            },
            editor_session_id: EditorSessionId::new(editor_session_id),
            chapter_id: chapter_b.clone(),
            expected_current_chapter_id: chapter_a,
            expected_target_revision_id: opened_b.chapter.revision_id.as_ref().to_owned(),
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        switched.effect,
        SetCurrentChapterSettlementEffect::Applied { .. }
    ));
    let admin = open_admin().await;
    admin.batch_execute("BEGIN").await.unwrap();
    // Create Volume and two Create Chapter settlements occupy sequences 1 through 3.
    admin
        .execute(
            "INSERT INTO storyos.author_action_entries
               (owner_user_id, project_id, author_action_sequence, disposition,
                receipt_id, receipt_result_kind)
             VALUES ($1::text::uuid, $2::text::uuid, 4, 'forward',
                     $3::text::uuid, 'authoritative_applied')",
            &[
                &USER_A,
                &scope.project_id.as_ref(),
                &switched.ids.receipt_id,
            ],
        )
        .await
        .unwrap();
    admin.batch_execute("COMMIT").await.unwrap();
    let bound: serde_json::Value = serde_json::from_str(
        &admin
            .query_one(
                "SELECT jsonb_build_object(
                          'command_kind', receipt.command_kind,
                          'action_sequence', action.author_action_sequence,
                          'action_commit_id', action.authoritative_commit_id,
                          'receipt_commit_ids', receipt.authoritative_commit_ids
                        )::text
                   FROM storyos.domain_receipts AS receipt
                   JOIN storyos.author_action_entries AS action
                     ON (action.owner_user_id, action.project_id, action.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                  WHERE receipt.project_id = $1::text::uuid
                    AND receipt.receipt_id = $2::text::uuid",
                &[&scope.project_id.as_ref(), &switched.ids.receipt_id],
            )
            .await
            .unwrap()
            .get::<_, String>(0),
    )
    .unwrap();
    assert_eq!(
        bound,
        serde_json::json!({
            "command_kind": "setCurrentChapter",
            "action_sequence": 4,
            "action_commit_id": null,
            "receipt_commit_ids": [],
        })
    );
    let error = admin
        .execute(
            "UPDATE storyos.domain_receipts
                SET authoritative_commit_ids = ARRAY[$3::text::uuid]
              WHERE project_id = $1::text::uuid
                AND receipt_id = $2::text::uuid",
            &[
                &scope.project_id.as_ref(),
                &switched.ids.receipt_id,
                &"018f0000-0000-7001-8000-00000000085d",
            ],
        )
        .await
        .expect_err("Applied setCurrentChapter must keep zero Commit identities");
    assert_eq!(
        error.code(),
        Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
    );
}
