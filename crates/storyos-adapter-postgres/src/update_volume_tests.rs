use super::*;
use storyos_application::{
    AuthorCommandAdmissionIds, CreateProjectChallengeBinding, CreateProjectCommand,
    CreateVolumeCommand, CreateVolumeSettlementEffect, EditorClientBinding,
    IssueCreateProjectChallenge, IssueProjectCommandChallenge, ProjectCommandChallengeBinding,
    ProjectId, ProjectScope, UpdateVolumeCommand, UpdateVolumeSettlementEffect, UserId, VolumeId,
    VolumeNode, create_project, create_volume, get_manuscript_tree, issue_create_project_challenge,
    issue_project_command_challenge, update_volume,
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
const UPDATE_BYTES: &[u8] = br#"{"expected_volume_revision":"3","order":"2","title":"Volume B"}"#;
const UPDATE_DIGEST: &str = "sha256:storyos.command.updateVolume.jcs.v1:179d2c183c4474eb27766b7c4364ef9a4386696a87e6259947d8ed1c8909edce";
const INVALID_ORDER_BYTES: &[u8] =
    br#"{"expected_volume_revision":"3","order":"3","title":"Volume B"}"#;
const INVALID_ORDER_DIGEST: &str = "sha256:storyos.command.updateVolume.jcs.v1:102ae6cb8a2f5ecd902561ed249a75ef4caad50efa7ab0022d91e2279ec75649";
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

fn update_command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
    volume_id: &str,
    title: &str,
    order: u64,
    expected_volume_revision: u64,
    bytes: &[u8],
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
        canonical_command_bytes: bytes.to_vec(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{ids_suffix}"),
        volume_id: VolumeId::new(volume_id),
        title: title.to_owned(),
        order,
        expected_volume_revision,
        ids: AuthorCommandAdmissionIds {
            command_id: format!("018f0000-0000-7001-8000-00000001{ids_suffix}"),
            author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{ids_suffix}"),
            receipt_id: format!("018f0000-0000-7001-8000-00000003{ids_suffix}"),
        },
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
            &first_volume_id,
            "Volume B",
            2,
            3,
            UPDATE_BYTES,
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
    let replay = update_volume(
        &store,
        &update_command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "0b99",
            &first_volume_id,
            "Volume B",
            2,
            3,
            UPDATE_BYTES,
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
            &first_volume_id,
            "Volume B",
            2,
            3,
            UPDATE_BYTES,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        stale.effect,
        UpdateVolumeSettlementEffect::Conflicted {
            reason: storyos_core::UpdateVolumeConflict::StaleVolumeRevision,
        }
    );

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
            MISSING_VOLUME,
            "Volume B",
            2,
            3,
            UPDATE_BYTES,
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
            &first_volume_id,
            "Volume B",
            3,
            3,
            INVALID_ORDER_BYTES,
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
                      WHERE project_id = $1::text::uuid AND event_kind = 'volume_updated')
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
        ("4".to_owned(), 2, 4, 1)
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
            &first_volume_id,
            "Volume B",
            2,
            3,
            UPDATE_BYTES,
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
