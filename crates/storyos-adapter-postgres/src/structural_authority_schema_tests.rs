//! The Structural Authority records of each structure command: the Authority History Floor, the
//! Commit that the Author Action and the Receipt bind, and the Receipt checks.

use crate::PostgresProjectReader;
use crate::command_sequence::tests::{
    CommandCall, create_chapter_call, create_volume_call, delete_chapter_call, delete_volume_call,
    seed_project, set_current_chapter_call, stores, titled_volume, update_chapter_call,
    update_volume_call,
};
use crate::command_sequence::{ProjectCommand, settle_project_command};

async fn settled<C: ProjectCommand>(
    store: &PostgresProjectReader,
    call: &CommandCall<C>,
) -> String {
    let Ok(_) = settle_project_command(store, &call.envelope, &call.input).await else {
        panic!("the structure command must settle");
    };
    call.envelope.ids.receipt_id.clone()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn authority_history_floor_exists_without_rewriting_structure_activity() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "9780").await;
    let volume_id = titled_volume(&store, &scope, "9781", "Volume A", 1).await;
    let observed: serde_json::Value = serde_json::from_str(
        &admin
            .query_one(
                "SELECT jsonb_build_object(
                          'replay_generation', snapshot.replay_generation,
                          'floor_matches_snapshot',
                            floor.floor_activity_position
                              = snapshot.project_activity_position,
                          'replay_generation_count', (
                            SELECT count(*) FROM storyos.replay_generations
                             WHERE project_id = $1::text::uuid
                          ),
                          'volume_created_payload', (
                            SELECT payload
                              FROM storyos.project_activity_event_payloads
                             WHERE project_id = $1::text::uuid
                               AND event_kind = 'volume_created'
                          )
                        )::text
                   FROM storyos.authority_history_floors AS floor
                   JOIN storyos.project_snapshots AS snapshot
                     ON (snapshot.owner_user_id, snapshot.project_id,
                         snapshot.snapshot_id) =
                        (floor.owner_user_id, floor.project_id, floor.snapshot_id)
                  WHERE floor.project_id = $1::text::uuid",
                &[&scope.project_id.as_ref()],
            )
            .await
            .unwrap()
            .get::<_, String>(0),
    )
    .unwrap();
    assert_eq!(
        observed,
        serde_json::json!({
            "replay_generation": 1,
            "floor_matches_snapshot": true,
            "replay_generation_count": 1,
            "volume_created_payload": {
                "kind": "volume_created",
                "volume_id": volume_id,
                "title": "Volume A",
                "tree_revision": "2",
                "order": "1",
            },
        })
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_applied_structure_command_binds_its_commit_to_its_action_and_receipt() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let receipts = [
        settled(&store, &create_volume_call(&store, /*base*/ 0x9710).await).await,
        settled(&store, &create_chapter_call(&store, /*base*/ 0x9720).await).await,
        settled(&store, &update_volume_call(&store, /*base*/ 0x9730).await).await,
        settled(&store, &update_chapter_call(&store, /*base*/ 0x9740).await).await,
        settled(&store, &delete_volume_call(&store, /*base*/ 0x9750).await).await,
        settled(&store, &delete_chapter_call(&store, /*base*/ 0x9760).await).await,
        settled(
            &store,
            &set_current_chapter_call(&store, /*base*/ 0x9770).await,
        )
        .await,
    ];
    let mut observed = Vec::new();
    for receipt_id in &receipts {
        let json: String = admin
            .query_one(
                "SELECT jsonb_build_object(
                          'action_binds_commit',
                            action.authoritative_commit_id
                              IS NOT DISTINCT FROM made.authoritative_commit_id,
                          'receipt_binds_commit',
                            receipt.authoritative_commit_ids
                              = array_remove(ARRAY[made.authoritative_commit_id], NULL),
                          'commit', CASE WHEN made.authoritative_commit_id IS NOT NULL THEN
                            jsonb_build_object(
                              'tree', jsonb_build_array(made.prior_manuscript_tree_revision,
                                                        made.resulting_manuscript_tree_revision),
                              'affected', CASE WHEN made.affected_chapter_id IS NOT NULL
                                               THEN 'chapter:' || (made.affected_chapter_id::text
                                                 = payload.payload->>'chapter_id')
                                               ELSE 'volume:' || (made.affected_volume_id::text
                                                 = payload.payload->>'volume_id') END,
                              'object', CASE WHEN made.manuscript_object_id IS NULL THEN 'none'
                                             WHEN made.manuscript_object_id
                                                    = made.affected_chapter_id
                                             THEN 'affected chapter' ELSE 'other' END,
                              'revision_pair', jsonb_build_array(
                                made.prior_revision_id IS NOT NULL,
                                made.resulting_revision_id IS NOT NULL),
                              'head_is_resulting',
                                head.current_revision_id = made.resulting_revision_id
                            ) END
                        )::text
                   FROM storyos.domain_receipts AS receipt
                   JOIN storyos.author_action_entries AS action
                     ON action.receipt_id = receipt.receipt_id
                   JOIN storyos.project_activity_event_payloads AS payload
                     ON payload.receipt_id = receipt.receipt_id
                   LEFT JOIN storyos.authoritative_commits AS made
                     ON made.receipt_id = receipt.receipt_id
                   LEFT JOIN storyos.authoritative_heads AS head
                     ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                        (made.owner_user_id, made.project_id, made.manuscript_object_id)
                  WHERE receipt.receipt_id = $1::text::uuid",
                &[receipt_id],
            )
            .await
            .unwrap()
            .get(/*idx*/ 0);
        observed.push(serde_json::from_str::<serde_json::Value>(&json).unwrap());
    }
    // An empty Revision pair, except the genesis Revision of a new Chapter.
    let structural = |tree: [u64; 2], affected: &str, genesis: bool| {
        serde_json::json!({
            "action_binds_commit": true,
            "receipt_binds_commit": true,
            "commit": {
                "tree": tree,
                "affected": format!("{affected}:true"),
                "object": if genesis { "affected chapter" } else { "none" },
                "revision_pair": [false, genesis],
                "head_is_resulting": if genesis { Some(true) } else { None },
            },
        })
    };
    assert_eq!(
        observed,
        vec![
            structural([1, 2], "volume", false),
            structural([2, 3], "chapter", true),
            structural([2, 3], "volume", false),
            structural([3, 4], "chapter", false),
            structural([2, 3], "volume", false),
            structural([3, 4], "chapter", false),
            // A Current Chapter selection binds its Author Action and no Commit.
            serde_json::json!({
                "action_binds_commit": true,
                "receipt_binds_commit": true,
                "commit": null,
            }),
        ]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_structure_receipt_without_a_commit_refuses_a_commit_identity() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut stale = create_volume_call(&store, /*base*/ 0x9790).await;
    stale.input.expected_tree_revision = 2;
    let receipts = [
        settled(&store, &stale).await,
        settled(
            &store,
            &set_current_chapter_call(&store, /*base*/ 0x97a0).await,
        )
        .await,
    ];
    for receipt_id in receipts {
        let error = admin
            .execute(
                "UPDATE storyos.domain_receipts
                    SET authoritative_commit_ids = ARRAY[$2::text::uuid]
                  WHERE receipt_id = $1::text::uuid",
                &[&receipt_id, &"018f0000-0000-7001-8000-0000000097ff"],
            )
            .await
            .expect_err("a structure Receipt without a Commit keeps zero Commit identities");
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
        );
    }
}
