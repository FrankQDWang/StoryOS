use super::*;
use storyos_application::{
    AuthorCommandAdmissionIds, ChapterId, ChapterNode, CreateChapterCommand,
    CreateChapterSettlementEffect, CreateProjectChallengeBinding, CreateProjectCommand,
    CreateVolumeCommand, EditorClientBinding, IssueCreateProjectChallenge,
    IssueProjectCommandChallenge, OpenChapter, ProjectCommandChallengeBinding, ProjectId,
    ProjectScope, UserId, VolumeId, VolumeNode, create_chapter, create_project, create_volume,
    get_manuscript_tree, issue_create_project_challenge, issue_project_command_challenge,
    open_chapter, open_current_chapter, open_project,
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
const CHAPTER_A_DIGEST: &str = "sha256:storyos.command.createChapter.jcs.v1:fdee6f3020b94d08f6e75dfab8be7caf86d1a8c84c521254b03e6a45153a1de2";
const CHAPTER_B_BYTES: &[u8] = br#"{"expected_tree_revision":"3","title":"Chapter B"}"#;
const CHAPTER_B_DIGEST: &str = "sha256:storyos.command.createChapter.jcs.v1:23a99998cce1f35b113794d41235dae8fae7d8ae9f234ba67161a2f9e8e86e65";
const CHAPTER_C_BYTES: &[u8] = br#"{"expected_tree_revision":"4","title":"Chapter C"}"#;
const CHAPTER_C_DIGEST: &str = "sha256:storyos.command.createChapter.jcs.v1:675e40d4c1b160d396410a4fd620c5352547ce2213dfb9c758e213c0cd7ed52a";
const MISSING_VOLUME: &str = "018f0000-0000-7001-8000-00000000ffff";

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

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn create_chapter_is_atomic_replayable_and_scope_safe() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let issue = create_project_issue("018f0000-0000-7001-8000-000000000910", "0910");
    let issued = issue_create_project_challenge(&store, &issue)
        .await
        .unwrap();
    let mut project_binding = issue.binding.clone();
    project_binding.prospective_project_id = issued.prospective_project_id.clone();
    project_binding.canonical_command_digest = issued.canonical_command_digest.clone();
    create_project(
        &store,
        &create_project_command(project_binding.clone(), &issue.nonce_digest, "0911"),
    )
    .await
    .unwrap();
    let scope = ProjectScope::new(
        project_binding.owner_user_id.clone(),
        project_binding.prospective_project_id.clone(),
    );

    let volume_issue = volume_issue(&scope, "0912");
    issue_project_command_challenge(&store, &volume_issue)
        .await
        .unwrap();
    let volume = create_volume(
        &store,
        &volume_command(
            volume_issue.binding.clone(),
            &volume_issue.nonce_digest,
            "0913",
        ),
    )
    .await
    .unwrap();
    let storyos_application::CreateVolumeSettlementEffect::Applied { volume_id, .. } =
        volume.effect
    else {
        panic!("Create Volume on an empty active Project must apply");
    };

    let first_issue = chapter_issue(&scope, "0914", CHAPTER_A_DIGEST);
    issue_project_command_challenge(&store, &first_issue)
        .await
        .unwrap();
    let first = create_chapter(
        &store,
        &chapter_command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "0915",
            &volume_id,
            "Chapter A",
            2,
            CHAPTER_A_BYTES,
        ),
    )
    .await
    .unwrap();
    let CreateChapterSettlementEffect::Applied {
        tree_revision,
        chapter_id,
        current,
        order,
    } = first.effect.clone()
    else {
        panic!("the first Chapter on an empty active Project must apply");
    };
    assert_eq!(tree_revision, 3);
    assert_eq!(current, storyos_core::CreateChapterCurrent::SelectCreated);
    assert_eq!(order, 1);
    let replay = create_chapter(
        &store,
        &chapter_command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "0999",
            &volume_id,
            "Chapter A",
            2,
            CHAPTER_A_BYTES,
        ),
    )
    .await
    .unwrap();
    assert_eq!(replay, first);

    let opened = open_current_chapter(&store, &scope, &ChapterId::new(chapter_id.clone()))
        .await
        .unwrap()
        .expect("the first Chapter is the current Chapter");
    assert_eq!(opened.chapter_id, ChapterId::new(chapter_id.clone()));
    assert_eq!(opened.title, "Chapter A");
    assert_eq!(opened.body, "");
    assert_eq!(
        open_project(&store, &scope)
            .await
            .unwrap()
            .expect("the Project remains in exact Scope")
            .current_chapter_id,
        Some(ChapterId::new(chapter_id.clone()))
    );

    let second_issue = chapter_issue(&scope, "0916", CHAPTER_B_DIGEST);
    issue_project_command_challenge(&store, &second_issue)
        .await
        .unwrap();
    let second = create_chapter(
        &store,
        &chapter_command(
            second_issue.binding.clone(),
            &second_issue.nonce_digest,
            "0917",
            &volume_id,
            "Chapter B",
            3,
            CHAPTER_B_BYTES,
        ),
    )
    .await
    .unwrap();
    let CreateChapterSettlementEffect::Applied {
        tree_revision: second_revision,
        chapter_id: chapter_b,
        current: second_current,
        order: second_order,
    } = second.effect.clone()
    else {
        panic!("a later Chapter on the same Volume must apply");
    };
    assert_eq!(second_revision, 4);
    assert_eq!(
        second_current,
        storyos_core::CreateChapterCurrent::PreserveExisting
    );
    assert_eq!(second_order, 2);
    assert_eq!(
        create_chapter(
            &store,
            &chapter_command(
                second_issue.binding.clone(),
                &second_issue.nonce_digest,
                "0998",
                &volume_id,
                "Chapter B",
                3,
                CHAPTER_B_BYTES,
            ),
        )
        .await
        .unwrap(),
        second
    );

    let third_issue = chapter_issue(&scope, "0918", CHAPTER_C_DIGEST);
    issue_project_command_challenge(&store, &third_issue)
        .await
        .unwrap();
    let third = create_chapter(
        &store,
        &chapter_command(
            third_issue.binding.clone(),
            &third_issue.nonce_digest,
            "0919",
            &volume_id,
            "Chapter C",
            4,
            CHAPTER_C_BYTES,
        ),
    )
    .await
    .unwrap();
    let CreateChapterSettlementEffect::Applied {
        tree_revision: third_revision,
        chapter_id: chapter_c,
        current: third_current,
        order: third_order,
    } = third.effect.clone()
    else {
        panic!("the third Chapter on the same Volume must apply");
    };
    assert_eq!(third_revision, 5);
    assert_eq!(
        third_current,
        storyos_core::CreateChapterCurrent::PreserveExisting
    );
    assert_eq!(third_order, 3);

    let tree = get_manuscript_tree(&store, &scope)
        .await
        .unwrap()
        .expect("the Project still has a Canonical Query");
    assert_eq!(tree.project_scope, scope);
    assert_eq!(tree.tree_revision, 5);
    assert_eq!(
        tree.volumes,
        vec![VolumeNode {
            volume_id: VolumeId::new(volume_id.clone()),
            title: VOLUME_TITLE.to_owned(),
            order: 1,
            chapters: vec![
                ChapterNode {
                    chapter_id: ChapterId::new(chapter_id.clone()),
                    title: "Chapter A".to_owned(),
                    order: 1,
                },
                ChapterNode {
                    chapter_id: ChapterId::new(chapter_b.clone()),
                    title: "Chapter B".to_owned(),
                    order: 2,
                },
                ChapterNode {
                    chapter_id: ChapterId::new(chapter_c.clone()),
                    title: "Chapter C".to_owned(),
                    order: 3,
                },
            ],
        }]
    );
    assert_eq!(
        open_current_chapter(&store, &scope, &ChapterId::new(chapter_b.clone()))
            .await
            .unwrap(),
        None
    );
    let later = open_chapter(&store, &scope, &ChapterId::new(chapter_b.clone()))
        .await
        .unwrap();
    let OpenChapter::Found(later_facts) = later else {
        panic!("a later in-scope Chapter must open from its Snapshot and Head");
    };
    assert_eq!(later_facts.project_scope, scope);
    assert_eq!(
        later_facts.chapter.chapter_id,
        ChapterId::new(chapter_b.clone())
    );
    assert_eq!(later_facts.chapter.title, "Chapter B");
    assert_eq!(later_facts.chapter.body, "");
    assert_eq!(later_facts.chapter.blocks.len(), 1);
    assert_eq!(
        later_facts.chapter.blocks[0].block_kind,
        storyos_core::ManuscriptBlockKind::Paragraph
    );
    assert_eq!(later_facts.chapter.blocks[0].text, "");
    assert!(!later_facts.chapter.blocks[0].manuscript_block_id.is_empty());
    assert!(!later_facts.snapshot.snapshot_id.is_empty());
    assert_eq!(
        open_chapter(
            &store,
            &ProjectScope::new(UserId::new(USER_B), scope.project_id.clone()),
            &ChapterId::new(chapter_b.clone()),
        )
        .await
        .unwrap(),
        OpenChapter::Missing
    );
    assert_eq!(
        open_chapter(
            &store,
            &scope,
            &ChapterId::new("018f0000-0000-7001-8000-00000000ffff"),
        )
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
    assert_eq!(
        open_current_chapter(
            &store,
            &ProjectScope::new(UserId::new(USER_B), scope.project_id.clone()),
            &ChapterId::new(chapter_id.clone()),
        )
        .await
        .unwrap(),
        None
    );

    let stale_issue = chapter_issue(&scope, "0920", CHAPTER_A_DIGEST);
    issue_project_command_challenge(&store, &stale_issue)
        .await
        .unwrap();
    let stale = create_chapter(
        &store,
        &chapter_command(
            stale_issue.binding.clone(),
            &stale_issue.nonce_digest,
            "0921",
            &volume_id,
            "Chapter A",
            2,
            CHAPTER_A_BYTES,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        stale.effect,
        CreateChapterSettlementEffect::Conflicted {
            reason: storyos_core::CreateChapterConflict::StaleTreeRevision,
        }
    );

    let invalid_issue = chapter_issue(&scope, "0922", CHAPTER_A_DIGEST);
    issue_project_command_challenge(&store, &invalid_issue)
        .await
        .unwrap();
    let invalid = create_chapter(
        &store,
        &chapter_command(
            invalid_issue.binding.clone(),
            &invalid_issue.nonce_digest,
            "0923",
            MISSING_VOLUME,
            "Chapter A",
            2,
            CHAPTER_A_BYTES,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        invalid.effect,
        CreateChapterSettlementEffect::Refused {
            reason: storyos_core::CreateChapterRefusal::InvalidVolumeJoin,
        }
    );

    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    let row = admin
        .query_one(
            "SELECT tree_revision::text,
                    current_chapter_id::text,
                    (SELECT count(*) FROM storyos.manuscript_objects
                      WHERE project_id = $1::text::uuid AND object_kind = 'chapter'),
                    (SELECT count(*) FROM storyos.authoritative_heads
                      WHERE project_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.authoritative_payloads
                      WHERE project_id = $1::text::uuid AND octet_length(canonical_bytes) = 0),
                    (SELECT count(*) FROM storyos.domain_receipts
                      WHERE project_id = $1::text::uuid AND command_kind = 'createChapter'),
                    (SELECT count(*) FROM storyos.project_activity_event_payloads
                      WHERE project_id = $1::text::uuid AND event_kind = 'chapter_created')
               FROM storyos.projects
              WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            row.get::<_, String>(0),
            row.get::<_, String>(1),
            row.get::<_, i64>(2),
            row.get::<_, i64>(3),
            row.get::<_, i64>(4),
            row.get::<_, i64>(5),
            row.get::<_, i64>(6)
        ),
        ("5".to_owned(), chapter_id.clone(), 3, 3, 3, 5, 3)
    );

    admin
        .execute(
            "UPDATE storyos.projects SET lifecycle_state = 'archived'
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&USER_A, &scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let archived_issue = chapter_issue(&scope, "0924", CHAPTER_A_DIGEST);
    issue_project_command_challenge(&store, &archived_issue)
        .await
        .unwrap();
    let archived = create_chapter(
        &store,
        &chapter_command(
            archived_issue.binding.clone(),
            archived_issue.nonce_digest.as_str(),
            "0925",
            &volume_id,
            "Chapter A",
            2,
            CHAPTER_A_BYTES,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        archived.effect,
        CreateChapterSettlementEffect::Refused {
            reason: storyos_core::CreateChapterRefusal::ArchivedProject,
        }
    );
    let chapters_after_refuse = admin
        .query_one(
            "SELECT count(*) FROM storyos.manuscript_objects
              WHERE project_id = $1::text::uuid AND object_kind = 'chapter'",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(chapters_after_refuse, 3);

    admin
        .execute(
            "UPDATE storyos.project_snapshots
                SET expires_at = clock_timestamp() - interval '1 second'
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&USER_A, &scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    assert_eq!(
        open_chapter(&store, &scope, &ChapterId::new(chapter_b.clone()))
            .await
            .unwrap(),
        OpenChapter::SnapshotExpired
    );
}
