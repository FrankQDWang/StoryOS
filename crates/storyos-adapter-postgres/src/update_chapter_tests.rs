use super::*;
use storyos_application::{
    AuthorCommandAdmissionIds, ChapterId, ChapterNode, CreateChapterCommand,
    CreateChapterSettlementEffect, CreateProjectChallengeBinding, CreateProjectCommand,
    CreateVolumeCommand, CreateVolumeSettlementEffect, EditorClientBinding,
    IssueCreateProjectChallenge, IssueProjectCommandChallenge, ProjectCommandChallengeBinding,
    ProjectId, ProjectScope, UpdateChapterCommand, UpdateChapterSettlementEffect, UserId, VolumeId,
    VolumeNode, create_chapter, create_project, create_volume, get_manuscript_tree,
    issue_create_project_challenge, issue_project_command_challenge, open_current_chapter,
    open_project, update_chapter,
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
const UPDATE_BYTES: &[u8] = br#"{"expected_chapter_revision":"4","order":"2","title":"Chapter B"}"#;
const UPDATE_DIGEST: &str = "sha256:storyos.command.updateChapter.jcs.v1:7e5935287ca887f4a10196ad538dab2b920f5c9f7667be61b7137c5016704e97";
const INVALID_ORDER_BYTES: &[u8] =
    br#"{"expected_chapter_revision":"4","order":"3","title":"Chapter B"}"#;
const INVALID_ORDER_DIGEST: &str = "sha256:storyos.command.updateChapter.jcs.v1:1a149134a94f3218f8a11e0e98c2fed2009d802f7cf5e234938b45a675e65496";
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

fn update_issue(
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
            method: "PATCH".to_owned(),
            route_template: "/api/v1/projects/{project_id}/chapters/{chapter_id}".to_owned(),
            command_schema: "storyos.command.update-chapter.request.v1".to_owned(),
            command_kind: "updateChapter".to_owned(),
            canonical_command_digest: digest.to_owned(),
            idempotency_key: format!("018f0000-0000-7001-8000-00000000{idempotency_suffix}"),
        },
        nonce: format!("opaque-nonce-{idempotency_suffix}"),
        nonce_digest: format!("sha256:nonce-{idempotency_suffix}"),
    }
}

struct UpdateFixture<'a> {
    chapter_id: &'a str,
    title: &'a str,
    order: u64,
    expected_chapter_revision: u64,
    bytes: &'a [u8],
}

fn update_command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
    fixture: UpdateFixture<'_>,
) -> UpdateChapterCommand {
    UpdateChapterCommand {
        project_scope: binding.project_scope.clone(),
        client_binding: EditorClientBinding {
            binding_ref: binding.client_session_binding_digest.clone(),
            session_generation: binding.client_session_generation,
            client_contract_revision: binding.client_contract_revision.clone(),
            security_policy_revision: binding.security_policy_revision.clone(),
        },
        challenge_binding: binding,
        nonce_digest: nonce_digest.to_owned(),
        canonical_command_bytes: fixture.bytes.to_vec(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{ids_suffix}"),
        chapter_id: ChapterId::new(fixture.chapter_id),
        title: fixture.title.to_owned(),
        order: fixture.order,
        expected_chapter_revision: fixture.expected_chapter_revision,
        ids: AuthorCommandAdmissionIds {
            command_id: format!("018f0000-0000-7001-8000-00000001{ids_suffix}"),
            author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{ids_suffix}"),
            receipt_id: format!("018f0000-0000-7001-8000-00000003{ids_suffix}"),
        },
    }
}

fn applied_fixture(chapter_id: &str) -> UpdateFixture<'_> {
    UpdateFixture {
        chapter_id,
        title: "Chapter B",
        order: 2,
        expected_chapter_revision: 4,
        bytes: UPDATE_BYTES,
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn update_chapter_is_atomic_replayable_and_scope_safe() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let issue = create_project_issue("018f0000-0000-7001-8000-000000000d10", "0d10");
    let issued = issue_create_project_challenge(&store, &issue)
        .await
        .unwrap();
    let mut project_binding = issue.binding.clone();
    project_binding.prospective_project_id = issued.prospective_project_id.clone();
    project_binding.canonical_command_digest = issued.canonical_command_digest.clone();
    create_project(
        &store,
        &create_project_command(project_binding.clone(), &issue.nonce_digest, "0d11"),
    )
    .await
    .unwrap();
    let scope = ProjectScope::new(
        project_binding.owner_user_id.clone(),
        project_binding.prospective_project_id.clone(),
    );

    let volume_issue = volume_issue(&scope, "0d12");
    issue_project_command_challenge(&store, &volume_issue)
        .await
        .unwrap();
    let volume = create_volume(
        &store,
        &volume_command(
            volume_issue.binding.clone(),
            &volume_issue.nonce_digest,
            "0d13",
        ),
    )
    .await
    .unwrap();
    let CreateVolumeSettlementEffect::Applied { volume_id, .. } = volume.effect else {
        panic!("Create Volume on an empty active Project must apply");
    };

    let first_chapter_issue = chapter_issue(&scope, "0d14", CHAPTER_A_DIGEST);
    issue_project_command_challenge(&store, &first_chapter_issue)
        .await
        .unwrap();
    let first_chapter = create_chapter(
        &store,
        &chapter_command(
            first_chapter_issue.binding.clone(),
            &first_chapter_issue.nonce_digest,
            "0d15",
            &volume_id,
            "Chapter A",
            2,
            CHAPTER_A_BYTES,
        ),
    )
    .await
    .unwrap();
    let CreateChapterSettlementEffect::Applied {
        chapter_id: first_chapter_id,
        ..
    } = first_chapter.effect
    else {
        panic!("the first Chapter must apply");
    };
    let second_chapter_issue = chapter_issue(&scope, "0d16", CHAPTER_B_DIGEST);
    issue_project_command_challenge(&store, &second_chapter_issue)
        .await
        .unwrap();
    let second_chapter = create_chapter(
        &store,
        &chapter_command(
            second_chapter_issue.binding.clone(),
            &second_chapter_issue.nonce_digest,
            "0d17",
            &volume_id,
            "Chapter B",
            3,
            CHAPTER_B_BYTES,
        ),
    )
    .await
    .unwrap();
    let CreateChapterSettlementEffect::Applied {
        chapter_id: second_chapter_id,
        ..
    } = second_chapter.effect
    else {
        panic!("the second Chapter must apply");
    };

    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    let identity_before = admin
        .query_one(
            "SELECT current_chapter_id::text,
                    (SELECT string_agg(
                       manuscript_object_id::text || ':' || current_revision_id::text, ','
                       ORDER BY manuscript_object_id)
                      FROM storyos.authoritative_heads
                     WHERE project_id = $1::text::uuid)
               FROM storyos.projects
              WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let current_before = identity_before.get::<_, String>(0);
    let heads_before = identity_before.get::<_, Option<String>>(1);

    let first_issue = update_issue(&scope, "0d18", UPDATE_DIGEST);
    issue_project_command_challenge(&store, &first_issue)
        .await
        .unwrap();
    let first = update_chapter(
        &store,
        &update_command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "0d19",
            applied_fixture(&first_chapter_id),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        first.effect,
        UpdateChapterSettlementEffect::Applied {
            title: "Chapter B".to_owned(),
            order: 2,
            tree_revision: 5,
        }
    );
    let replay = update_chapter(
        &store,
        &update_command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "0d99",
            applied_fixture(&first_chapter_id),
        ),
    )
    .await
    .unwrap();
    assert_eq!(replay, first);

    let identity_after = admin
        .query_one(
            "SELECT current_chapter_id::text,
                    (SELECT string_agg(
                       manuscript_object_id::text || ':' || current_revision_id::text, ','
                       ORDER BY manuscript_object_id)
                      FROM storyos.authoritative_heads
                     WHERE project_id = $1::text::uuid)
               FROM storyos.projects
              WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    assert_eq!(identity_after.get::<_, String>(0), current_before);
    assert_eq!(identity_after.get::<_, Option<String>>(1), heads_before);
    assert_eq!(current_before, first_chapter_id);
    let opened = open_current_chapter(&store, &scope, &ChapterId::new(first_chapter_id.clone()))
        .await
        .unwrap()
        .expect("the current Chapter identity is unchanged");
    assert_eq!(opened.chapter_id, ChapterId::new(first_chapter_id.clone()));
    assert_eq!(opened.title, "Chapter B");
    assert_eq!(opened.body, "");
    assert_eq!(
        open_project(&store, &scope)
            .await
            .unwrap()
            .expect("the Project remains in exact Scope")
            .current_chapter_id,
        Some(ChapterId::new(first_chapter_id.clone()))
    );

    let tree = get_manuscript_tree(&store, &scope)
        .await
        .unwrap()
        .expect("the Project still has a Canonical Query");
    assert_eq!(tree.tree_revision, 5);
    assert_eq!(
        tree.volumes,
        vec![VolumeNode {
            volume_id: VolumeId::new(volume_id),
            title: VOLUME_TITLE.to_owned(),
            order: 1,
            chapters: vec![
                ChapterNode {
                    chapter_id: ChapterId::new(second_chapter_id),
                    title: "Chapter B".to_owned(),
                    order: 1,
                },
                ChapterNode {
                    chapter_id: ChapterId::new(first_chapter_id.clone()),
                    title: "Chapter B".to_owned(),
                    order: 2,
                },
            ],
        }]
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

    let stale_issue = update_issue(&scope, "0d1a", UPDATE_DIGEST);
    issue_project_command_challenge(&store, &stale_issue)
        .await
        .unwrap();
    let stale = update_chapter(
        &store,
        &update_command(
            stale_issue.binding.clone(),
            &stale_issue.nonce_digest,
            "0d1b",
            applied_fixture(&first_chapter_id),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        stale.effect,
        UpdateChapterSettlementEffect::Conflicted {
            reason: storyos_core::UpdateChapterConflict::StaleChapterRevision,
        }
    );

    let missing_issue = update_issue(&scope, "0d1c", UPDATE_DIGEST);
    issue_project_command_challenge(&store, &missing_issue)
        .await
        .unwrap();
    let missing = update_chapter(
        &store,
        &update_command(
            missing_issue.binding.clone(),
            &missing_issue.nonce_digest,
            "0d1d",
            applied_fixture(MISSING_CHAPTER),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        missing.effect,
        UpdateChapterSettlementEffect::Refused {
            reason: storyos_core::UpdateChapterRefusal::InvalidChapterJoin,
        }
    );

    let invalid_issue = update_issue(&scope, "0d1e", INVALID_ORDER_DIGEST);
    issue_project_command_challenge(&store, &invalid_issue)
        .await
        .unwrap();
    let invalid = update_chapter(
        &store,
        &update_command(
            invalid_issue.binding.clone(),
            &invalid_issue.nonce_digest,
            "0d1f",
            UpdateFixture {
                chapter_id: &first_chapter_id,
                title: "Chapter B",
                order: 3,
                expected_chapter_revision: 4,
                bytes: INVALID_ORDER_BYTES,
            },
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        invalid.effect,
        UpdateChapterSettlementEffect::Refused {
            reason: storyos_core::UpdateChapterRefusal::InvalidOrder,
        }
    );

    let row = admin
        .query_one(
            "SELECT tree_revision::text,
                    (SELECT count(*) FROM storyos.manuscript_objects
                      WHERE project_id = $1::text::uuid AND object_kind = 'chapter'),
                    (SELECT count(*) FROM storyos.domain_receipts
                      WHERE project_id = $1::text::uuid AND command_kind = 'updateChapter'),
                    (SELECT count(*) FROM storyos.project_activity_event_payloads
                      WHERE project_id = $1::text::uuid AND event_kind = 'chapter_updated')
               FROM storyos.projects
              WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            row.get::<_, String>(0),
            row.get::<_, i64>(1),
            row.get::<_, i64>(2),
            row.get::<_, i64>(3)
        ),
        ("5".to_owned(), 2, 4, 1)
    );

    admin
        .execute(
            "UPDATE storyos.projects SET lifecycle_state = 'archived'
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&USER_A, &scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let archived_issue = update_issue(&scope, "0d20", UPDATE_DIGEST);
    issue_project_command_challenge(&store, &archived_issue)
        .await
        .unwrap();
    let archived = update_chapter(
        &store,
        &update_command(
            archived_issue.binding.clone(),
            archived_issue.nonce_digest.as_str(),
            "0d21",
            applied_fixture(&first_chapter_id),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        archived.effect,
        UpdateChapterSettlementEffect::Refused {
            reason: storyos_core::UpdateChapterRefusal::ArchivedProject,
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
    assert_eq!(chapters_after_refuse, 2);
}
