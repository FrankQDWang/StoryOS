use super::*;
use storyos_application::{
    AuthorCommandAdmissionIds, CreateProjectChallengeBinding, CreateProjectCommand,
    CreateVolumeCommand, CreateVolumePublicOrder, CreateVolumeSettlementEffect,
    EditorClientBinding, IssueCreateProjectChallenge, IssueProjectCommandChallenge,
    ProjectCommandChallengeBinding, ProjectId, ProjectScope, UserId, VolumeId, VolumeNode,
    create_project, create_volume, get_manuscript_tree, issue_create_project_challenge,
    issue_project_command_challenge,
};
use tokio_postgres::NoTls;

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const CLIENT: &str = "storyos.web-client.release-1.v3";
const SECURITY: &str = "storyos.web-security-policy.release-1.v1";
const TITLE: &str = "Volume A";
const COMMAND_BYTES: &[u8] = br#"{"expected_tree_revision":"1","title":"Volume A"}"#;
const COMMAND_DIGEST: &str = "sha256:storyos.command.createVolume.jcs.v1:2b02ae40bec5ed5ccf7ec412519d2178186dc80e3829100851514a6f3422cedf";
const VOLUME_B_TITLE: &str = "Volume B";
const VOLUME_B_BYTES: &[u8] = br#"{"expected_tree_revision":"2","title":"Volume B"}"#;
const VOLUME_B_DIGEST: &str = "sha256:storyos.command.createVolume.jcs.v1:1093f06260a76658116fbc95ea1503ed34ef1b3a7bcc07cc187e1c89f4a6bea2";

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

fn issue_request(scope: &ProjectScope, idempotency_suffix: &str) -> IssueProjectCommandChallenge {
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

fn command(
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

fn titled_command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
    title: &str,
    bytes: &[u8],
    expected_tree_revision: u64,
) -> CreateVolumeCommand {
    let mut created = command(binding, nonce_digest, ids_suffix, expected_tree_revision);
    created.title = title.to_owned();
    created.canonical_command_bytes = bytes.to_vec();
    created
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn create_volume_is_atomic_replayable_and_scope_safe() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let issue = create_project_issue("018f0000-0000-7001-8000-000000000710", "0710");
    let issued = issue_create_project_challenge(&store, &issue)
        .await
        .unwrap();
    let mut project_binding = issue.binding.clone();
    project_binding.prospective_project_id = issued.prospective_project_id.clone();
    project_binding.canonical_command_digest = issued.canonical_command_digest.clone();
    create_project(
        &store,
        &create_project_command(project_binding.clone(), &issue.nonce_digest, "0711"),
    )
    .await
    .unwrap();
    let scope = ProjectScope::new(
        project_binding.owner_user_id.clone(),
        project_binding.prospective_project_id.clone(),
    );

    let first_issue = issue_request(&scope, "0712");
    issue_project_command_challenge(&store, &first_issue)
        .await
        .unwrap();
    let first = create_volume(
        &store,
        &command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "0713",
            1,
        ),
    )
    .await
    .unwrap();
    let CreateVolumeSettlementEffect::Applied {
        tree_revision,
        volume_id,
        order,
    } = first.effect.clone()
    else {
        panic!("Create Volume on an empty active Project must apply");
    };
    assert_eq!(tree_revision, 2);
    assert_eq!(order, CreateVolumePublicOrder::CanonicalSiblingOrder(1));
    let replay = create_volume(
        &store,
        &command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "0799",
            1,
        ),
    )
    .await
    .unwrap();
    assert_eq!(replay, first);

    let tree = get_manuscript_tree(&store, &scope)
        .await
        .unwrap()
        .expect("the Project still has a Canonical Query");
    assert_eq!(tree.project_scope, scope);
    assert_eq!(tree.tree_revision, 2);
    assert_eq!(
        tree.volumes,
        vec![VolumeNode {
            volume_id: VolumeId::new(volume_id.clone()),
            title: TITLE.to_owned(),
            order: 1,
            chapters: Vec::new(),
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

    let stale_issue = issue_request(&scope, "0714");
    issue_project_command_challenge(&store, &stale_issue)
        .await
        .unwrap();
    let stale = create_volume(
        &store,
        &command(
            stale_issue.binding.clone(),
            &stale_issue.nonce_digest,
            "0715",
            1,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        stale.effect,
        CreateVolumeSettlementEffect::Conflicted {
            reason: storyos_core::CreateVolumeConflict::StaleTreeRevision,
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
                      WHERE project_id = $1::text::uuid AND command_kind = 'createVolume'),
                    (SELECT count(*) FROM storyos.project_activity_event_payloads
                      WHERE project_id = $1::text::uuid AND event_kind = 'volume_created')
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
        ("2".to_owned(), 1, 2, 1)
    );

    admin
        .execute(
            "UPDATE storyos.projects SET lifecycle_state = 'archived'
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&USER_A, &scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let archived_issue = issue_request(&scope, "0716");
    issue_project_command_challenge(&store, &archived_issue)
        .await
        .unwrap();
    let archived = create_volume(
        &store,
        &command(
            archived_issue.binding.clone(),
            archived_issue.nonce_digest.as_str(),
            "0717",
            1,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        archived.effect,
        CreateVolumeSettlementEffect::Refused {
            reason: storyos_core::CreateVolumeRefusal::ArchivedProject,
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
    assert_eq!(volumes_after_refuse, 1);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn create_volume_replays_canonical_sibling_order_and_keeps_historical_acks() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let issue = create_project_issue("018f0000-0000-7001-8000-000000000720", "0720");
    let issued = issue_create_project_challenge(&store, &issue)
        .await
        .unwrap();
    let mut project_binding = issue.binding.clone();
    project_binding.prospective_project_id = issued.prospective_project_id.clone();
    project_binding.canonical_command_digest = issued.canonical_command_digest.clone();
    create_project(
        &store,
        &create_project_command(project_binding.clone(), &issue.nonce_digest, "0721"),
    )
    .await
    .unwrap();
    let scope = ProjectScope::new(
        project_binding.owner_user_id.clone(),
        project_binding.prospective_project_id.clone(),
    );

    let first_issue = issue_request(&scope, "0722");
    issue_project_command_challenge(&store, &first_issue)
        .await
        .unwrap();
    let first = create_volume(
        &store,
        &command(
            first_issue.binding.clone(),
            &first_issue.nonce_digest,
            "0723",
            1,
        ),
    )
    .await
    .unwrap();
    let CreateVolumeSettlementEffect::Applied {
        order: first_order, ..
    } = first.effect.clone()
    else {
        panic!("the first Create Volume must apply");
    };
    assert_eq!(
        first_order,
        CreateVolumePublicOrder::CanonicalSiblingOrder(1)
    );

    let mut second_issue = issue_request(&scope, "0724");
    second_issue.binding.canonical_command_digest = VOLUME_B_DIGEST.to_owned();
    issue_project_command_challenge(&store, &second_issue)
        .await
        .unwrap();
    let second = create_volume(
        &store,
        &titled_command(
            second_issue.binding.clone(),
            &second_issue.nonce_digest,
            "0725",
            VOLUME_B_TITLE,
            VOLUME_B_BYTES,
            2,
        ),
    )
    .await
    .unwrap();
    let CreateVolumeSettlementEffect::Applied {
        tree_revision,
        volume_id,
        order,
    } = second.effect.clone()
    else {
        panic!("the second Create Volume must apply");
    };
    assert_eq!(tree_revision, 3);
    assert_eq!(order, CreateVolumePublicOrder::CanonicalSiblingOrder(2));
    let replay = create_volume(
        &store,
        &titled_command(
            second_issue.binding.clone(),
            &second_issue.nonce_digest,
            "0798",
            VOLUME_B_TITLE,
            VOLUME_B_BYTES,
            2,
        ),
    )
    .await
    .unwrap();
    assert_eq!(replay, second);

    let tree = get_manuscript_tree(&store, &scope)
        .await
        .unwrap()
        .expect("the Project still has a Canonical Query");
    assert_eq!(tree.tree_revision, 3);
    assert_eq!(tree.volumes[1].order, 2);
    assert_eq!(tree.volumes[1].volume_id, VolumeId::new(volume_id.clone()));

    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    let stored = admin
        .query_one(
            "SELECT receipt.result_payload::text,
                    payload.payload->>'order'
               FROM storyos.domain_receipts AS receipt
               JOIN storyos.project_activity_event_payloads AS payload
                 ON (payload.owner_user_id, payload.project_id, payload.receipt_id) =
                    (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
              WHERE receipt.receipt_id = $1::text::uuid",
            &[&second.ids.receipt_id],
        )
        .await
        .unwrap();
    let stored_receipt: serde_json::Value =
        serde_json::from_str(&stored.get::<_, String>(0)).unwrap();
    assert_eq!(stored_receipt, serde_json::json!({"order": "2"}));
    assert_eq!(stored.get::<_, String>(1), "2");

    admin
        .execute(
            "UPDATE storyos.domain_receipts
                SET result_payload = '{}'::jsonb
              WHERE receipt_id = $1::text::uuid",
            &[&second.ids.receipt_id],
        )
        .await
        .unwrap();
    let historical = create_volume(
        &store,
        &titled_command(
            second_issue.binding.clone(),
            &second_issue.nonce_digest,
            "0797",
            VOLUME_B_TITLE,
            VOLUME_B_BYTES,
            2,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        historical.effect,
        CreateVolumeSettlementEffect::Applied {
            tree_revision,
            volume_id,
            order: CreateVolumePublicOrder::HistoricalCreateVolumeAck,
        }
    );
}
