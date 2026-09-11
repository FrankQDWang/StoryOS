use super::*;
use storyos_application::{
    AuthorCommandAdmissionIds, ChapterId, ChapterNode, CreateChapterCommand,
    CreateChapterSettlementEffect, CreateProjectChallengeBinding, CreateProjectCommand,
    CreateVolumeCommand, CreateVolumeSettlementEffect, EditorClientBinding, EditorSessionId,
    GetManuscriptTree, IssueCreateProjectChallenge, IssueProjectCommandChallenge, OpenChapter,
    OpenEditorSession, ProjectCommandChallengeBinding, ProjectId, ProjectScope,
    UndoLatestAuthorActionCommand, UndoLatestAuthorActionSettlementEffect, UpdateChapterCommand,
    UpdateChapterSettlementEffect, UserId, VolumeId, VolumeNode, create_chapter,
    create_editor_session, create_project, create_volume, get_manuscript_tree,
    issue_create_project_challenge, issue_project_command_challenge, open_chapter,
    open_current_chapter, open_project, undo_latest_author_action, update_chapter,
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
const UPDATE_BYTES: &[u8] = br#"{"expected_tree_revision":"4","order":"2","title":"Chapter B"}"#;
const UPDATE_DIGEST: &str = "sha256:storyos.command.updateChapter.jcs.v1:90b4ef5e863b6db8d2e14f1d5b513b329605aced7d0cc7713b041398bcd766fd";
const INVALID_ORDER_BYTES: &[u8] =
    br#"{"expected_tree_revision":"4","order":"3","title":"Chapter B"}"#;
const INVALID_ORDER_DIGEST: &str = "sha256:storyos.command.updateChapter.jcs.v1:2088399d6b9bb10ca203eb34d71e4601c0d971814928af59fcbbb3c0fb2dee99";
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
    expected_tree_revision: u64,
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
        expected_tree_revision: fixture.expected_tree_revision,
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
        expected_tree_revision: 4,
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
    let authority = first
        .authority
        .clone()
        .expect("Applied Update Chapter must write Structural Authority Settlement");
    assert_eq!(authority.prior_manuscript_tree_revision, 4);
    assert_eq!(authority.resulting_manuscript_tree_revision, 5);
    assert_eq!(authority.author_action_sequence, 4);
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
        GetManuscriptTree::Missing
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
            reason: storyos_core::UpdateChapterConflict::StaleTreeRevision,
        }
    );
    assert_eq!(stale.authority, None);

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
    assert_eq!(missing.authority, None);

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
                expected_tree_revision: 4,
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
    assert_eq!(invalid.authority, None);

    let row = admin
        .query_one(
            "SELECT tree_revision::text,
                    (SELECT count(*) FROM storyos.manuscript_objects
                      WHERE project_id = $1::text::uuid AND object_kind = 'chapter'),
                    (SELECT count(*) FROM storyos.domain_receipts
                      WHERE project_id = $1::text::uuid AND command_kind = 'updateChapter'),
                    (SELECT count(*) FROM storyos.project_activity_event_payloads
                      WHERE project_id = $1::text::uuid AND event_kind = 'chapter_updated'),
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
                      WHERE receipt_id = $4::text::uuid),
                    (SELECT payload.payload->>'prior_title'
                       FROM storyos.project_activity_event_payloads AS payload
                      WHERE payload.project_id = $1::text::uuid
                        AND payload.event_kind = 'chapter_updated'),
                    (SELECT payload.payload->>'prior_order'
                       FROM storyos.project_activity_event_payloads AS payload
                      WHERE payload.project_id = $1::text::uuid
                        AND payload.event_kind = 'chapter_updated')
               FROM storyos.projects
              WHERE project_id = $1::text::uuid",
            &[
                &scope.project_id.as_ref(),
                &first_chapter_id,
                &authority.authoritative_commit_id,
                &stale.ids.receipt_id,
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
            row.get::<_, bool>(6),
            row.get::<_, i64>(7),
            row.get::<_, String>(8),
            row.get::<_, String>(9)
        ),
        (
            "5".to_owned(),
            2,
            4,
            1,
            4,
            4,
            true,
            0,
            "Chapter A".to_owned(),
            "1".to_owned()
        )
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
    assert_eq!(archived.authority, None);
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

async fn apply_update(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    fixture: UpdateFixture<'_>,
) -> storyos_application::UpdateChapterSettlement {
    let digest = format!(
        "sha256:storyos.command.updateChapter.jcs.v1:{}",
        crate::author_edit::sha256_hex(fixture.bytes)
    );
    let issue = update_issue(scope, suffix, &digest);
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    update_chapter(
        store,
        &update_command(issue.binding, &issue.nonce_digest, suffix, fixture),
    )
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_compensates_update_chapter_and_restores_title_and_canonical_sibling_order() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let scope = seed_project(&store, "1180").await;
    let volume_id = apply_volume(&store, &scope, "1182").await;
    let chapter_a = apply_chapter(
        &store,
        &scope,
        "1184",
        &volume_id,
        "Chapter A",
        CHAPTER_A_BYTES,
        /*expected_tree_revision*/ 2,
    )
    .await;
    let chapter_b = apply_chapter(
        &store,
        &scope,
        "1186",
        &volume_id,
        "Chapter B",
        CHAPTER_B_BYTES,
        /*expected_tree_revision*/ 3,
    )
    .await;
    // Sibling Update Chapter changes B's live order. A's last Activity still says order 1.
    let move_b = br#"{"expected_tree_revision":"4","order":"1","title":"Chapter B"}"#;
    apply_update(
        &store,
        &scope,
        "1188",
        UpdateFixture {
            chapter_id: &chapter_b,
            title: "Chapter B",
            order: 1,
            expected_tree_revision: 4,
            bytes: move_b,
        },
    )
    .await;
    let rename_a = br#"{"expected_tree_revision":"5","order":"2","title":"Renamed A"}"#;
    let updated = apply_update(
        &store,
        &scope,
        "118a",
        UpdateFixture {
            chapter_id: &chapter_a,
            title: "Renamed A",
            order: 2,
            expected_tree_revision: 5,
            bytes: rename_a,
        },
    )
    .await;
    let UpdateChapterSettlementEffect::Applied { tree_revision, .. } = updated.effect else {
        panic!("Rename A must apply");
    };
    assert_eq!(tree_revision, 6);
    let authority = updated
        .authority
        .clone()
        .expect("Applied Update Chapter must write authority");
    let session_issue = named_issue(
        &scope,
        "118c",
        "POST",
        "/api/v1/projects/{project_id}/editor-sessions",
        "storyos.command.create-editor-session.request.v1",
        "createEditorSession",
        "sha256:storyos.test:118c",
    );
    issue_project_command_challenge(&store, &session_issue)
        .await
        .unwrap();
    let editor_session_id = "018f0000-0000-7001-8000-00000000118d";
    create_editor_session(
        &store,
        &OpenEditorSession {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(editor_session_id),
            snapshot_id: "018f0000-0000-7001-8000-00000000118e".to_owned(),
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
        open_chapter(&store, &scope, &ChapterId::new(chapter_a.clone()))
            .await
            .unwrap()
    else {
        panic!("Chapter A must open");
    };
    let undo_input = serde_json::json!({
        "expected_author_undo_frontier_sequence": authority.author_action_sequence.to_string(),
        "expected_authoritative_revision_id": opened.chapter.revision_id.as_ref(),
        "editor_session_id": editor_session_id,
        "client_contract_revision": CLIENT,
        "security_policy_revision": SECURITY,
        "correlation_id": "018f0000-0000-7001-8000-00000000118f",
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
        "118f",
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
            correlation_id: "018f0000-0000-7001-8000-00000000118f".to_owned(),
            ids: AuthorCommandAdmissionIds {
                command_id: "018f0000-0000-7001-8000-00000001118f".to_owned(),
                author_command_admission_id: "018f0000-0000-7001-8000-00000002118f".to_owned(),
                receipt_id: "018f0000-0000-7001-8000-00000003118f".to_owned(),
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
        panic!("Update Chapter Undo must write structure Compensation");
    };
    assert_eq!(source_sequence, authority.author_action_sequence);
    let GetManuscriptTree::Found(tree) = get_manuscript_tree(&store, &scope).await.unwrap() else {
        panic!("the tree remains after Update Chapter Compensation");
    };
    assert_eq!(tree.tree_revision, 5);
    assert_eq!(tree.snapshot.snapshot_id, snapshot_id);
    assert_eq!(
        tree.volumes,
        vec![VolumeNode {
            volume_id: VolumeId::new(volume_id),
            title: VOLUME_TITLE.to_owned(),
            order: 1,
            chapters: vec![
                ChapterNode {
                    chapter_id: ChapterId::new(chapter_b),
                    title: "Chapter B".to_owned(),
                    order: 1,
                },
                ChapterNode {
                    chapter_id: ChapterId::new(chapter_a),
                    title: "Chapter A".to_owned(),
                    order: 2,
                },
            ],
        }]
    );
    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    let observed: serde_json::Value = serde_json::from_str(
        &admin
            .query_one(
                "SELECT jsonb_build_object(
                          'prior_title', (
                            SELECT payload->>'prior_title'
                              FROM storyos.project_activity_event_payloads
                             WHERE project_id = $1::text::uuid
                               AND receipt_id = $2::text::uuid
                          ),
                          'prior_order', (
                            SELECT payload->>'prior_order'
                              FROM storyos.project_activity_event_payloads
                             WHERE project_id = $1::text::uuid
                               AND receipt_id = $2::text::uuid
                          ),
                          'compensation_actions', (
                            SELECT count(*) FROM storyos.author_action_entries
                             WHERE project_id = $1::text::uuid
                               AND disposition = 'compensation'
                               AND compensated_source_sequence = $3::text::numeric
                          ),
                          'forward_update_actions', (
                            SELECT count(*) FROM storyos.author_action_entries
                             WHERE project_id = $1::text::uuid
                               AND disposition = 'forward'
                               AND receipt_id IN (
                                 SELECT receipt_id FROM storyos.domain_receipts
                                  WHERE project_id = $1::text::uuid
                                    AND command_kind = 'updateChapter'
                               )
                          ),
                          'chapter_removal_decisions', (
                            SELECT count(*) FROM storyos.chapter_removal_decisions
                             WHERE project_id = $1::text::uuid
                          )
                        )::text",
                &[
                    &scope.project_id.as_ref(),
                    &updated.ids.receipt_id,
                    &source_sequence.to_string(),
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
            "prior_title": "Chapter A",
            "prior_order": "2",
            "compensation_actions": 1,
            "forward_update_actions": 2,
            "chapter_removal_decisions": 0,
        })
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_compensates_update_chapter_and_restores_prior_live_sibling_place() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let scope = seed_project(&store, "11c0").await;
    let volume_id = apply_volume(&store, &scope, "11c2").await;
    let chapter_a = apply_chapter(
        &store,
        &scope,
        "11c4",
        &volume_id,
        "Chapter A",
        CHAPTER_A_BYTES,
        /*expected_tree_revision*/ 2,
    )
    .await;
    let chapter_b = apply_chapter(
        &store,
        &scope,
        "11c6",
        &volume_id,
        "Chapter B",
        CHAPTER_B_BYTES,
        /*expected_tree_revision*/ 3,
    )
    .await;
    let move_a = br#"{"expected_tree_revision":"4","order":"2","title":"Renamed A"}"#;
    let updated = apply_update(
        &store,
        &scope,
        "11c8",
        UpdateFixture {
            chapter_id: &chapter_a,
            title: "Renamed A",
            order: 2,
            expected_tree_revision: 4,
            bytes: move_a,
        },
    )
    .await;
    let UpdateChapterSettlementEffect::Applied { tree_revision, .. } = updated.effect else {
        panic!("Move A must apply");
    };
    assert_eq!(tree_revision, 5);
    let authority = updated
        .authority
        .clone()
        .expect("Applied Update Chapter must write authority");
    let GetManuscriptTree::Found(tree_after_move) =
        get_manuscript_tree(&store, &scope).await.unwrap()
    else {
        panic!("the tree remains after Update Chapter");
    };
    assert_eq!(
        tree_after_move.volumes,
        vec![VolumeNode {
            volume_id: VolumeId::new(volume_id.clone()),
            title: VOLUME_TITLE.to_owned(),
            order: 1,
            chapters: vec![
                ChapterNode {
                    chapter_id: ChapterId::new(chapter_b.clone()),
                    title: "Chapter B".to_owned(),
                    order: 1,
                },
                ChapterNode {
                    chapter_id: ChapterId::new(chapter_a.clone()),
                    title: "Renamed A".to_owned(),
                    order: 2,
                },
            ],
        }]
    );
    let session_issue = named_issue(
        &scope,
        "11ca",
        "POST",
        "/api/v1/projects/{project_id}/editor-sessions",
        "storyos.command.create-editor-session.request.v1",
        "createEditorSession",
        "sha256:storyos.test:11ca",
    );
    issue_project_command_challenge(&store, &session_issue)
        .await
        .unwrap();
    let editor_session_id = "018f0000-0000-7001-8000-0000000011cb";
    create_editor_session(
        &store,
        &OpenEditorSession {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(editor_session_id),
            snapshot_id: "018f0000-0000-7001-8000-0000000011cc".to_owned(),
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
        open_chapter(&store, &scope, &ChapterId::new(chapter_a.clone()))
            .await
            .unwrap()
    else {
        panic!("Chapter A must open");
    };
    let undo_input = serde_json::json!({
        "expected_author_undo_frontier_sequence": authority.author_action_sequence.to_string(),
        "expected_authoritative_revision_id": opened.chapter.revision_id.as_ref(),
        "editor_session_id": editor_session_id,
        "client_contract_revision": CLIENT,
        "security_policy_revision": SECURITY,
        "correlation_id": "018f0000-0000-7001-8000-0000000011ce",
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
        "11ce",
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
            correlation_id: "018f0000-0000-7001-8000-0000000011ce".to_owned(),
            ids: AuthorCommandAdmissionIds {
                command_id: "018f0000-0000-7001-8000-0000000111ce".to_owned(),
                author_command_admission_id: "018f0000-0000-7001-8000-0000000211ce".to_owned(),
                receipt_id: "018f0000-0000-7001-8000-0000000311ce".to_owned(),
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
        panic!("Update Chapter Undo must write structure Compensation");
    };
    assert_eq!(source_sequence, authority.author_action_sequence);
    let GetManuscriptTree::Found(tree) = get_manuscript_tree(&store, &scope).await.unwrap() else {
        panic!("the tree remains after Update Chapter Compensation");
    };
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
                    chapter_id: ChapterId::new(chapter_a),
                    title: "Chapter A".to_owned(),
                    order: 1,
                },
                ChapterNode {
                    chapter_id: ChapterId::new(chapter_b),
                    title: "Chapter B".to_owned(),
                    order: 2,
                },
            ],
        }]
    );
    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    let observed: serde_json::Value = serde_json::from_str(
        &admin
            .query_one(
                "SELECT jsonb_build_object(
                          'prior_title', (
                            SELECT payload->>'prior_title'
                              FROM storyos.project_activity_event_payloads
                             WHERE project_id = $1::text::uuid
                               AND receipt_id = $2::text::uuid
                          ),
                          'prior_order', (
                            SELECT payload->>'prior_order'
                              FROM storyos.project_activity_event_payloads
                             WHERE project_id = $1::text::uuid
                               AND receipt_id = $2::text::uuid
                          ),
                          'compensation_actions', (
                            SELECT count(*) FROM storyos.author_action_entries
                             WHERE project_id = $1::text::uuid
                               AND disposition = 'compensation'
                               AND compensated_source_sequence = $3::text::numeric
                          ),
                          'forward_update_actions', (
                            SELECT count(*) FROM storyos.author_action_entries
                             WHERE project_id = $1::text::uuid
                               AND disposition = 'forward'
                               AND receipt_id IN (
                                 SELECT receipt_id FROM storyos.domain_receipts
                                  WHERE project_id = $1::text::uuid
                                    AND command_kind = 'updateChapter'
                               )
                          ),
                          'chapter_removal_decisions', (
                            SELECT count(*) FROM storyos.chapter_removal_decisions
                             WHERE project_id = $1::text::uuid
                          )
                        )::text",
                &[
                    &scope.project_id.as_ref(),
                    &updated.ids.receipt_id,
                    &source_sequence.to_string(),
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
            "prior_title": "Chapter A",
            "prior_order": "1",
            "compensation_actions": 1,
            "forward_update_actions": 1,
            "chapter_removal_decisions": 0,
        })
    );
}
