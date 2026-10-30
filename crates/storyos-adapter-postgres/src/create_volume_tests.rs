use super::*;
use storyos_application::{
    CreateVolumeInput, CreateVolumePublicOrder, GetManuscriptTree, ProjectCommandChallengeBinding,
    ProjectScope, SnapshotLookup, UserId, VolumeCreated, VolumeId, VolumeNode, create_project,
    get_manuscript_tree, get_snapshot, issue_create_project_challenge,
    issue_project_command_challenge,
};
use storyos_core::TransitionOutcome;
use tokio_postgres::NoTls;

use crate::command_sequence::tests::{
    CREATE_VOLUME, CommandCall, applied, command_call, command_issue, create_project_command,
    create_project_issue, create_volume,
};

const USER_A: &str = "018f0000-0000-7001-8000-000000000001";
const USER_B: &str = "018f0000-0000-7001-8000-000000000101";
const TITLE: &str = "Volume A";
const COMMAND_BYTES: &[u8] = br#"{"expected_tree_revision":"1","title":"Volume A"}"#;
const COMMAND_DIGEST: &str = "sha256:storyos.command.createVolume.jcs.v1:2b02ae40bec5ed5ccf7ec412519d2178186dc80e3829100851514a6f3422cedf";
const VOLUME_B_TITLE: &str = "Volume B";
const VOLUME_B_BYTES: &[u8] = br#"{"expected_tree_revision":"2","title":"Volume B"}"#;
const VOLUME_B_DIGEST: &str = "sha256:storyos.command.createVolume.jcs.v1:1093f06260a76658116fbc95ea1503ed34ef1b3a7bcc07cc187e1c89f4a6bea2";

fn command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
    expected_tree_revision: u64,
) -> CommandCall<CreateVolumeInput> {
    command_call(
        binding,
        nonce_digest,
        ids_suffix,
        COMMAND_BYTES,
        CreateVolumeInput {
            title: TITLE.to_owned(),
            expected_tree_revision,
        },
    )
}

fn titled_command(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
    title: &str,
    bytes: &[u8],
    expected_tree_revision: u64,
) -> CommandCall<CreateVolumeInput> {
    command_call(
        binding,
        nonce_digest,
        ids_suffix,
        bytes,
        CreateVolumeInput {
            title: title.to_owned(),
            expected_tree_revision,
        },
    )
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
    let issue = create_project_issue(USER_A, "018f0000-0000-7001-8000-000000000710", "0710");
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

    let first_issue = command_issue(&scope, "0712", &CREATE_VOLUME, COMMAND_DIGEST);
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
    let (
        VolumeCreated {
            tree_revision,
            volume_id,
            order,
        },
        authority,
    ) = applied(&first);
    assert_eq!(tree_revision, 2);
    assert_eq!(order, CreateVolumePublicOrder::CanonicalSiblingOrder(1));
    assert_eq!(authority.prior_manuscript_tree_revision, 1);
    assert_eq!(authority.resulting_manuscript_tree_revision, 2);
    assert_eq!(authority.author_action_sequence, 1);
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

    let GetManuscriptTree::Found(tree) = get_manuscript_tree(&store, &scope).await.unwrap() else {
        panic!("the Project still has a Canonical Query");
    };
    assert_eq!(tree.project_scope, scope);
    assert_eq!(tree.tree_revision, 2);
    assert_eq!(tree.snapshot.snapshot_id, authority.snapshot_id);
    let TransitionOutcome::Applied(first_applied) = &first.outcome else {
        panic!("Create Volume on an empty active Project must apply");
    };
    assert_eq!(
        tree.snapshot.project_activity_position,
        first_applied.project_activity_position
    );
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
        GetManuscriptTree::Missing
    );

    let stale_issue = command_issue(&scope, "0714", &CREATE_VOLUME, COMMAND_DIGEST);
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
        stale.outcome,
        TransitionOutcome::Conflicted(storyos_core::CreateVolumeConflict::StaleTreeRevision)
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
                      WHERE project_id = $1::text::uuid AND event_kind = 'volume_created'),
                    (SELECT count(*) FROM storyos.authoritative_commits
                      WHERE project_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.author_action_entries
                      WHERE project_id = $1::text::uuid AND disposition = 'forward'),
                    (SELECT manuscript_object_id IS NULL
                              AND prior_revision_id IS NULL
                              AND resulting_revision_id IS NULL
                              AND affected_volume_id = $2::text::uuid
                              AND prior_manuscript_tree_revision = 1
                              AND resulting_manuscript_tree_revision = 2
                       FROM storyos.authoritative_commits
                      WHERE project_id = $1::text::uuid
                        AND authoritative_commit_id = $3::text::uuid),
                    (SELECT count(*) FROM storyos.authoritative_commits
                      WHERE receipt_id = $4::text::uuid)
               FROM storyos.projects
              WHERE project_id = $1::text::uuid",
            &[
                &scope.project_id.as_ref(),
                &volume_id,
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
            row.get::<_, i64>(7)
        ),
        ("2".to_owned(), 1, 2, 1, 1, 1, true, 0)
    );

    admin
        .execute(
            "UPDATE storyos.projects SET lifecycle_state = 'archived'
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&USER_A, &scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let archived_issue = command_issue(&scope, "0716", &CREATE_VOLUME, COMMAND_DIGEST);
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
        archived.outcome,
        TransitionOutcome::Refused(storyos_core::CreateVolumeRefusal::ArchivedProject)
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
    let issue = create_project_issue(USER_A, "018f0000-0000-7001-8000-000000000720", "0720");
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

    let first_issue = command_issue(&scope, "0722", &CREATE_VOLUME, COMMAND_DIGEST);
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
    let (
        VolumeCreated {
            order: first_order, ..
        },
        _,
    ) = applied(&first);
    assert_eq!(
        first_order,
        CreateVolumePublicOrder::CanonicalSiblingOrder(1)
    );

    let mut second_issue = command_issue(&scope, "0724", &CREATE_VOLUME, COMMAND_DIGEST);
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
    let (
        VolumeCreated {
            tree_revision,
            volume_id,
            order,
        },
        second_authority,
    ) = applied(&second);
    assert_eq!(tree_revision, 3);
    assert_eq!(order, CreateVolumePublicOrder::CanonicalSiblingOrder(2));
    assert_eq!(second_authority.author_action_sequence, 2);
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

    let GetManuscriptTree::Found(tree) = get_manuscript_tree(&store, &scope).await.unwrap() else {
        panic!("the Project still has a Canonical Query");
    };
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
        applied(&historical).0,
        VolumeCreated {
            tree_revision,
            volume_id,
            order: CreateVolumePublicOrder::HistoricalCreateVolumeAck,
        }
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn stale_latest_snapshot_resyncs_and_does_not_return_the_live_tree() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let store = PostgresProjectReader::new(runtime_url);
    let project_issue =
        create_project_issue(USER_A, "018f0000-0000-7001-8000-0000000006c0", "06c0");
    let issued = issue_create_project_challenge(&store, &project_issue)
        .await
        .unwrap();
    let mut project_binding = project_issue.binding.clone();
    project_binding.prospective_project_id = issued.prospective_project_id.clone();
    project_binding.canonical_command_digest = issued.canonical_command_digest.clone();
    create_project(
        &store,
        &create_project_command(project_binding.clone(), &project_issue.nonce_digest, "06c1"),
    )
    .await
    .unwrap();
    let scope = ProjectScope::new(
        project_binding.owner_user_id.clone(),
        project_binding.prospective_project_id.clone(),
    );
    let GetManuscriptTree::Found(empty) = get_manuscript_tree(&store, &scope).await.unwrap() else {
        panic!("Create Project binds an empty Canonical Manuscript Tree");
    };
    let volume_issue = command_issue(&scope, "06c2", &CREATE_VOLUME, COMMAND_DIGEST);
    issue_project_command_challenge(&store, &volume_issue)
        .await
        .unwrap();
    let created = create_volume(
        &store,
        &command(
            volume_issue.binding.clone(),
            &volume_issue.nonce_digest,
            "06c3",
            /*expected_tree_revision*/ 1,
        ),
    )
    .await
    .unwrap();
    let (_, authority) = applied(&created);
    let GetManuscriptTree::Found(tree) = get_manuscript_tree(&store, &scope).await.unwrap() else {
        panic!("Create Volume must leave a Canonical Manuscript Tree");
    };
    assert_eq!(tree.snapshot.snapshot_id, authority.snapshot_id);
    assert_ne!(tree.snapshot.snapshot_id, empty.snapshot.snapshot_id);
    assert_eq!(tree.volumes.len(), 1);
    let recovery = get_snapshot(
        &store,
        &SnapshotLookup {
            project_scope: scope.clone(),
            snapshot_id: tree.snapshot.snapshot_id.clone(),
        },
    )
    .await
    .unwrap()
    .expect("recovery can read the latest Snapshot after the structure change");
    assert_eq!(recovery, tree.snapshot);

    let (admin, admin_connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(async move {
        admin_connection.await.unwrap();
    });
    admin
        .execute(
            "UPDATE storyos.project_snapshots
                SET expires_at = clock_timestamp() - interval '1 second'
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND snapshot_id = $3::text::uuid",
            &[&USER_A, &scope.project_id.as_ref(), &authority.snapshot_id],
        )
        .await
        .unwrap();
    assert_eq!(
        get_manuscript_tree(&store, &scope).await.unwrap(),
        GetManuscriptTree::SnapshotExpired
    );
}
