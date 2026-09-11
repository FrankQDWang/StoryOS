use super::*;
use storyos_application::{
    AuthorCommandAdmissionIds, ChapterId, ChapterNode, CreateChapterCommand,
    CreateChapterSettlementEffect, CreateProjectChallengeBinding, CreateProjectCommand,
    CreateVolumeCommand, CreateVolumeSettlementEffect, EditorClientBinding, EditorSessionId,
    IssueCreateProjectChallenge, IssueProjectCommandChallenge, OpenChapter, OpenEditorSession,
    ProjectCommandChallengeBinding, ProjectId, ProjectScope, SetCurrentChapterCommand,
    SetCurrentChapterSettlementEffect, UndoLatestAuthorActionCommand,
    UndoLatestAuthorActionSettlementEffect, UserId, VolumeId, VolumeNode, create_chapter,
    create_editor_session, create_project, create_volume, get_manuscript_tree,
    issue_create_project_challenge, issue_project_command_challenge, open_chapter, open_project,
    set_current_chapter, undo_latest_author_action,
};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const CLIENT: &str = "storyos.web-client.release-1.v3";
const SECURITY: &str = "storyos.web-security-policy.release-1.v1";
const VOLUME_BYTES: &[u8] = br#"{"expected_tree_revision":"1","title":"Volume A"}"#;
const VOLUME_DIGEST: &str = "sha256:storyos.command.createVolume.jcs.v1:2b02ae40bec5ed5ccf7ec412519d2178186dc80e3829100851514a6f3422cedf";
const CHAPTER_A_BYTES: &[u8] = br#"{"expected_tree_revision":"2","title":"Chapter A"}"#;
const CHAPTER_A_DIGEST: &str = "sha256:storyos.command.createChapter.jcs.v1:fdee6f3020b94d08f6e75dfab8be7caf86d1a8c84c521254b03e6a45153a1de2";
const CHAPTER_B_BYTES: &[u8] = br#"{"expected_tree_revision":"3","title":"Chapter B"}"#;
const CHAPTER_B_DIGEST: &str = "sha256:storyos.command.createChapter.jcs.v1:23a99998cce1f35b113794d41235dae8fae7d8ae9f234ba67161a2f9e8e86e65";
const CURRENT_BYTES: &[u8] = b"{}";
const CURRENT_DIGEST: &str = "sha256:storyos.command.setCurrentChapter.jcs.v1:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";

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

async fn post_volume(store: &PostgresProjectReader, scope: &ProjectScope, suffix: &str) -> String {
    let issue = command_issue(
        scope,
        suffix,
        "POST",
        "/api/v1/projects/{project_id}/volumes",
        "storyos.command.create-volume.request.v1",
        "createVolume",
        VOLUME_DIGEST,
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    let settlement = create_volume(
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
            canonical_command_bytes: VOLUME_BYTES.to_vec(),
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
    .unwrap();
    let CreateVolumeSettlementEffect::Applied { volume_id, .. } = settlement.effect else {
        panic!("Create Volume must apply");
    };
    volume_id
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
) -> String {
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
    let settlement = create_chapter(
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
    .unwrap();
    let CreateChapterSettlementEffect::Applied { chapter_id, .. } = settlement.effect else {
        panic!("Create Chapter must apply");
    };
    chapter_id
}

async fn open_session(store: &PostgresProjectReader, scope: &ProjectScope, suffix: &str) -> String {
    let issue = command_issue(
        scope,
        suffix,
        "POST",
        "/api/v1/projects/{project_id}/editor-sessions",
        "storyos.command.create-editor-session.request.v1",
        "createEditorSession",
        &format!("sha256:storyos.test:{suffix}"),
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    let editor_session_id = format!("018f0000-0000-7001-8000-00000001{suffix}");
    create_editor_session(
        store,
        &OpenEditorSession {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(editor_session_id.clone()),
            snapshot_id: format!("018f0000-0000-7001-8000-00000002{suffix}"),
            client_binding: EditorClientBinding {
                binding_ref: issue.binding.client_session_binding_digest.clone(),
                session_generation: issue.binding.client_session_generation,
                client_contract_revision: issue.binding.client_contract_revision.clone(),
                security_policy_revision: issue.binding.security_policy_revision.clone(),
            },
            challenge_binding: issue.binding,
            nonce_digest: issue.nonce_digest,
        },
    )
    .await
    .unwrap();
    editor_session_id
}

async fn switch_current(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    editor_session_id: &str,
    chapter_id: &str,
    expected_current_chapter_id: &str,
    expected_target_revision_id: &str,
) -> storyos_application::SetCurrentChapterSettlement {
    let issue = command_issue(
        scope,
        suffix,
        "PUT",
        "/api/v1/projects/{project_id}/current-chapter",
        "storyos.command.set-current-chapter.request.v1",
        "setCurrentChapter",
        CURRENT_DIGEST,
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    set_current_chapter(
        store,
        &SetCurrentChapterCommand {
            project_scope: scope.clone(),
            client_binding: EditorClientBinding {
                binding_ref: issue.binding.client_session_binding_digest.clone(),
                session_generation: issue.binding.client_session_generation,
                client_contract_revision: issue.binding.client_contract_revision.clone(),
                security_policy_revision: issue.binding.security_policy_revision.clone(),
            },
            challenge_binding: issue.binding,
            nonce_digest: issue.nonce_digest,
            canonical_command_bytes: CURRENT_BYTES.to_vec(),
            correlation_id: format!("018f0000-0000-7001-8000-00000000{suffix}"),
            ids: AuthorCommandAdmissionIds {
                command_id: format!("018f0000-0000-7001-8000-00000001{suffix}"),
                author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{suffix}"),
                receipt_id: format!("018f0000-0000-7001-8000-00000003{suffix}"),
            },
            editor_session_id: EditorSessionId::new(editor_session_id),
            chapter_id: chapter_id.to_owned(),
            expected_current_chapter_id: expected_current_chapter_id.to_owned(),
            expected_target_revision_id: expected_target_revision_id.to_owned(),
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

async fn seed_two_chapters(
    store: &PostgresProjectReader,
    owner: &str,
    project_suffix: &str,
    volume_suffix: &str,
    chapter_a_suffix: &str,
    chapter_b_suffix: &str,
) -> (ProjectScope, String, String, String) {
    let scope = seed_project(store, owner, project_suffix).await;
    let volume_id = post_volume(store, &scope, volume_suffix).await;
    let chapter_a = post_chapter(
        store,
        &scope,
        &volume_id,
        SeedChapter {
            suffix: chapter_a_suffix,
            title: "Chapter A",
            bytes: CHAPTER_A_BYTES,
            digest: CHAPTER_A_DIGEST,
            expected_tree_revision: 2,
        },
    )
    .await;
    let chapter_b = post_chapter(
        store,
        &scope,
        &volume_id,
        SeedChapter {
            suffix: chapter_b_suffix,
            title: "Chapter B",
            bytes: CHAPTER_B_BYTES,
            digest: CHAPTER_B_DIGEST,
            expected_tree_revision: 3,
        },
    )
    .await;
    (scope, volume_id, chapter_a, chapter_b)
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn applied_set_current_chapter_writes_action_snapshot_and_no_commit() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let (scope, volume_id, chapter_a, chapter_b) =
        seed_two_chapters(&store, USER_A, "f810", "f812", "f814", "f816").await;
    let other = seed_two_chapters(&store, USER_B, "f818", "f81a", "f81c", "f81e").await;
    let editor_session_id = open_session(&store, &scope, "f820").await;
    let OpenChapter::Found(opened_b) =
        open_chapter(&store, &scope, &ChapterId::new(chapter_b.clone()))
            .await
            .unwrap()
    else {
        panic!("Chapter B must open");
    };
    let revision_b = opened_b.chapter.revision_id.as_ref().to_owned();
    let first_issue = command_issue(
        &scope,
        "f822",
        "PUT",
        "/api/v1/projects/{project_id}/current-chapter",
        "storyos.command.set-current-chapter.request.v1",
        "setCurrentChapter",
        CURRENT_DIGEST,
    );
    issue_project_command_challenge(&store, &first_issue)
        .await
        .unwrap();
    let first_command = SetCurrentChapterCommand {
        project_scope: scope.clone(),
        client_binding: EditorClientBinding {
            binding_ref: first_issue.binding.client_session_binding_digest.clone(),
            session_generation: first_issue.binding.client_session_generation,
            client_contract_revision: first_issue.binding.client_contract_revision.clone(),
            security_policy_revision: first_issue.binding.security_policy_revision.clone(),
        },
        challenge_binding: first_issue.binding,
        nonce_digest: first_issue.nonce_digest,
        canonical_command_bytes: CURRENT_BYTES.to_vec(),
        correlation_id: "018f0000-0000-7001-8000-00000000f822".to_owned(),
        ids: AuthorCommandAdmissionIds {
            command_id: "018f0000-0000-7001-8000-00000001f822".to_owned(),
            author_command_admission_id: "018f0000-0000-7001-8000-00000002f822".to_owned(),
            receipt_id: "018f0000-0000-7001-8000-00000003f822".to_owned(),
        },
        editor_session_id: EditorSessionId::new(editor_session_id.clone()),
        chapter_id: chapter_b.clone(),
        expected_current_chapter_id: chapter_a.clone(),
        expected_target_revision_id: revision_b.clone(),
    };
    let first = set_current_chapter(&store, &first_command).await.unwrap();
    let SetCurrentChapterSettlementEffect::Applied {
        current_chapter_id, ..
    } = first.effect.clone()
    else {
        panic!("switching to Chapter B must apply");
    };
    assert_eq!(current_chapter_id, chapter_b);
    let authority = first
        .authority
        .clone()
        .expect("Applied Set Current Chapter must write a Forward Author Action");
    assert_eq!(authority.author_action_sequence, 4);
    assert_eq!(authority.manuscript_tree_revision, 4);
    let mut replay_command = first_command.clone();
    replay_command.ids = AuthorCommandAdmissionIds {
        command_id: "018f0000-0000-7001-8000-00000001f823".to_owned(),
        author_command_admission_id: "018f0000-0000-7001-8000-00000002f823".to_owned(),
        receipt_id: "018f0000-0000-7001-8000-00000003f823".to_owned(),
    };
    let replay = set_current_chapter(&store, &replay_command).await.unwrap();
    assert_eq!(replay, first);
    let tree = get_manuscript_tree(&store, &scope)
        .await
        .unwrap()
        .expect("the Canonical Manuscript Tree remains");
    assert_eq!(tree.tree_revision, 4);
    assert_eq!(tree.snapshot.snapshot_id, authority.snapshot_id);
    assert_eq!(
        tree.snapshot.project_activity_position,
        first.project_activity_position
    );
    assert_eq!(
        tree.volumes,
        vec![VolumeNode {
            volume_id: VolumeId::new(volume_id),
            title: "Volume A".to_owned(),
            order: 1,
            chapters: vec![
                ChapterNode {
                    chapter_id: ChapterId::new(chapter_a.clone()),
                    title: "Chapter A".to_owned(),
                    order: 1,
                },
                ChapterNode {
                    chapter_id: ChapterId::new(chapter_b.clone()),
                    title: "Chapter B".to_owned(),
                    order: 2,
                },
            ],
        }]
    );
    let other_session = open_session(&store, &other.0, "f826").await;
    let OpenChapter::Found(other_opened) =
        open_chapter(&store, &other.0, &ChapterId::new(other.3.clone()))
            .await
            .unwrap()
    else {
        panic!("the second User Chapter must open");
    };
    let other_switch = switch_current(
        &store,
        &other.0,
        "f824",
        &other_session,
        &other.3,
        &other.2,
        other_opened.chapter.revision_id.as_ref(),
    )
    .await;
    assert_eq!(
        other_switch
            .authority
            .expect("the second User switch must write its own Author Action")
            .author_action_sequence,
        4
    );
    let stale = switch_current(
        &store,
        &scope,
        "f828",
        &editor_session_id,
        &chapter_a,
        &chapter_a,
        &revision_b,
    )
    .await;
    assert!(matches!(
        stale.effect,
        SetCurrentChapterSettlementEffect::Conflicted {
            reason: storyos_core::SetCurrentChapterConflict::StaleCurrentChapter,
        }
    ));
    assert_eq!(stale.authority, None);
    let admin = open_admin().await;
    let observed: serde_json::Value = serde_json::from_str(
        &admin
            .query_one(
                "SELECT jsonb_build_object(
                          'tree_revision', project.tree_revision,
                          'switch_receipts', (
                            SELECT count(*) FROM storyos.domain_receipts
                             WHERE project_id = $1::text::uuid
                               AND command_kind = 'setCurrentChapter'
                          ),
                          'switch_activity', (
                            SELECT count(*) FROM storyos.project_activity_event_payloads
                             WHERE project_id = $1::text::uuid
                               AND event_kind = 'current_chapter_set'
                          ),
                          'switch_commits', (
                            SELECT count(*) FROM storyos.authoritative_commits
                             WHERE project_id = $1::text::uuid
                               AND receipt_id = $2::text::uuid
                          ),
                          'forward_switch_actions', (
                            SELECT count(*) FROM storyos.author_action_entries
                             WHERE project_id = $1::text::uuid
                               AND disposition = 'forward'
                               AND receipt_id = $2::text::uuid
                               AND authoritative_commit_id IS NULL
                          ),
                          'stale_actions', (
                            SELECT count(*) FROM storyos.author_action_entries
                             WHERE project_id = $1::text::uuid
                               AND receipt_id = $3::text::uuid
                          ),
                          'stale_snapshots', (
                            SELECT count(*) FROM storyos.project_snapshots
                             WHERE project_id = $1::text::uuid
                               AND project_activity_position = $4::text::numeric
                          )
                        )::text
                   FROM storyos.projects AS project
                  WHERE project.project_id = $1::text::uuid",
                &[
                    &scope.project_id.as_ref(),
                    &first.ids.receipt_id,
                    &stale.ids.receipt_id,
                    &stale.project_activity_position.to_string(),
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
            "tree_revision": 4,
            "switch_receipts": 2,
            "switch_activity": 1,
            "switch_commits": 0,
            "forward_switch_actions": 1,
            "stale_actions": 0,
            "stale_snapshots": 0,
        })
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_restores_prior_current_chapter_or_stays_unavailable() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let (scope, volume_id, chapter_a, chapter_b) =
        seed_two_chapters(&store, USER_A, "f830", "f832", "f834", "f836").await;
    let editor_session_id = open_session(&store, &scope, "f838").await;
    let OpenChapter::Found(opened_b) =
        open_chapter(&store, &scope, &ChapterId::new(chapter_b.clone()))
            .await
            .unwrap()
    else {
        panic!("Chapter B must open");
    };
    let revision_b = opened_b.chapter.revision_id.as_ref().to_owned();
    let switched = switch_current(
        &store,
        &scope,
        "f83a",
        &editor_session_id,
        &chapter_b,
        &chapter_a,
        &revision_b,
    )
    .await;
    let authority = switched
        .authority
        .as_ref()
        .expect("the switch must write a Forward Author Action");
    let compensated = undo_named(
        &store,
        &scope,
        &editor_session_id,
        authority.author_action_sequence,
        &revision_b,
        "f83c",
    )
    .await;
    let UndoLatestAuthorActionSettlementEffect::CompensatedCurrentChapter {
        source_sequence,
        snapshot_id,
        ..
    } = compensated.effect.clone()
    else {
        panic!("Set Current Chapter Undo must write Current Chapter Compensation");
    };
    assert_eq!(source_sequence, authority.author_action_sequence);
    assert_eq!(
        open_project(&store, &scope)
            .await
            .unwrap()
            .expect("the Project remains in exact Scope")
            .current_chapter_id,
        Some(ChapterId::new(chapter_a.clone()))
    );
    let tree = get_manuscript_tree(&store, &scope)
        .await
        .unwrap()
        .expect("the tree remains after Current Chapter Compensation");
    assert_eq!(tree.tree_revision, 4);
    assert_eq!(tree.snapshot.snapshot_id, snapshot_id);
    assert_eq!(
        tree.volumes,
        vec![VolumeNode {
            volume_id: VolumeId::new(volume_id),
            title: "Volume A".to_owned(),
            order: 1,
            chapters: vec![
                ChapterNode {
                    chapter_id: ChapterId::new(chapter_a.clone()),
                    title: "Chapter A".to_owned(),
                    order: 1,
                },
                ChapterNode {
                    chapter_id: ChapterId::new(chapter_b.clone()),
                    title: "Chapter B".to_owned(),
                    order: 2,
                },
            ],
        }]
    );
    let admin = open_admin().await;
    let after_restore: serde_json::Value = serde_json::from_str(
        &admin
            .query_one(
                "SELECT jsonb_build_object(
                          'forward_switch_actions', (
                            SELECT count(*) FROM storyos.author_action_entries
                             WHERE project_id = $1::text::uuid
                               AND disposition = 'forward'
                               AND receipt_id = $2::text::uuid
                          ),
                          'compensation_actions', (
                            SELECT count(*) FROM storyos.author_action_entries
                             WHERE project_id = $1::text::uuid
                               AND disposition = 'compensation'
                               AND compensated_source_sequence = $3::text::numeric
                               AND authoritative_commit_id IS NULL
                          ),
                          'compensation_commits', (
                            SELECT count(*) FROM storyos.authoritative_commits
                             WHERE project_id = $1::text::uuid
                               AND receipt_id = $4::text::uuid
                          )
                        )::text",
                &[
                    &scope.project_id.as_ref(),
                    &switched.ids.receipt_id,
                    &source_sequence.to_string(),
                    &compensated.ids.receipt_id,
                ],
            )
            .await
            .unwrap()
            .get::<_, String>(0),
    )
    .unwrap();
    assert_eq!(
        after_restore,
        serde_json::json!({
            "forward_switch_actions": 1,
            "compensation_actions": 1,
            "compensation_commits": 0,
        })
    );

    let (blocked_scope, _, blocked_a, blocked_b) =
        seed_two_chapters(&store, USER_A, "f840", "f842", "f844", "f846").await;
    let blocked_session = open_session(&store, &blocked_scope, "f848").await;
    let OpenChapter::Found(blocked_opened) =
        open_chapter(&store, &blocked_scope, &ChapterId::new(blocked_b.clone()))
            .await
            .unwrap()
    else {
        panic!("the blocked Project Chapter B must open");
    };
    let blocked_switch = switch_current(
        &store,
        &blocked_scope,
        "f84a",
        &blocked_session,
        &blocked_b,
        &blocked_a,
        blocked_opened.chapter.revision_id.as_ref(),
    )
    .await;
    let blocked_authority = blocked_switch
        .authority
        .as_ref()
        .expect("the blocked switch must write a Forward Author Action");
    admin
        .execute(
            "UPDATE storyos.project_activity_event_payloads
                SET payload = jsonb_set(
                  payload,
                  '{prior_chapter_id}',
                  to_jsonb($3::text)
                )
              WHERE project_id = $1::text::uuid AND receipt_id = $2::text::uuid",
            &[
                &blocked_scope.project_id.as_ref(),
                &blocked_switch.ids.receipt_id,
                &"018f0000-0000-7001-8000-00000000ffff",
            ],
        )
        .await
        .unwrap();
    let blocked = undo_named(
        &store,
        &blocked_scope,
        &blocked_session,
        blocked_authority.author_action_sequence,
        blocked_opened.chapter.revision_id.as_ref(),
        "f84c",
    )
    .await;
    assert!(matches!(
        blocked.effect,
        UndoLatestAuthorActionSettlementEffect::Unavailable {
            reason: storyos_core::UndoLatestAuthorActionUnavailable::Barrier,
        }
    ));
    assert_eq!(
        open_project(&store, &blocked_scope)
            .await
            .unwrap()
            .expect("the blocked Project remains in exact Scope")
            .current_chapter_id,
        Some(ChapterId::new(blocked_b))
    );
    let invented = admin
        .query_one(
            "SELECT count(*) FROM storyos.manuscript_objects
              WHERE project_id = $1::text::uuid
                AND manuscript_object_id = $2::text::uuid",
            &[
                &blocked_scope.project_id.as_ref(),
                &"018f0000-0000-7001-8000-00000000ffff",
            ],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(invented, 0);
    let blocked_actions = admin
        .query_one(
            "SELECT count(*) FROM storyos.author_action_entries
              WHERE project_id = $1::text::uuid AND receipt_id = $2::text::uuid",
            &[&blocked_scope.project_id.as_ref(), &blocked.ids.receipt_id],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(blocked_actions, 0);
}
