use super::*;
use storyos_application::{
    AuthorCommandAdmissionIds, ChapterId, ChapterNode, CreateChapterCommand,
    CreateChapterSettlementEffect, CreateProjectChallengeBinding, CreateProjectCommand,
    CreateVolumeCommand, CreateVolumeSettlementEffect, DeleteVolumeCommand,
    DeleteVolumeSettlementEffect, EditorClientBinding, EditorSessionId, GetManuscriptTree,
    IssueCreateProjectChallenge, IssueProjectCommandChallenge, OpenChapter, OpenEditorSession,
    ProjectCommandChallengeBinding, ProjectId, ProjectScope, UndoLatestAuthorActionCommand,
    UndoLatestAuthorActionSettlementEffect, UserId, VolumeId, VolumeNode, create_chapter,
    create_editor_session, create_project, create_volume, delete_volume, get_manuscript_tree,
    issue_create_project_challenge, issue_project_command_challenge, open_chapter,
    undo_latest_author_action,
};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const CLIENT: &str = "storyos.web-client.release-1.v3";
const SECURITY: &str = "storyos.web-security-policy.release-1.v1";
const VOLUME_A_TITLE: &str = "Volume A";
const VOLUME_B_TITLE: &str = "Volume B";
const VOLUME_A_BYTES: &[u8] = br#"{"expected_tree_revision":"1","title":"Volume A"}"#;
const VOLUME_A_DIGEST: &str = "sha256:storyos.command.createVolume.jcs.v1:2b02ae40bec5ed5ccf7ec412519d2178186dc80e3829100851514a6f3422cedf";

fn ids(suffix: &str) -> AuthorCommandAdmissionIds {
    AuthorCommandAdmissionIds {
        command_id: format!("018f0000-0000-7001-8000-00000001{suffix}"),
        author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{suffix}"),
        receipt_id: format!("018f0000-0000-7001-8000-00000003{suffix}"),
    }
}

fn client_binding(binding: &ProjectCommandChallengeBinding) -> EditorClientBinding {
    EditorClientBinding {
        binding_ref: binding.client_session_binding_digest.clone(),
        session_generation: binding.client_session_generation,
        client_contract_revision: binding.client_contract_revision.clone(),
        security_policy_revision: binding.security_policy_revision.clone(),
    }
}

fn named_issue(
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

fn delete_command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    suffix: &str,
    volume_id: &str,
    expected_tree_revision: u64,
    bytes: &[u8],
) -> DeleteVolumeCommand {
    DeleteVolumeCommand {
        project_scope: binding.project_scope.clone(),
        client_binding: client_binding(&binding),
        challenge_binding: binding,
        nonce_digest: nonce_digest.to_owned(),
        canonical_command_bytes: bytes.to_vec(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{suffix}"),
        volume_id: VolumeId::new(volume_id),
        expected_tree_revision,
        ids: ids(suffix),
    }
}

async fn seed_project(store: &PostgresProjectReader, suffix: &str) -> ProjectScope {
    let digest = format!("sha256:storyos.command.createProject.jcs.v1:{suffix:0<64}");
    let issue = IssueCreateProjectChallenge {
        binding: CreateProjectChallengeBinding {
            owner_user_id: UserId::new(USER_A),
            prospective_project_id: ProjectId::new(format!(
                "018f0000-0000-7001-8000-00000000{suffix}"
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
            create_input_digest: format!("{suffix:0<64}"),
            canonical_command_digest: digest,
            idempotency_key: format!("018f0000-0000-7001-8000-00000000{suffix}"),
        },
        nonce_digest: format!("sha256:nonce-{suffix}"),
    };
    let issued = issue_create_project_challenge(store, &issue).await.unwrap();
    let mut binding = issue.binding.clone();
    binding.prospective_project_id = issued.prospective_project_id.clone();
    binding.canonical_command_digest = issued.canonical_command_digest.clone();
    create_project(
        store,
        &CreateProjectCommand {
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
            challenge_binding: binding.clone(),
            nonce_digest: issue.nonce_digest,
            canonical_command_bytes: br#"{"title":"Empty Novel"}"#.to_vec(),
            correlation_id: format!("018f0000-0000-7001-8000-00000000{suffix}"),
            title: "Empty Novel".to_owned(),
            ids: ids(suffix),
        },
    )
    .await
    .unwrap();
    ProjectScope::new(binding.owner_user_id, binding.prospective_project_id)
}

async fn apply_volume(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    title: &str,
    expected_tree_revision: u64,
    bytes: &[u8],
) -> String {
    let digest = if bytes == VOLUME_A_BYTES {
        VOLUME_A_DIGEST.to_owned()
    } else {
        format!(
            "sha256:storyos.command.createVolume.jcs.v1:{}",
            crate::author_edit::sha256_hex(bytes)
        )
    };
    let issue = named_issue(
        scope,
        suffix,
        "POST",
        "/api/v1/projects/{project_id}/volumes",
        "storyos.command.create-volume.request.v1",
        "createVolume",
        &digest,
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    let settlement = create_volume(
        store,
        &CreateVolumeCommand {
            project_scope: issue.binding.project_scope.clone(),
            client_binding: client_binding(&issue.binding),
            challenge_binding: issue.binding,
            nonce_digest: issue.nonce_digest,
            canonical_command_bytes: bytes.to_vec(),
            correlation_id: format!("018f0000-0000-7001-8000-00000000{suffix}"),
            title: title.to_owned(),
            expected_tree_revision,
            ids: ids(suffix),
        },
    )
    .await
    .unwrap();
    let CreateVolumeSettlementEffect::Applied { volume_id, .. } = settlement.effect else {
        panic!("{title} must apply");
    };
    volume_id
}

async fn apply_chapter(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    volume_id: &str,
    title: &str,
    expected_tree_revision: u64,
    bytes: &[u8],
) -> String {
    let digest = format!(
        "sha256:storyos.command.createChapter.jcs.v1:{}",
        crate::author_edit::sha256_hex(bytes)
    );
    let issue = named_issue(
        scope,
        suffix,
        "POST",
        "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
        "storyos.command.create-chapter.request.v1",
        "createChapter",
        &digest,
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    let settlement = create_chapter(
        store,
        &CreateChapterCommand {
            project_scope: issue.binding.project_scope.clone(),
            client_binding: client_binding(&issue.binding),
            challenge_binding: issue.binding,
            nonce_digest: issue.nonce_digest,
            canonical_command_bytes: bytes.to_vec(),
            correlation_id: format!("018f0000-0000-7001-8000-00000000{suffix}"),
            volume_id: volume_id.to_owned(),
            title: title.to_owned(),
            expected_tree_revision,
            ids: ids(suffix),
        },
    )
    .await
    .unwrap();
    let CreateChapterSettlementEffect::Applied { chapter_id, .. } = settlement.effect else {
        panic!("{title} must apply");
    };
    chapter_id
}

async fn apply_delete(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    volume_id: &str,
    expected_tree_revision: u64,
    bytes: &[u8],
) -> storyos_application::DeleteVolumeSettlement {
    let digest = format!(
        "sha256:storyos.command.deleteVolume.jcs.v1:{}",
        crate::author_edit::sha256_hex(bytes)
    );
    let issue = named_issue(
        scope,
        suffix,
        "DELETE",
        "/api/v1/projects/{project_id}/volumes/{volume_id}",
        "storyos.command.delete-volume.request.v1",
        "deleteVolume",
        &digest,
    );
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    delete_volume(
        store,
        &delete_command(
            issue.binding,
            &issue.nonce_digest,
            suffix,
            volume_id,
            expected_tree_revision,
            bytes,
        ),
    )
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn delete_volume_is_atomic_replayable_and_scope_safe() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let scope = seed_project(&store, "e450").await;
    let volume_a = apply_volume(
        &store,
        &scope,
        "e452",
        VOLUME_A_TITLE,
        /*expected_tree_revision*/ 1,
        VOLUME_A_BYTES,
    )
    .await;
    let volume_b = apply_volume(
        &store,
        &scope,
        "e454",
        VOLUME_B_TITLE,
        /*expected_tree_revision*/ 2,
        br#"{"expected_tree_revision":"2","title":"Volume B"}"#,
    )
    .await;
    let chapter_b = apply_chapter(
        &store,
        &scope,
        "e456",
        &volume_b,
        "Chapter B",
        /*expected_tree_revision*/ 3,
        br#"{"expected_tree_revision":"3","title":"Chapter B"}"#,
    )
    .await;
    let nonempty = apply_delete(
        &store,
        &scope,
        "e457",
        &volume_b,
        /*expected_tree_revision*/ 4,
        br#"{"expected_tree_revision":"4"}"#,
    )
    .await;
    assert_eq!(
        nonempty.effect,
        DeleteVolumeSettlementEffect::Refused {
            reason: storyos_core::DeleteVolumeRefusal::NonemptyVolume,
        }
    );
    assert_eq!(nonempty.authority, None);

    let delete_bytes = br#"{"expected_tree_revision":"4"}"#;
    let delete_digest = format!(
        "sha256:storyos.command.deleteVolume.jcs.v1:{}",
        crate::author_edit::sha256_hex(delete_bytes)
    );
    let first_issue = named_issue(
        &scope,
        "e458",
        "DELETE",
        "/api/v1/projects/{project_id}/volumes/{volume_id}",
        "storyos.command.delete-volume.request.v1",
        "deleteVolume",
        &delete_digest,
    );
    issue_project_command_challenge(&store, &first_issue)
        .await
        .unwrap();
    let first = delete_volume(
        &store,
        &delete_command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "e458",
            &volume_a,
            /*expected_tree_revision*/ 4,
            delete_bytes,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        first.effect,
        DeleteVolumeSettlementEffect::Applied {
            tree_revision: 5,
            volume_id: volume_a.clone(),
        }
    );
    let authority = first
        .authority
        .clone()
        .expect("Applied Delete Volume must write Structural Authority Settlement");
    assert_eq!(authority.prior_manuscript_tree_revision, 4);
    assert_eq!(authority.resulting_manuscript_tree_revision, 5);
    assert_eq!(authority.author_action_sequence, 4);
    let replay = delete_volume(
        &store,
        &delete_command(
            first_issue.binding,
            &first_issue.nonce_digest,
            "e499",
            &volume_a,
            /*expected_tree_revision*/ 4,
            delete_bytes,
        ),
    )
    .await
    .unwrap();
    assert_eq!(replay, first);

    let GetManuscriptTree::Found(tree) = get_manuscript_tree(&store, &scope).await.unwrap() else {
        panic!("the Project still has a Canonical Query");
    };
    assert_eq!(tree.tree_revision, 5);
    assert_eq!(tree.snapshot.snapshot_id, authority.snapshot_id);
    assert_eq!(
        tree.snapshot.project_activity_position,
        first.project_activity_position
    );
    assert_eq!(
        tree.volumes,
        vec![VolumeNode {
            volume_id: VolumeId::new(volume_b.clone()),
            title: VOLUME_B_TITLE.to_owned(),
            order: 1,
            chapters: vec![ChapterNode {
                chapter_id: ChapterId::new(chapter_b),
                title: "Chapter B".to_owned(),
                order: 1,
            }],
        }]
    );

    let already = apply_delete(
        &store,
        &scope,
        "e45a",
        &volume_a,
        /*expected_tree_revision*/ 5,
        br#"{"expected_tree_revision":"5"}"#,
    )
    .await;
    assert_eq!(
        already.effect,
        DeleteVolumeSettlementEffect::NoEffect {
            reason: storyos_core::DeleteVolumeNoEffect::AlreadyRemoved,
        }
    );
    assert_eq!(already.authority, None);

    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    let row = admin
        .query_one(
            "SELECT tree_revision::text,
                    (SELECT count(*) FROM storyos.volume_removal_decisions
                      WHERE project_id = $1::text::uuid AND volume_id = $2::text::uuid),
                    (SELECT count(*) FROM storyos.project_activity_event_payloads
                      WHERE project_id = $1::text::uuid AND event_kind = 'volume_deleted'),
                    (SELECT manuscript_object_id IS NULL
                              AND prior_revision_id IS NULL
                              AND resulting_revision_id IS NULL
                              AND affected_volume_id = $2::text::uuid
                              AND prior_manuscript_tree_revision = 4
                              AND resulting_manuscript_tree_revision = 5
                       FROM storyos.authoritative_commits
                      WHERE project_id = $1::text::uuid
                        AND authoritative_commit_id = $3::text::uuid),
                    (SELECT count(*) FROM storyos.authoritative_commits
                      WHERE receipt_id = ANY($4::text[]::uuid[]))
               FROM storyos.projects
              WHERE project_id = $1::text::uuid",
            &[
                &scope.project_id.as_ref(),
                &volume_a,
                &authority.authoritative_commit_id,
                &vec![
                    nonempty.ids.receipt_id.clone(),
                    already.ids.receipt_id.clone(),
                ],
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            row.get::<_, String>(0),
            row.get::<_, i64>(1),
            row.get::<_, i64>(2),
            row.get::<_, bool>(3),
            row.get::<_, i64>(4)
        ),
        ("5".to_owned(), 1, 1, true, 0)
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_compensates_delete_volume_and_restores_prior_volume_identity() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let scope = seed_project(&store, "e470").await;
    let volume_a = apply_volume(
        &store,
        &scope,
        "e472",
        VOLUME_A_TITLE,
        /*expected_tree_revision*/ 1,
        VOLUME_A_BYTES,
    )
    .await;
    let chapter_a = apply_chapter(
        &store,
        &scope,
        "e474",
        &volume_a,
        "Chapter A",
        /*expected_tree_revision*/ 2,
        br#"{"expected_tree_revision":"2","title":"Chapter A"}"#,
    )
    .await;
    let volume_b = apply_volume(
        &store,
        &scope,
        "e476",
        VOLUME_B_TITLE,
        /*expected_tree_revision*/ 3,
        br#"{"expected_tree_revision":"3","title":"Volume B"}"#,
    )
    .await;
    let deleted = apply_delete(
        &store,
        &scope,
        "e478",
        &volume_b,
        /*expected_tree_revision*/ 4,
        br#"{"expected_tree_revision":"4"}"#,
    )
    .await;
    assert_eq!(
        deleted.effect,
        DeleteVolumeSettlementEffect::Applied {
            tree_revision: 5,
            volume_id: volume_b.clone(),
        }
    );
    let authority = deleted
        .authority
        .clone()
        .expect("Applied Delete Volume must write authority");
    let session_issue = named_issue(
        &scope,
        "e47a",
        "POST",
        "/api/v1/projects/{project_id}/editor-sessions",
        "storyos.command.create-editor-session.request.v1",
        "createEditorSession",
        "sha256:storyos.test:e47a",
    );
    issue_project_command_challenge(&store, &session_issue)
        .await
        .unwrap();
    let editor_session_id = "018f0000-0000-7001-8000-00000000e47b";
    create_editor_session(
        &store,
        &OpenEditorSession {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(editor_session_id),
            snapshot_id: "018f0000-0000-7001-8000-00000000e47c".to_owned(),
            client_binding: client_binding(&session_issue.binding),
            challenge_binding: session_issue.binding,
            nonce_digest: session_issue.nonce_digest,
        },
    )
    .await
    .unwrap();
    let OpenChapter::Found(opened) =
        open_chapter(&store, &scope, &ChapterId::new(chapter_a.clone()))
            .await
            .unwrap()
    else {
        panic!("Chapter A must open");
    };
    let undo_bytes = serde_json::to_vec(&serde_json::json!({
        "command_schema": "storyos.command.undo-latest-author-action.request.v1",
        "undo_latest_author_action_input": {
            "expected_author_undo_frontier_sequence": authority.author_action_sequence.to_string(),
            "expected_authoritative_revision_id": opened.chapter.revision_id.as_ref(),
            "editor_session_id": editor_session_id,
            "client_contract_revision": CLIENT,
            "security_policy_revision": SECURITY,
            "correlation_id": "018f0000-0000-7001-8000-00000000e47d",
        },
    }))
    .unwrap();
    let undo_digest = format!(
        "sha256:storyos.command.undoLatestAuthorAction.jcs.v1:{}",
        crate::author_edit::sha256_hex(&undo_bytes)
    );
    let undo_issue = named_issue(
        &scope,
        "e47d",
        "POST",
        "/api/v1/projects/{project_id}/author-actions/undo",
        "storyos.command.undo-latest-author-action.request.v1",
        "undoLatestAuthorAction",
        &undo_digest,
    );
    issue_project_command_challenge(&store, &undo_issue)
        .await
        .unwrap();
    let undone = undo_latest_author_action(
        &store,
        &UndoLatestAuthorActionCommand {
            project_scope: scope.clone(),
            client_binding: client_binding(&undo_issue.binding),
            challenge_binding: undo_issue.binding,
            nonce_digest: undo_issue.nonce_digest,
            canonical_command_bytes: undo_bytes,
            correlation_id: "018f0000-0000-7001-8000-00000000e47d".to_owned(),
            ids: ids("e47d"),
            editor_session_id: EditorSessionId::new(editor_session_id),
            expected_author_undo_frontier_sequence: authority.author_action_sequence,
            expected_authoritative_revision_id: opened.chapter.revision_id.as_ref().to_owned(),
        },
    )
    .await
    .unwrap();
    let UndoLatestAuthorActionSettlementEffect::CompensatedStructure {
        source_sequence,
        snapshot_id,
        ..
    } = undone.effect.clone()
    else {
        panic!("Delete Volume Undo must write structure Compensation");
    };
    assert_eq!(source_sequence, authority.author_action_sequence);
    let GetManuscriptTree::Found(tree) = get_manuscript_tree(&store, &scope).await.unwrap() else {
        panic!("the tree remains after Delete Volume Compensation");
    };
    assert_eq!(tree.tree_revision, 4);
    assert_eq!(tree.snapshot.snapshot_id, snapshot_id);
    assert_eq!(
        tree.volumes,
        vec![
            VolumeNode {
                volume_id: VolumeId::new(volume_a),
                title: VOLUME_A_TITLE.to_owned(),
                order: 1,
                chapters: vec![ChapterNode {
                    chapter_id: ChapterId::new(chapter_a),
                    title: "Chapter A".to_owned(),
                    order: 1,
                }],
            },
            VolumeNode {
                volume_id: VolumeId::new(volume_b),
                title: VOLUME_B_TITLE.to_owned(),
                order: 2,
                chapters: Vec::new(),
            },
        ]
    );

    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    let observed: serde_json::Value = serde_json::from_str(
        &admin
            .query_one(
                "SELECT jsonb_build_object(
                          'delete_volume_receipts', (
                            SELECT count(*) FROM storyos.domain_receipts
                             WHERE project_id = $1::text::uuid
                               AND command_kind = 'deleteVolume'
                          ),
                          'create_volume_receipts_after_undo', (
                            SELECT count(*) FROM storyos.domain_receipts
                             WHERE project_id = $1::text::uuid
                               AND command_kind = 'createVolume'
                               AND receipt_id = $2::text::uuid
                          ),
                          'compensation_actions', (
                            SELECT count(*) FROM storyos.author_action_entries
                             WHERE project_id = $1::text::uuid
                               AND disposition = 'compensation'
                               AND compensated_source_sequence = $3::text::numeric
                          ),
                          'live_removal_decisions', (
                            SELECT count(*) FROM storyos.volume_removal_decisions
                             WHERE project_id = $1::text::uuid
                          )
                        )::text",
                &[
                    &scope.project_id.as_ref(),
                    &undone.ids.receipt_id,
                    &authority.author_action_sequence.to_string(),
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
            "delete_volume_receipts": 1,
            "create_volume_receipts_after_undo": 0,
            "compensation_actions": 1,
            "live_removal_decisions": 0,
        })
    );
}
