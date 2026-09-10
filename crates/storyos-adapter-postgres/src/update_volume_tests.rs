use super::*;
use storyos_application::{
    AuthorCommandAdmissionIds, ChapterId, ChapterNode, CreateChapterCommand,
    CreateChapterSettlementEffect, CreateProjectChallengeBinding, CreateProjectCommand,
    CreateVolumeCommand, CreateVolumeSettlementEffect, EditorClientBinding, EditorSessionId,
    IssueCreateProjectChallenge, IssueProjectCommandChallenge, OpenChapter, OpenEditorSession,
    ProjectCommandChallengeBinding, ProjectId, ProjectScope, UndoLatestAuthorActionCommand,
    UndoLatestAuthorActionSettlementEffect, UpdateVolumeCommand, UpdateVolumeSettlementEffect,
    UserId, VolumeId, VolumeNode, create_chapter, create_editor_session, create_project,
    create_volume, get_manuscript_tree, issue_create_project_challenge,
    issue_project_command_challenge, open_chapter, undo_latest_author_action, update_volume,
};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const CLIENT: &str = "storyos.web-client.release-1.v3";
const SECURITY: &str = "storyos.web-security-policy.release-1.v1";
const VOLUME_A_BYTES: &[u8] = br#"{"expected_tree_revision":"1","title":"Volume A"}"#;
const VOLUME_A_DIGEST: &str = "sha256:storyos.command.createVolume.jcs.v1:2b02ae40bec5ed5ccf7ec412519d2178186dc80e3829100851514a6f3422cedf";
const VOLUME_B_BYTES: &[u8] = br#"{"expected_tree_revision":"2","title":"Volume B"}"#;
const VOLUME_B_DIGEST: &str = "sha256:storyos.command.createVolume.jcs.v1:1093f06260a76658116fbc95ea1503ed34ef1b3a7bcc07cc187e1c89f4a6bea2";
const UPDATE_BYTES: &[u8] = br#"{"expected_tree_revision":"3","order":"2","title":"Volume B"}"#;
const UPDATE_DIGEST: &str = "sha256:storyos.command.updateVolume.jcs.v1:577c976b262b0e8bf54e7adb4b30b4f98597b0bcfab3fd877ba65042fac9d28c";
const INVALID_ORDER_BYTES: &[u8] =
    br#"{"expected_tree_revision":"3","order":"3","title":"Volume B"}"#;
const INVALID_ORDER_DIGEST: &str = "sha256:storyos.command.updateVolume.jcs.v1:5a19077e90043813455d23f77e75e6361894998627ca4767b09879e9d75f6edb";
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

fn volume_issue(
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
            route_template: "/api/v1/projects/{project_id}/volumes".to_owned(),
            command_schema: "storyos.command.create-volume.request.v1".to_owned(),
            command_kind: "createVolume".to_owned(),
            canonical_command_digest: digest.to_owned(),
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
    title: &str,
    bytes: &[u8],
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
        canonical_command_bytes: bytes.to_vec(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{ids_suffix}"),
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
            route_template: "/api/v1/projects/{project_id}/volumes/{volume_id}".to_owned(),
            command_schema: "storyos.command.update-volume.request.v1".to_owned(),
            command_kind: "updateVolume".to_owned(),
            canonical_command_digest: digest.to_owned(),
            idempotency_key: format!("018f0000-0000-7001-8000-00000000{idempotency_suffix}"),
        },
        nonce: format!("opaque-nonce-{idempotency_suffix}"),
        nonce_digest: format!("sha256:nonce-{idempotency_suffix}"),
    }
}

struct UpdateFixture<'a> {
    volume_id: &'a str,
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
) -> UpdateVolumeCommand {
    UpdateVolumeCommand {
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
        volume_id: VolumeId::new(fixture.volume_id),
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

fn applied_fixture(volume_id: &str) -> UpdateFixture<'_> {
    UpdateFixture {
        volume_id,
        title: "Volume B",
        order: 2,
        expected_tree_revision: 3,
        bytes: UPDATE_BYTES,
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn update_volume_is_atomic_replayable_and_scope_safe() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let issue = create_project_issue("018f0000-0000-7001-8000-000000000b10", "0b10");
    let issued = issue_create_project_challenge(&store, &issue)
        .await
        .unwrap();
    let mut project_binding = issue.binding.clone();
    project_binding.prospective_project_id = issued.prospective_project_id.clone();
    project_binding.canonical_command_digest = issued.canonical_command_digest.clone();
    create_project(
        &store,
        &create_project_command(project_binding.clone(), &issue.nonce_digest, "0b11"),
    )
    .await
    .unwrap();
    let scope = ProjectScope::new(
        project_binding.owner_user_id.clone(),
        project_binding.prospective_project_id.clone(),
    );

    let first_volume_issue = volume_issue(&scope, "0b12", VOLUME_A_DIGEST);
    issue_project_command_challenge(&store, &first_volume_issue)
        .await
        .unwrap();
    let first_volume = create_volume(
        &store,
        &volume_command(
            first_volume_issue.binding.clone(),
            &first_volume_issue.nonce_digest,
            "0b13",
            "Volume A",
            VOLUME_A_BYTES,
            1,
        ),
    )
    .await
    .unwrap();
    let CreateVolumeSettlementEffect::Applied {
        volume_id: first_volume_id,
        ..
    } = first_volume.effect
    else {
        panic!("the first Volume must apply");
    };
    let second_volume_issue = volume_issue(&scope, "0b14", VOLUME_B_DIGEST);
    issue_project_command_challenge(&store, &second_volume_issue)
        .await
        .unwrap();
    let second_volume = create_volume(
        &store,
        &volume_command(
            second_volume_issue.binding.clone(),
            &second_volume_issue.nonce_digest,
            "0b15",
            "Volume B",
            VOLUME_B_BYTES,
            2,
        ),
    )
    .await
    .unwrap();
    let CreateVolumeSettlementEffect::Applied {
        volume_id: second_volume_id,
        ..
    } = second_volume.effect
    else {
        panic!("the second Volume must apply");
    };

    let first_issue = update_issue(&scope, "0b16", UPDATE_DIGEST);
    issue_project_command_challenge(&store, &first_issue)
        .await
        .unwrap();
    let first = update_volume(
        &store,
        &update_command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "0b17",
            applied_fixture(&first_volume_id),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        first.effect,
        UpdateVolumeSettlementEffect::Applied {
            title: "Volume B".to_owned(),
            order: 2,
            tree_revision: 4,
        }
    );
    let authority = first
        .authority
        .clone()
        .expect("Applied Update Volume must write Structural Authority Settlement");
    assert_eq!(authority.prior_manuscript_tree_revision, 3);
    assert_eq!(authority.resulting_manuscript_tree_revision, 4);
    assert_eq!(authority.author_action_sequence, 3);
    let replay = update_volume(
        &store,
        &update_command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "0b99",
            applied_fixture(&first_volume_id),
        ),
    )
    .await
    .unwrap();
    assert_eq!(replay, first);

    let tree = get_manuscript_tree(&store, &scope)
        .await
        .unwrap()
        .expect("the Project still has a Canonical Query");
    assert_eq!(tree.tree_revision, 4);
    assert_eq!(tree.snapshot.snapshot_id, authority.snapshot_id);
    assert_eq!(
        tree.snapshot.project_activity_position,
        first.project_activity_position
    );
    assert_eq!(
        tree.volumes,
        vec![
            VolumeNode {
                volume_id: VolumeId::new(second_volume_id.clone()),
                title: "Volume B".to_owned(),
                order: 1,
                chapters: Vec::new(),
            },
            VolumeNode {
                volume_id: VolumeId::new(first_volume_id.clone()),
                title: "Volume B".to_owned(),
                order: 2,
                chapters: Vec::new(),
            },
        ]
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

    let stale_issue = update_issue(&scope, "0b18", UPDATE_DIGEST);
    issue_project_command_challenge(&store, &stale_issue)
        .await
        .unwrap();
    let stale = update_volume(
        &store,
        &update_command(
            stale_issue.binding.clone(),
            &stale_issue.nonce_digest,
            "0b19",
            applied_fixture(&first_volume_id),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        stale.effect,
        UpdateVolumeSettlementEffect::Conflicted {
            reason: storyos_core::UpdateVolumeConflict::StaleTreeRevision,
        }
    );
    assert_eq!(stale.authority, None);

    let missing_issue = update_issue(&scope, "0b1a", UPDATE_DIGEST);
    issue_project_command_challenge(&store, &missing_issue)
        .await
        .unwrap();
    let missing = update_volume(
        &store,
        &update_command(
            missing_issue.binding.clone(),
            &missing_issue.nonce_digest,
            "0b1b",
            applied_fixture(MISSING_VOLUME),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        missing.effect,
        UpdateVolumeSettlementEffect::Refused {
            reason: storyos_core::UpdateVolumeRefusal::InvalidVolumeJoin,
        }
    );
    assert_eq!(missing.authority, None);

    let invalid_issue = update_issue(&scope, "0b1c", INVALID_ORDER_DIGEST);
    issue_project_command_challenge(&store, &invalid_issue)
        .await
        .unwrap();
    let invalid = update_volume(
        &store,
        &update_command(
            invalid_issue.binding.clone(),
            &invalid_issue.nonce_digest,
            "0b1d",
            UpdateFixture {
                volume_id: &first_volume_id,
                title: "Volume B",
                order: 3,
                expected_tree_revision: 3,
                bytes: INVALID_ORDER_BYTES,
            },
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        invalid.effect,
        UpdateVolumeSettlementEffect::Refused {
            reason: storyos_core::UpdateVolumeRefusal::InvalidOrder,
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
                      WHERE project_id = $1::text::uuid AND object_kind = 'volume'),
                    (SELECT count(*) FROM storyos.domain_receipts
                      WHERE project_id = $1::text::uuid AND command_kind = 'updateVolume'),
                    (SELECT count(*) FROM storyos.project_activity_event_payloads
                      WHERE project_id = $1::text::uuid AND event_kind = 'volume_updated'),
                    (SELECT count(*) FROM storyos.authoritative_commits
                      WHERE project_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.author_action_entries
                      WHERE project_id = $1::text::uuid AND disposition = 'forward'),
                    (SELECT manuscript_object_id IS NULL
                              AND prior_revision_id IS NULL
                              AND resulting_revision_id IS NULL
                              AND affected_volume_id = $2::text::uuid
                              AND prior_manuscript_tree_revision = 3
                              AND resulting_manuscript_tree_revision = 4
                       FROM storyos.authoritative_commits
                      WHERE project_id = $1::text::uuid
                        AND authoritative_commit_id = $3::text::uuid),
                    (SELECT count(*) FROM storyos.authoritative_commits
                      WHERE receipt_id = $4::text::uuid),
                    (SELECT payload.payload->>'prior_title'
                       FROM storyos.project_activity_event_payloads AS payload
                      WHERE payload.project_id = $1::text::uuid
                        AND payload.event_kind = 'volume_updated'),
                    (SELECT payload.payload->>'prior_order'
                       FROM storyos.project_activity_event_payloads AS payload
                      WHERE payload.project_id = $1::text::uuid
                        AND payload.event_kind = 'volume_updated')
               FROM storyos.projects
              WHERE project_id = $1::text::uuid",
            &[
                &scope.project_id.as_ref(),
                &first_volume_id,
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
            "4".to_owned(),
            2,
            4,
            1,
            3,
            3,
            true,
            0,
            "Volume A".to_owned(),
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
    let archived_issue = update_issue(&scope, "0b1e", UPDATE_DIGEST);
    issue_project_command_challenge(&store, &archived_issue)
        .await
        .unwrap();
    let archived = update_volume(
        &store,
        &update_command(
            archived_issue.binding.clone(),
            archived_issue.nonce_digest.as_str(),
            "0b1f",
            applied_fixture(&first_volume_id),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        archived.effect,
        UpdateVolumeSettlementEffect::Refused {
            reason: storyos_core::UpdateVolumeRefusal::ArchivedProject,
        }
    );
    assert_eq!(archived.authority, None);
    let volumes_after_refuse = admin
        .query_one(
            "SELECT count(*) FROM storyos.manuscript_objects
              WHERE project_id = $1::text::uuid AND object_kind = 'volume'",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(volumes_after_refuse, 2);
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

async fn apply_volume(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    title: &str,
    bytes: &[u8],
    digest: &str,
    expected_tree_revision: u64,
) -> String {
    let issue = volume_issue(scope, suffix, digest);
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    let settlement = create_volume(
        store,
        &volume_command(
            issue.binding,
            &issue.nonce_digest,
            suffix,
            title,
            bytes,
            expected_tree_revision,
        ),
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
    bytes: &[u8],
    expected_tree_revision: u64,
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
        panic!("{title} must apply");
    };
    chapter_id
}

async fn apply_update(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: &str,
    fixture: UpdateFixture<'_>,
) -> storyos_application::UpdateVolumeSettlement {
    let digest = format!(
        "sha256:storyos.command.updateVolume.jcs.v1:{}",
        crate::author_edit::sha256_hex(fixture.bytes)
    );
    let issue = update_issue(scope, suffix, &digest);
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    update_volume(
        store,
        &update_command(issue.binding, &issue.nonce_digest, suffix, fixture),
    )
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_compensates_update_volume_and_restores_title_and_canonical_sibling_order() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let scope = seed_project(&store, "0b30").await;
    let volume_a = apply_volume(
        &store,
        &scope,
        "0b32",
        "Volume A",
        VOLUME_A_BYTES,
        VOLUME_A_DIGEST,
        1,
    )
    .await;
    let volume_b = apply_volume(
        &store,
        &scope,
        "0b34",
        "Volume B",
        VOLUME_B_BYTES,
        VOLUME_B_DIGEST,
        2,
    )
    .await;
    let chapter_bytes = br#"{"expected_tree_revision":"3","title":"Chapter A"}"#;
    let chapter_id = apply_chapter(
        &store,
        &scope,
        "0b35",
        &volume_a,
        "Chapter A",
        chapter_bytes,
        3,
    )
    .await;
    // Sibling Update Volume changes A's live order. A's last Activity still says order 1.
    let move_b = br#"{"expected_tree_revision":"4","order":"1","title":"Volume B"}"#;
    apply_update(
        &store,
        &scope,
        "0b36",
        UpdateFixture {
            volume_id: &volume_b,
            title: "Volume B",
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
        "0b38",
        UpdateFixture {
            volume_id: &volume_a,
            title: "Renamed A",
            order: 2,
            expected_tree_revision: 5,
            bytes: rename_a,
        },
    )
    .await;
    let UpdateVolumeSettlementEffect::Applied { tree_revision, .. } = updated.effect else {
        panic!("Rename A must apply");
    };
    assert_eq!(tree_revision, 6);
    let authority = updated
        .authority
        .clone()
        .expect("Applied Update Volume must write authority");
    let session_issue = named_issue(
        &scope,
        "0b3a",
        "POST",
        "/api/v1/projects/{project_id}/editor-sessions",
        "storyos.command.create-editor-session.request.v1",
        "createEditorSession",
        "sha256:storyos.test:0b3a",
    );
    issue_project_command_challenge(&store, &session_issue)
        .await
        .unwrap();
    let editor_session_id = "018f0000-0000-7001-8000-000000000b3b";
    create_editor_session(
        &store,
        &OpenEditorSession {
            project_scope: scope.clone(),
            editor_session_id: EditorSessionId::new(editor_session_id),
            snapshot_id: "018f0000-0000-7001-8000-000000000b3c".to_owned(),
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
        open_chapter(&store, &scope, &ChapterId::new(chapter_id.clone()))
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
        "correlation_id": "018f0000-0000-7001-8000-000000000b3e",
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
        "0b3e",
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
            correlation_id: "018f0000-0000-7001-8000-000000000b3e".to_owned(),
            ids: AuthorCommandAdmissionIds {
                command_id: "018f0000-0000-7001-8000-000000010b3e".to_owned(),
                author_command_admission_id: "018f0000-0000-7001-8000-000000020b3e".to_owned(),
                receipt_id: "018f0000-0000-7001-8000-000000030b3e".to_owned(),
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
        panic!("Update Volume Undo must write structure Compensation");
    };
    assert_eq!(source_sequence, authority.author_action_sequence);
    let tree = get_manuscript_tree(&store, &scope)
        .await
        .unwrap()
        .expect("the tree remains after Update Volume Compensation");
    assert_eq!(tree.tree_revision, 5);
    assert_eq!(tree.snapshot.snapshot_id, snapshot_id);
    assert_eq!(
        tree.volumes,
        vec![
            VolumeNode {
                volume_id: VolumeId::new(volume_b),
                title: "Volume B".to_owned(),
                order: 1,
                chapters: Vec::new(),
            },
            VolumeNode {
                volume_id: VolumeId::new(volume_a),
                title: "Volume A".to_owned(),
                order: 2,
                chapters: vec![ChapterNode {
                    chapter_id: ChapterId::new(chapter_id),
                    title: "Chapter A".to_owned(),
                    order: 1,
                }],
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
                                    AND command_kind = 'updateVolume'
                               )
                          ),
                          'volume_removal_decisions', (
                            SELECT count(*) FROM storyos.volume_removal_decisions
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
            "prior_title": "Volume A",
            "prior_order": "2",
            "compensation_actions": 1,
            "forward_update_actions": 2,
            "volume_removal_decisions": 0,
        })
    );
}
