use super::*;
use storyos_application::{
    AuthorCommandAdmissionIds, ChapterId, ChapterNode, CreateChapterCommand,
    CreateChapterSettlementEffect, CreateProjectChallengeBinding, CreateProjectCommand,
    CreateVolumeCommand, CreateVolumeSettlementEffect, EditorClientBinding, EditorSessionId,
    GetManuscriptTree, IssueCreateProjectChallenge, IssueProjectCommandChallenge, OpenChapter,
    OpenEditorSession, ProjectCommandChallengeBinding, ProjectId, ProjectScope,
    UndoLatestAuthorActionCommand, UndoLatestAuthorActionSettlementEffect, UserId, VolumeId,
    VolumeNode, create_chapter, create_editor_session, create_project, create_volume,
    get_manuscript_tree, issue_create_project_challenge, issue_project_command_challenge,
    open_chapter, open_project, undo_latest_author_action,
};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const CLIENT: &str = "storyos.web-client.release-1.v3";
const SECURITY: &str = "storyos.web-security-policy.release-1.v1";
const VOLUME_A_BYTES: &[u8] = br#"{"expected_tree_revision":"1","title":"Volume A"}"#;
const VOLUME_A_DIGEST: &str = "sha256:storyos.command.createVolume.jcs.v1:2b02ae40bec5ed5ccf7ec412519d2178186dc80e3829100851514a6f3422cedf";
const CHAPTER_A_BYTES: &[u8] = br#"{"expected_tree_revision":"2","title":"Chapter A"}"#;
const CHAPTER_A_DIGEST: &str = "sha256:storyos.command.createChapter.jcs.v1:fdee6f3020b94d08f6e75dfab8be7caf86d1a8c84c521254b03e6a45153a1de2";
const CHAPTER_B_BYTES: &[u8] = br#"{"expected_tree_revision":"3","title":"Chapter B"}"#;
const CHAPTER_B_DIGEST: &str = "sha256:storyos.command.createChapter.jcs.v1:23a99998cce1f35b113794d41235dae8fae7d8ae9f234ba67161a2f9e8e86e65";

fn create_project_issue(
    owner: &str,
    idempotency_key: &str,
    project_suffix: &str,
) -> IssueCreateProjectChallenge {
    let digest = format!("sha256:storyos.command.createProject.jcs.v1:{project_suffix:0<64}");
    IssueCreateProjectChallenge {
        binding: CreateProjectChallengeBinding {
            owner_user_id: UserId::new(owner),
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

fn command_issue(
    scope: &ProjectScope,
    suffix: &str,
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
            idempotency_key: format!("018f0000-0000-7001-8000-00000000{suffix}"),
        },
        nonce: format!("opaque-nonce-{suffix}"),
        nonce_digest: format!("sha256:nonce-{suffix}"),
    }
}

async fn seed_project(
    store: &PostgresProjectReader,
    owner: &str,
    project_suffix: &str,
) -> ProjectScope {
    let issue = create_project_issue(
        owner,
        &format!("018f0000-0000-7001-8000-00000000{project_suffix}"),
        project_suffix,
    );
    let issued = issue_create_project_challenge(store, &issue).await.unwrap();
    let mut binding = issue.binding.clone();
    binding.prospective_project_id = issued.prospective_project_id.clone();
    binding.canonical_command_digest = issued.canonical_command_digest.clone();
    create_project(
        store,
        &create_project_command(binding.clone(), &issue.nonce_digest, project_suffix),
    )
    .await
    .unwrap();
    ProjectScope::new(binding.owner_user_id, binding.prospective_project_id)
}

async fn post_volume(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
) -> storyos_application::CreateVolumeSettlement {
    let issue = command_issue(
        scope,
        suffix,
        "POST",
        "/api/v1/projects/{project_id}/volumes",
        "storyos.command.create-volume.request.v1",
        "createVolume",
        VOLUME_A_DIGEST,
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    create_volume(
        store,
        &CreateVolumeCommand {
            project_scope: scope.clone(),
            client_binding: EditorClientBinding {
                binding_ref: issue.binding.client_session_binding_digest.clone(),
                session_generation: issue.binding.client_session_generation,
                client_contract_revision: issue.binding.client_contract_revision.clone(),
                security_policy_revision: issue.binding.security_policy_revision.clone(),
            },
            challenge_binding: issue.binding,
            nonce_digest: issue.nonce_digest,
            canonical_command_bytes: VOLUME_A_BYTES.to_vec(),
            correlation_id: format!("018f0000-0000-7001-8000-00000000{suffix}"),
            title: "Volume A".to_owned(),
            expected_tree_revision: 1,
            ids: AuthorCommandAdmissionIds {
                command_id: format!("018f0000-0000-7001-8000-00000001{suffix}"),
                author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{suffix}"),
                receipt_id: format!("018f0000-0000-7001-8000-00000003{suffix}"),
            },
        },
    )
    .await
    .unwrap()
}

struct SeedChapter<'a> {
    suffix: &'a str,
    title: &'a str,
    bytes: &'a [u8],
    digest: &'a str,
    expected_tree_revision: u64,
}

async fn post_chapter(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    volume_id: &str,
    chapter: SeedChapter<'_>,
) -> storyos_application::CreateChapterSettlement {
    let SeedChapter {
        suffix,
        title,
        bytes,
        digest,
        expected_tree_revision,
    } = chapter;
    let issue = command_issue(
        scope,
        suffix,
        "POST",
        "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
        "storyos.command.create-chapter.request.v1",
        "createChapter",
        digest,
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    create_chapter(
        store,
        &CreateChapterCommand {
            project_scope: scope.clone(),
            client_binding: EditorClientBinding {
                binding_ref: issue.binding.client_session_binding_digest.clone(),
                session_generation: issue.binding.client_session_generation,
                client_contract_revision: issue.binding.client_contract_revision.clone(),
                security_policy_revision: issue.binding.security_policy_revision.clone(),
            },
            challenge_binding: issue.binding,
            nonce_digest: issue.nonce_digest,
            canonical_command_bytes: bytes.to_vec(),
            correlation_id: format!("018f0000-0000-7001-8000-00000000{suffix}"),
            volume_id: volume_id.to_owned(),
            title: title.to_owned(),
            expected_tree_revision,
            ids: AuthorCommandAdmissionIds {
                command_id: format!("018f0000-0000-7001-8000-00000001{suffix}"),
                author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{suffix}"),
                receipt_id: format!("018f0000-0000-7001-8000-00000003{suffix}"),
            },
        },
    )
    .await
    .unwrap()
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

async fn undo_named(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    editor_session_id: &str,
    expected_frontier: u64,
    expected_revision_id: &str,
    suffix: &str,
) -> storyos_application::UndoLatestAuthorActionSettlement {
    let input = serde_json::json!({
        "expected_author_undo_frontier_sequence": expected_frontier.to_string(),
        "expected_authoritative_revision_id": expected_revision_id,
        "editor_session_id": editor_session_id,
        "client_contract_revision": CLIENT,
        "security_policy_revision": SECURITY,
        "correlation_id": format!("018f0000-0000-7001-8000-00000000{suffix}"),
    });
    let body = serde_json::json!({
        "command_schema": "storyos.command.undo-latest-author-action.request.v1",
        "undo_latest_author_action_input": input,
    });
    let bytes = serde_json::to_vec(&body).unwrap();
    let digest = format!(
        "sha256:storyos.command.undoLatestAuthorAction.jcs.v1:{}",
        crate::author_edit::sha256_hex(&bytes)
    );
    let issue = command_issue(
        scope,
        suffix,
        "POST",
        "/api/v1/projects/{project_id}/author-actions/undo",
        "storyos.command.undo-latest-author-action.request.v1",
        "undoLatestAuthorAction",
        &digest,
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    undo_latest_author_action(
        store,
        &UndoLatestAuthorActionCommand {
            project_scope: scope.clone(),
            client_binding: EditorClientBinding {
                binding_ref: issue.binding.client_session_binding_digest.clone(),
                session_generation: issue.binding.client_session_generation,
                client_contract_revision: issue.binding.client_contract_revision.clone(),
                security_policy_revision: issue.binding.security_policy_revision.clone(),
            },
            challenge_binding: issue.binding,
            nonce_digest: issue.nonce_digest,
            canonical_command_bytes: bytes,
            correlation_id: format!("018f0000-0000-7001-8000-00000000{suffix}"),
            ids: AuthorCommandAdmissionIds {
                command_id: format!("018f0000-0000-7001-8000-00000001{suffix}"),
                author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{suffix}"),
                receipt_id: format!("018f0000-0000-7001-8000-00000003{suffix}"),
            },
            editor_session_id: EditorSessionId::new(editor_session_id),
            expected_author_undo_frontier_sequence: expected_frontier,
            expected_authoritative_revision_id: expected_revision_id.to_owned(),
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn two_users_and_two_projects_keep_separate_chapter_sequences() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let first = seed_project(&store, USER_A, "f710").await;
    let second = seed_project(&store, USER_A, "f712").await;
    let other_user = seed_project(&store, USER_B, "f714").await;
    let first_volume = post_volume(&store, &first, "f716").await;
    let second_volume = post_volume(&store, &second, "f718").await;
    let other_volume = post_volume(&store, &other_user, "f71a").await;
    let CreateVolumeSettlementEffect::Applied {
        volume_id: first_volume_id,
        ..
    } = first_volume.effect
    else {
        panic!("first Volume must apply");
    };
    let CreateVolumeSettlementEffect::Applied {
        volume_id: second_volume_id,
        ..
    } = second_volume.effect
    else {
        panic!("second Volume must apply");
    };
    let CreateVolumeSettlementEffect::Applied {
        volume_id: other_volume_id,
        ..
    } = other_volume.effect
    else {
        panic!("second User Volume must apply");
    };
    let first_chapter = post_chapter(
        &store,
        &first,
        &first_volume_id,
        SeedChapter {
            suffix: "f71c",
            title: "Chapter A",
            bytes: CHAPTER_A_BYTES,
            digest: CHAPTER_A_DIGEST,
            expected_tree_revision: 2,
        },
    )
    .await;
    let second_chapter = post_chapter(
        &store,
        &second,
        &second_volume_id,
        SeedChapter {
            suffix: "f71e",
            title: "Chapter A",
            bytes: CHAPTER_A_BYTES,
            digest: CHAPTER_A_DIGEST,
            expected_tree_revision: 2,
        },
    )
    .await;
    let other_chapter = post_chapter(
        &store,
        &other_user,
        &other_volume_id,
        SeedChapter {
            suffix: "f720",
            title: "Chapter A",
            bytes: CHAPTER_A_BYTES,
            digest: CHAPTER_A_DIGEST,
            expected_tree_revision: 2,
        },
    )
    .await;
    let first_authority = first_chapter.authority.expect("first Project authority");
    let second_authority = second_chapter.authority.expect("second Project authority");
    let other_authority = other_chapter.authority.expect("second User authority");
    assert_eq!(first_authority.author_action_sequence, 2);
    assert_eq!(second_authority.author_action_sequence, 2);
    assert_eq!(other_authority.author_action_sequence, 2);
    assert_ne!(
        first_authority.authoritative_commit_id,
        second_authority.authoritative_commit_id
    );
    assert_ne!(
        first_authority.authoritative_commit_id,
        other_authority.authoritative_commit_id
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_compensates_create_chapter_and_restores_the_initial_revision() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let scope = seed_project(&store, USER_A, "f722").await;
    let volume = post_volume(&store, &scope, "f724").await;
    let CreateVolumeSettlementEffect::Applied { volume_id, .. } = volume.effect else {
        panic!("Volume A must apply");
    };
    let chapter_a = post_chapter(
        &store,
        &scope,
        &volume_id,
        SeedChapter {
            suffix: "f726",
            title: "Chapter A",
            bytes: CHAPTER_A_BYTES,
            digest: CHAPTER_A_DIGEST,
            expected_tree_revision: 2,
        },
    )
    .await;
    let CreateChapterSettlementEffect::Applied {
        chapter_id: chapter_a_id,
        ..
    } = chapter_a.effect.clone()
    else {
        panic!("Chapter A must apply");
    };
    let chapter_a_authority = chapter_a.authority.clone().expect("Chapter A authority");
    let chapter_b = post_chapter(
        &store,
        &scope,
        &volume_id,
        SeedChapter {
            suffix: "f728",
            title: "Chapter B",
            bytes: CHAPTER_B_BYTES,
            digest: CHAPTER_B_DIGEST,
            expected_tree_revision: 3,
        },
    )
    .await;
    let CreateChapterSettlementEffect::Applied {
        chapter_id: chapter_b_id,
        ..
    } = chapter_b.effect.clone()
    else {
        panic!("Chapter B must apply");
    };
    let chapter_b_authority = chapter_b
        .authority
        .as_ref()
        .expect("Chapter B authority")
        .clone();
    let session_issue = command_issue(
        &scope,
        "f72a",
        "POST",
        "/api/v1/projects/{project_id}/editor-sessions",
        "storyos.command.create-editor-session.request.v1",
        "createEditorSession",
        "sha256:storyos.test:f72a",
    );
    issue_project_command_challenge(&store, &session_issue)
        .await
        .unwrap();
    let editor_session_id = "018f0000-0000-7001-8000-00000000f72b";
    create_editor_session(
        &store,
        &OpenEditorSession {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(editor_session_id),
            snapshot_id: "018f0000-0000-7001-8000-00000000f72c".to_owned(),
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
    let OpenChapter::Found(opened) =
        open_chapter(&store, &scope, &ChapterId::new(chapter_a_id.clone()))
            .await
            .unwrap()
    else {
        panic!("Chapter A must open");
    };
    assert_eq!(
        opened.chapter.revision_id.as_ref(),
        chapter_a_authority.resulting_revision_id
    );
    let chapter_b_undo = undo_named(
        &store,
        &scope,
        editor_session_id,
        chapter_b_authority.author_action_sequence,
        opened.chapter.revision_id.as_ref(),
        "f72e",
    )
    .await;
    let UndoLatestAuthorActionSettlementEffect::CompensatedStructure {
        source_sequence,
        snapshot_id,
        ..
    } = chapter_b_undo.effect.clone()
    else {
        panic!("Create Chapter Undo must write structure Compensation");
    };
    assert_eq!(source_sequence, chapter_b_authority.author_action_sequence);
    let GetManuscriptTree::Found(tree_after_b) = get_manuscript_tree(&store, &scope).await.unwrap()
    else {
        panic!("the tree remains after Chapter B Compensation");
    };
    assert_eq!(tree_after_b.tree_revision, 3);
    assert_eq!(tree_after_b.snapshot.snapshot_id, snapshot_id);
    assert_eq!(
        tree_after_b.volumes,
        vec![VolumeNode {
            volume_id: VolumeId::new(volume_id.clone()),
            title: "Volume A".to_owned(),
            order: 1,
            chapters: vec![ChapterNode {
                chapter_id: ChapterId::new(chapter_a_id.clone()),
                title: "Chapter A".to_owned(),
                order: 1,
            }],
        }]
    );
    assert_eq!(
        open_chapter(&store, &scope, &ChapterId::new(chapter_b_id.clone()))
            .await
            .unwrap(),
        OpenChapter::Missing
    );
    let chapter_a_undo = undo_named(
        &store,
        &scope,
        editor_session_id,
        chapter_a_authority.author_action_sequence,
        opened.chapter.revision_id.as_ref(),
        "f730",
    )
    .await;
    assert!(matches!(
        chapter_a_undo.effect,
        UndoLatestAuthorActionSettlementEffect::CompensatedStructure { source_sequence, .. }
            if source_sequence == chapter_a_authority.author_action_sequence
    ));
    let GetManuscriptTree::Found(tree_after_a) = get_manuscript_tree(&store, &scope).await.unwrap()
    else {
        panic!("the Volume remains after Chapter A Compensation");
    };
    assert_eq!(tree_after_a.tree_revision, 2);
    assert_eq!(
        tree_after_a.volumes,
        vec![VolumeNode {
            volume_id: VolumeId::new(volume_id),
            title: "Volume A".to_owned(),
            order: 1,
            chapters: Vec::new(),
        }]
    );
    assert_eq!(
        open_project(&store, &scope)
            .await
            .unwrap()
            .expect("the Project remains in exact Scope")
            .current_chapter_id,
        None
    );
    assert_eq!(
        open_chapter(&store, &scope, &ChapterId::new(chapter_a_id))
            .await
            .unwrap(),
        OpenChapter::Missing
    );
    let admin = open_admin().await;
    let observed: serde_json::Value = serde_json::from_str(
        &admin
            .query_one(
                "SELECT jsonb_build_object(
                          'delete_chapter_receipts', (
                            SELECT count(*) FROM storyos.domain_receipts
                             WHERE project_id = $1::text::uuid
                               AND command_kind = 'deleteChapter'
                          ),
                          'compensation_actions', (
                            SELECT count(*) FROM storyos.author_action_entries
                             WHERE project_id = $1::text::uuid
                               AND disposition = 'compensation'
                          ),
                          'forward_delete_actions', (
                            SELECT count(*) FROM storyos.author_action_entries
                             WHERE project_id = $1::text::uuid
                               AND disposition = 'forward'
                               AND receipt_id IN (
                                 SELECT receipt_id FROM storyos.domain_receipts
                                  WHERE project_id = $1::text::uuid
                                    AND command_kind = 'deleteChapter'
                               )
                          ),
                          'initial_revision_remains', (
                            SELECT count(*) FROM storyos.authoritative_revisions
                             WHERE project_id = $1::text::uuid
                               AND revision_id = $2::text::uuid
                          ),
                          'compensation_commits_without_revision_pair', (
                            SELECT count(*) FROM storyos.authoritative_commits AS authoritative_commit
                            JOIN storyos.author_action_entries AS action
                              ON (action.owner_user_id, action.project_id,
                                  action.authoritative_commit_id) =
                                 (authoritative_commit.owner_user_id,
                                  authoritative_commit.project_id,
                                  authoritative_commit.authoritative_commit_id)
                             WHERE authoritative_commit.project_id = $1::text::uuid
                               AND action.disposition = 'compensation'
                               AND authoritative_commit.affected_chapter_id IS NOT NULL
                               AND authoritative_commit.manuscript_object_id IS NULL
                               AND authoritative_commit.prior_revision_id IS NULL
                               AND authoritative_commit.resulting_revision_id IS NULL
                          )
                        )::text",
                &[
                    &scope.project_id.as_ref(),
                    &chapter_a_authority.resulting_revision_id,
                ],
            )
            .await
            .unwrap()
            .get::<_, String>(0),
    )
    .unwrap();
    assert_eq!(
        observed,
        serde_json::json!({
            "delete_chapter_receipts": 0,
            "compensation_actions": 2,
            "forward_delete_actions": 0,
            "initial_revision_remains": 1,
            "compensation_commits_without_revision_pair": 2,
        })
    );
}
