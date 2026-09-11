use super::*;
use storyos_application::{
    AuthorCommandAdmissionIds, ChapterId, ChapterNode, CreateChapterCommand,
    CreateChapterSettlementEffect, CreateProjectChallengeBinding, CreateProjectCommand,
    CreateVolumeCommand, CreateVolumeSettlementEffect, DeleteChapterCommand,
    DeleteChapterSettlementEffect, EditorClientBinding, EditorSessionId,
    IssueCreateProjectChallenge, IssueProjectCommandChallenge, OpenChapter, OpenEditorSession,
    ProjectCommandChallengeBinding, ProjectId, ProjectScope, UndoLatestAuthorActionCommand,
    UndoLatestAuthorActionSettlementEffect, UserId, VolumeId, VolumeNode, create_chapter,
    create_editor_session, create_project, create_volume, delete_chapter, get_manuscript_tree,
    issue_create_project_challenge, issue_project_command_challenge, open_chapter, open_project,
    undo_latest_author_action,
};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const CLIENT: &str = "storyos.web-client.release-1.v3";
const SECURITY: &str = "storyos.web-security-policy.release-1.v1";
const VOLUME_TITLE: &str = "Volume A";
const VOLUME_BYTES: &[u8] = br#"{"expected_tree_revision":"1","title":"Volume A"}"#;
const VOLUME_DIGEST: &str = "sha256:storyos.command.createVolume.jcs.v1:2b02ae40bec5ed5ccf7ec412519d2178186dc80e3829100851514a6f3422cedf";
const CHAPTER_A_BYTES: &[u8] = br#"{"expected_tree_revision":"2","title":"Chapter A"}"#;
const CHAPTER_B_BYTES: &[u8] = br#"{"expected_tree_revision":"3","title":"Chapter B"}"#;
const MISSING_CHAPTER: &str = "018f0000-0000-7001-8000-00000000ffff";

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
            canonical_command_digest: VOLUME_DIGEST.to_owned(),
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
        canonical_command_bytes: VOLUME_BYTES.to_vec(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{ids_suffix}"),
        title: VOLUME_TITLE.to_owned(),
        expected_tree_revision: 1,
        ids: AuthorCommandAdmissionIds {
            command_id: format!("018f0000-0000-7001-8000-00000001{ids_suffix}"),
            author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{ids_suffix}"),
            receipt_id: format!("018f0000-0000-7001-8000-00000003{ids_suffix}"),
        },
    }
}

fn chapter_issue(
    scope: &ProjectScope,
    idempotency_suffix: &str,
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
            method: "POST".to_owned(),
            route_template: "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters".to_owned(),
            command_schema: "storyos.command.create-chapter.request.v1".to_owned(),
            command_kind: "createChapter".to_owned(),
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

fn delete_issue(
    scope: &ProjectScope,
    idempotency_suffix: &str,
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
            method: "DELETE".to_owned(),
            route_template: "/api/v1/projects/{project_id}/chapters/{chapter_id}".to_owned(),
            command_schema: "storyos.command.delete-chapter.request.v1".to_owned(),
            command_kind: "deleteChapter".to_owned(),
            canonical_command_digest: digest.to_owned(),
            idempotency_key: format!("018f0000-0000-7001-8000-00000000{idempotency_suffix}"),
        },
        nonce: format!("opaque-nonce-{idempotency_suffix}"),
        nonce_digest: format!("sha256:nonce-{idempotency_suffix}"),
    }
}

fn delete_command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
    chapter_id: &str,
    expected_tree_revision: u64,
    bytes: &[u8],
) -> DeleteChapterCommand {
    DeleteChapterCommand {
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
        chapter_id: ChapterId::new(chapter_id),
        expected_tree_revision,
        ids: AuthorCommandAdmissionIds {
            command_id: format!("018f0000-0000-7001-8000-00000001{ids_suffix}"),
            author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{ids_suffix}"),
            receipt_id: format!("018f0000-0000-7001-8000-00000003{ids_suffix}"),
        },
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

async fn seed_project(store: &PostgresProjectReader, suffix: &str) -> ProjectScope {
    let issue = create_project_issue(&format!("018f0000-0000-7001-8000-00000000{suffix}"), suffix);
    let issued = issue_create_project_challenge(store, &issue).await.unwrap();
    let mut binding = issue.binding.clone();
    binding.prospective_project_id = issued.prospective_project_id.clone();
    binding.canonical_command_digest = issued.canonical_command_digest.clone();
    create_project(
        store,
        &create_project_command(binding.clone(), &issue.nonce_digest, suffix),
    )
    .await
    .unwrap();
    ProjectScope::new(binding.owner_user_id, binding.prospective_project_id)
}

async fn apply_volume(store: &PostgresProjectReader, scope: &ProjectScope, suffix: &str) -> String {
    let issue = volume_issue(scope, suffix);
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    let settlement = create_volume(
        store,
        &volume_command(issue.binding, &issue.nonce_digest, suffix),
    )
    .await
    .unwrap();
    let CreateVolumeSettlementEffect::Applied { volume_id, .. } = settlement.effect else {
        panic!("Volume A must apply");
    };
    volume_id
}

async fn apply_chapter(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    volume_id: &str,
    title: &str,
    bytes: &[u8],
    expected_tree_revision: u64,
) -> String {
    let digest = format!(
        "sha256:storyos.command.createChapter.jcs.v1:{}",
        crate::author_edit::sha256_hex(bytes)
    );
    let issue = chapter_issue(scope, suffix, &digest);
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    let settlement = create_chapter(
        store,
        &chapter_command(
            issue.binding,
            &issue.nonce_digest,
            suffix,
            volume_id,
            title,
            expected_tree_revision,
            bytes,
        ),
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
    chapter_id: &str,
    expected_tree_revision: u64,
    bytes: &[u8],
) -> storyos_application::DeleteChapterSettlement {
    let digest = format!(
        "sha256:storyos.command.deleteChapter.jcs.v1:{}",
        crate::author_edit::sha256_hex(bytes)
    );
    let issue = delete_issue(scope, suffix, &digest);
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    delete_chapter(
        store,
        &delete_command(
            issue.binding,
            &issue.nonce_digest,
            suffix,
            chapter_id,
            expected_tree_revision,
            bytes,
        ),
    )
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn delete_chapter_is_atomic_replayable_and_scope_safe() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let scope = seed_project(&store, "e410").await;
    let volume_id = apply_volume(&store, &scope, "e412").await;
    let chapter_a = apply_chapter(
        &store,
        &scope,
        "e414",
        &volume_id,
        "Chapter A",
        CHAPTER_A_BYTES,
        /*expected_tree_revision*/ 2,
    )
    .await;
    let chapter_b = apply_chapter(
        &store,
        &scope,
        "e416",
        &volume_id,
        "Chapter B",
        CHAPTER_B_BYTES,
        /*expected_tree_revision*/ 3,
    )
    .await;

    let delete_bytes = br#"{"expected_tree_revision":"4"}"#;
    let delete_digest = format!(
        "sha256:storyos.command.deleteChapter.jcs.v1:{}",
        crate::author_edit::sha256_hex(delete_bytes)
    );
    let first_issue = delete_issue(&scope, "e418", &delete_digest);
    issue_project_command_challenge(&store, &first_issue)
        .await
        .unwrap();
    let first = delete_chapter(
        &store,
        &delete_command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "e418",
            &chapter_b,
            /*expected_tree_revision*/ 4,
            delete_bytes,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        first.effect,
        DeleteChapterSettlementEffect::Applied {
            tree_revision: 5,
            volume_id: volume_id.clone(),
            current: storyos_core::DeleteChapterCurrent::PreserveExisting,
        }
    );
    let authority = first
        .authority
        .clone()
        .expect("Applied Delete Chapter must write Structural Authority Settlement");
    assert_eq!(authority.prior_manuscript_tree_revision, 4);
    assert_eq!(authority.resulting_manuscript_tree_revision, 5);
    assert_eq!(authority.author_action_sequence, 4);
    let replay = delete_chapter(
        &store,
        &delete_command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "e499",
            &chapter_b,
            /*expected_tree_revision*/ 4,
            delete_bytes,
        ),
    )
    .await
    .unwrap();
    assert_eq!(replay, first);

    let tree = get_manuscript_tree(&store, &scope)
        .await
        .unwrap()
        .expect("the Project still has a Canonical Query");
    assert_eq!(tree.tree_revision, 5);
    assert_eq!(tree.snapshot.snapshot_id, authority.snapshot_id);
    assert_eq!(
        tree.snapshot.project_activity_position,
        first.project_activity_position
    );
    assert_eq!(
        tree.volumes,
        vec![VolumeNode {
            volume_id: VolumeId::new(volume_id.clone()),
            title: VOLUME_TITLE.to_owned(),
            order: 1,
            chapters: vec![ChapterNode {
                chapter_id: ChapterId::new(chapter_a.clone()),
                title: "Chapter A".to_owned(),
                order: 1,
            }],
        }]
    );
    assert_eq!(
        open_project(&store, &scope)
            .await
            .unwrap()
            .expect("the Project remains in exact Scope")
            .current_chapter_id,
        Some(ChapterId::new(chapter_a.clone()))
    );
    assert_eq!(
        open_chapter(&store, &scope, &ChapterId::new(chapter_b.clone()))
            .await
            .unwrap(),
        OpenChapter::Missing
    );
    assert_eq!(
        get_manuscript_tree(
            &store,
            &ProjectScope::new(UserId::new(USER_B), scope.project_id.clone()),
        )
        .await
        .unwrap(),
        None
    );

    let already = apply_delete(
        &store,
        &scope,
        "e41a",
        &chapter_b,
        /*expected_tree_revision*/ 5,
        br#"{"expected_tree_revision":"5"}"#,
    )
    .await;
    assert_eq!(
        already.effect,
        DeleteChapterSettlementEffect::NoEffect {
            reason: storyos_core::DeleteChapterNoEffect::AlreadyRemoved,
        }
    );
    assert_eq!(already.authority, None);

    let stale = apply_delete(
        &store,
        &scope,
        "e41c",
        &chapter_a,
        /*expected_tree_revision*/ 4,
        delete_bytes,
    )
    .await;
    assert_eq!(
        stale.effect,
        DeleteChapterSettlementEffect::Conflicted {
            reason: storyos_core::DeleteChapterConflict::StaleTreeRevision,
        }
    );
    assert_eq!(stale.authority, None);

    let invalid = apply_delete(
        &store,
        &scope,
        "e41e",
        MISSING_CHAPTER,
        /*expected_tree_revision*/ 5,
        br#"{"expected_tree_revision":"5"}"#,
    )
    .await;
    assert_eq!(
        invalid.effect,
        DeleteChapterSettlementEffect::Refused {
            reason: storyos_core::DeleteChapterRefusal::InvalidChapterJoin,
        }
    );
    assert_eq!(invalid.authority, None);

    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    let row = admin
        .query_one(
            "SELECT tree_revision::text,
                    (SELECT count(*) FROM storyos.manuscript_objects
                      WHERE project_id = $1::text::uuid AND object_kind = 'chapter'),
                    (SELECT count(*) FROM storyos.chapter_removal_decisions
                      WHERE project_id = $1::text::uuid AND chapter_id = $2::text::uuid),
                    (SELECT count(*) FROM storyos.domain_receipts
                      WHERE project_id = $1::text::uuid AND command_kind = 'deleteChapter'),
                    (SELECT count(*) FROM storyos.project_activity_event_payloads
                      WHERE project_id = $1::text::uuid AND event_kind = 'chapter_deleted'),
                    (SELECT count(*) FROM storyos.authoritative_commits
                      WHERE project_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.author_action_entries
                      WHERE project_id = $1::text::uuid AND disposition = 'forward'),
                    (SELECT manuscript_object_id IS NULL
                              AND prior_revision_id IS NULL
                              AND resulting_revision_id IS NULL
                              AND affected_chapter_id = $2::text::uuid
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
                &chapter_b,
                &authority.authoritative_commit_id,
                &vec![
                    already.ids.receipt_id.clone(),
                    stale.ids.receipt_id.clone(),
                    invalid.ids.receipt_id.clone(),
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
            row.get::<_, i64>(3),
            row.get::<_, i64>(4),
            row.get::<_, i64>(5),
            row.get::<_, i64>(6),
            row.get::<_, bool>(7),
            row.get::<_, i64>(8)
        ),
        ("5".to_owned(), 2, 1, 4, 1, 4, 4, true, 0)
    );

    admin
        .execute(
            "UPDATE storyos.projects SET lifecycle_state = 'archived'
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&USER_A, &scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let archived = apply_delete(
        &store,
        &scope,
        "e420",
        &chapter_a,
        /*expected_tree_revision*/ 5,
        br#"{"expected_tree_revision":"5"}"#,
    )
    .await;
    assert_eq!(
        archived.effect,
        DeleteChapterSettlementEffect::Refused {
            reason: storyos_core::DeleteChapterRefusal::ArchivedProject,
        }
    );
    assert_eq!(archived.authority, None);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_compensates_delete_chapter_and_restores_prior_chapter_identity() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let scope = seed_project(&store, "e430").await;
    let volume_id = apply_volume(&store, &scope, "e432").await;
    let chapter_a = apply_chapter(
        &store,
        &scope,
        "e434",
        &volume_id,
        "Chapter A",
        CHAPTER_A_BYTES,
        /*expected_tree_revision*/ 2,
    )
    .await;
    let chapter_b = apply_chapter(
        &store,
        &scope,
        "e436",
        &volume_id,
        "Chapter B",
        CHAPTER_B_BYTES,
        /*expected_tree_revision*/ 3,
    )
    .await;
    let deleted = apply_delete(
        &store,
        &scope,
        "e438",
        &chapter_a,
        /*expected_tree_revision*/ 4,
        br#"{"expected_tree_revision":"4"}"#,
    )
    .await;
    let DeleteChapterSettlementEffect::Applied { current, .. } = deleted.effect.clone() else {
        panic!("Delete Chapter A must apply");
    };
    assert_eq!(
        current,
        storyos_core::DeleteChapterCurrent::SelectSuccessor {
            chapter_id: chapter_b.clone(),
        }
    );
    let authority = deleted
        .authority
        .clone()
        .expect("Applied Delete Chapter must write authority");
    let session_issue = named_issue(
        &scope,
        "e43a",
        "POST",
        "/api/v1/projects/{project_id}/editor-sessions",
        "storyos.command.create-editor-session.request.v1",
        "createEditorSession",
        "sha256:storyos.test:e43a",
    );
    issue_project_command_challenge(&store, &session_issue)
        .await
        .unwrap();
    let editor_session_id = "018f0000-0000-7001-8000-00000000e43b";
    create_editor_session(
        &store,
        &OpenEditorSession {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(editor_session_id),
            snapshot_id: "018f0000-0000-7001-8000-00000000e43c".to_owned(),
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
        open_chapter(&store, &scope, &ChapterId::new(chapter_b.clone()))
            .await
            .unwrap()
    else {
        panic!("successor Chapter B must open");
    };
    let undo_input = serde_json::json!({
        "expected_author_undo_frontier_sequence": authority.author_action_sequence.to_string(),
        "expected_authoritative_revision_id": opened.chapter.revision_id.as_ref(),
        "editor_session_id": editor_session_id,
        "client_contract_revision": CLIENT,
        "security_policy_revision": SECURITY,
        "correlation_id": "018f0000-0000-7001-8000-00000000e43d",
    });
    let undo_body = serde_json::json!({
        "command_schema": "storyos.command.undo-latest-author-action.request.v1",
        "undo_latest_author_action_input": undo_input,
    });
    let undo_bytes = serde_json::to_vec(&undo_body).unwrap();
    let undo_digest = format!(
        "sha256:storyos.command.undoLatestAuthorAction.jcs.v1:{}",
        crate::author_edit::sha256_hex(&undo_bytes)
    );
    let undo_issue = named_issue(
        &scope,
        "e43d",
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
            client_binding: EditorClientBinding {
                binding_ref: undo_issue.binding.client_session_binding_digest.clone(),
                session_generation: undo_issue.binding.client_session_generation,
                client_contract_revision: undo_issue.binding.client_contract_revision.clone(),
                security_policy_revision: undo_issue.binding.security_policy_revision.clone(),
            },
            challenge_binding: undo_issue.binding,
            nonce_digest: undo_issue.nonce_digest,
            canonical_command_bytes: undo_bytes,
            correlation_id: "018f0000-0000-7001-8000-00000000e43d".to_owned(),
            ids: AuthorCommandAdmissionIds {
                command_id: "018f0000-0000-7001-8000-00000001e43d".to_owned(),
                author_command_admission_id: "018f0000-0000-7001-8000-00000002e43d".to_owned(),
                receipt_id: "018f0000-0000-7001-8000-00000003e43d".to_owned(),
            },
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
        panic!("Delete Chapter Undo must write structure Compensation");
    };
    assert_eq!(source_sequence, authority.author_action_sequence);
    let tree = get_manuscript_tree(&store, &scope)
        .await
        .unwrap()
        .expect("the tree remains after Delete Chapter Compensation");
    assert_eq!(tree.tree_revision, 4);
    assert_eq!(tree.snapshot.snapshot_id, snapshot_id);
    assert_eq!(
        tree.volumes,
        vec![VolumeNode {
            volume_id: VolumeId::new(volume_id),
            title: VOLUME_TITLE.to_owned(),
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
    assert_eq!(
        open_project(&store, &scope)
            .await
            .unwrap()
            .expect("the Project remains in exact Scope")
            .current_chapter_id,
        Some(ChapterId::new(chapter_a.clone()))
    );
    assert!(matches!(
        open_chapter(&store, &scope, &ChapterId::new(chapter_a))
            .await
            .unwrap(),
        OpenChapter::Found(_)
    ));

    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    let observed: serde_json::Value = serde_json::from_str(
        &admin
            .query_one(
                "SELECT jsonb_build_object(
                          'delete_chapter_receipts', (
                            SELECT count(*) FROM storyos.domain_receipts
                             WHERE project_id = $1::text::uuid
                               AND command_kind = 'deleteChapter'
                          ),
                          'create_chapter_receipts_after_undo', (
                            SELECT count(*) FROM storyos.domain_receipts
                             WHERE project_id = $1::text::uuid
                               AND command_kind = 'createChapter'
                               AND receipt_id = $2::text::uuid
                          ),
                          'compensation_actions', (
                            SELECT count(*) FROM storyos.author_action_entries
                             WHERE project_id = $1::text::uuid
                               AND disposition = 'compensation'
                               AND compensated_source_sequence = $3::text::numeric
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
                          'live_removal_decisions', (
                            SELECT count(*) FROM storyos.chapter_removal_decisions
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
            "delete_chapter_receipts": 1,
            "create_chapter_receipts_after_undo": 0,
            "compensation_actions": 1,
            "forward_delete_actions": 1,
            "live_removal_decisions": 0,
        })
    );
}
