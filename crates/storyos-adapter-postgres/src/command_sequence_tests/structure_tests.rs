use crate::statement_cache::Client;
use storyos_application::{
    ChapterCreated, CreateChapterPublicOrder, CreateVolumePublicOrder, VolumeCreated,
};
use storyos_application::{
    CreateChapterInput, GetManuscriptTree, ProjectCommandError, ProjectScope, UpdateChapterInput,
    UpdateVolumeInput, get_manuscript_tree,
};
use storyos_core::CreateChapterPlacement;

use crate::PostgresProjectReader;
use crate::command_sequence::{CommandSpec, ProjectCommand, settle_project_command};

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_damaged_evidence::{ReplayError, replay_past_checks};
use crate::shared_tests::command_sequence_project_session::update_project_call;
use crate::shared_tests::command_sequence_structure::{
    CREATE_CHAPTER, UPDATE_CHAPTER, UPDATE_VOLUME, create_chapter, create_chapter_call,
    create_volume, create_volume_call, delete_chapter, delete_chapter_call, named_call,
    new_chapter, new_volume, set_current_chapter_call, update_volume_call,
};
use crate::shared_tests::command_sequence_support::{
    CommandCall, SequenceError, applied, durable_project_rows, issued, run_without_foreign_keys,
    seed_project, stores, with_new_request_ids,
};

/// Settles the call and replays it once with its Activity payload set to `changed`, an SQL
/// expression over `payload`.
async fn replay_with_activity_payload<C: ProjectCommand<Error: SequenceError> + Clone>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
    changed: &str,
) -> ReplayError {
    let Ok(_) = settle_project_command(store, &call.envelope, &call.input).await else {
        panic!("the project command must settle");
    };
    let receipt_id = &call.envelope.ids.receipt_id;
    let payload: String = admin
        .query_one(
            "SELECT payload::text FROM storyos.project_activity_event_payloads
              WHERE receipt_id = $1::text::uuid",
            &[receipt_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    replay_past_checks(
        admin,
        store,
        call,
        "project_activity_event_payloads",
        "payload",
        &format!(
            "UPDATE storyos.project_activity_event_payloads SET payload = {changed}
              WHERE receipt_id = '{receipt_id}'"
        ),
        &format!(
            "UPDATE storyos.project_activity_event_payloads SET payload = '{payload}'::jsonb
              WHERE receipt_id = '{receipt_id}'"
        ),
    )
    .await
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_activity_replay_refuses_a_missing_null_or_malformed_required_field() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let observed = vec![
        // A required field that is null or absent.
        replay_with_activity_payload(
            &store,
            &admin,
            &update_project_call(&store, /*base*/ 0xa100).await,
            "jsonb_set(payload, '{title}', 'null'::jsonb)",
        )
        .await,
        replay_with_activity_payload(
            &store,
            &admin,
            &create_volume_call(&store, /*base*/ 0xa110).await,
            "payload - 'volume_id'",
        )
        .await,
        // An identity that is not canonical UUID text.
        replay_with_activity_payload(
            &store,
            &admin,
            &create_chapter_call(&store, /*base*/ 0xa120).await,
            "jsonb_set(payload, '{chapter_id}', '\"not a chapter\"'::jsonb)",
        )
        .await,
        // A number that is not unsigned decimal text.
        replay_with_activity_payload(
            &store,
            &admin,
            &update_volume_call(&store, /*base*/ 0xa130).await,
            "jsonb_set(payload, '{order}', '\"01\"'::jsonb)",
        )
        .await,
        // A nullable field that is absent.
        replay_with_activity_payload(
            &store,
            &admin,
            &delete_chapter_call(&store, /*base*/ 0xa140).await,
            "payload - 'current_chapter_id'",
        )
        .await,
    ];
    assert_eq!(observed, vec![ReplayError::Unavailable; 5]);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_current_chapter_replay_refuses_a_damaged_newest_tree_revision() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let call = set_current_chapter_call(&store, /*base*/ 0xa300).await;
    let Ok(_) = settle_project_command(&store, &call.envelope, &call.input).await else {
        panic!("the Current Chapter selection must settle");
    };
    let newest = admin
        .query_one(
            "SELECT payload.project_activity_position::text,
                    (payload.payload->'tree_revision')::text
               FROM storyos.project_activity_event_payloads AS payload
               JOIN storyos.domain_receipts AS receipt
                 ON (receipt.owner_user_id, receipt.project_id) =
                    (payload.owner_user_id, payload.project_id)
              WHERE receipt.receipt_id = $1::text::uuid AND payload.payload ? 'tree_revision'
              ORDER BY payload.project_activity_position DESC
              LIMIT 1",
            &[&call.envelope.ids.receipt_id],
        )
        .await
        .unwrap();
    let (position, tree_revision): (String, String) =
        (newest.get(/*idx*/ 0), newest.get(/*idx*/ 1));
    let at_newest = format!(
        "WHERE (owner_user_id, project_id, project_activity_position) =
               ('{}', '{}', {position})",
        call.envelope.project_scope.owner_user_id.as_ref(),
        call.envelope.project_scope.project_id.as_ref()
    );
    let observed = replay_past_checks(
        &admin,
        &store,
        &call,
        "project_activity_event_payloads",
        "tree_revision",
        &format!(
            "UPDATE storyos.project_activity_event_payloads
                SET payload = jsonb_set(payload, '{{tree_revision}}', '3'::jsonb) {at_newest}"
        ),
        &format!(
            "UPDATE storyos.project_activity_event_payloads
                SET payload = jsonb_set(payload, '{{tree_revision}}', '{tree_revision}'::jsonb)
                {at_newest}"
        ),
    )
    .await;
    assert_eq!(observed, ReplayError::Unavailable);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_chapter_deletion_without_the_historical_prior_current_field_replays() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let call = delete_chapter_call(&store, /*base*/ 0xa500).await;
    delete_chapter(&store, &call).await.unwrap();
    run_without_foreign_keys(
        &admin,
        &format!(
            "UPDATE storyos.project_activity_event_payloads
                SET payload = payload - 'prior_current_chapter_id'
              WHERE receipt_id = '{}'",
            call.envelope.ids.receipt_id
        ),
    )
    .await;
    let replayed = delete_chapter(&store, &with_new_request_ids(&call)).await;
    assert!(
        replayed.is_ok(),
        "the historical record must replay: {replayed:?}"
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_create_receipt_without_its_order_replays_the_historical_acknowledgement() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let without_order = |receipt_id: &str| {
        format!(
            "UPDATE storyos.domain_receipts SET result_payload = '{{}}'::jsonb
              WHERE receipt_id = '{receipt_id}'"
        )
    };
    let volume = create_volume_call(&store, /*base*/ 0x9600).await;
    let first = create_volume(&store, &volume).await.unwrap();
    run_without_foreign_keys(&admin, &without_order(&volume.envelope.ids.receipt_id)).await;
    let replayed = create_volume(&store, &with_new_request_ids(&volume))
        .await
        .unwrap();
    assert_eq!(
        applied(&replayed).0,
        VolumeCreated {
            order: CreateVolumePublicOrder::HistoricalCreateVolumeAck,
            ..applied(&first).0
        }
    );
    // The second Chapter of its Volume, so the order of the Activity record is 2.
    let earlier = create_chapter_call(&store, /*base*/ 0x9610).await;
    create_chapter(&store, &earlier).await.unwrap();
    let input = CreateChapterInput {
        placement: CreateChapterPlacement::Append,
        volume_id: earlier.input.volume_id.clone(),
        title: "Chapter B".to_owned(),
        expected_tree_revision: 3,
    };
    let scope = earlier.envelope.project_scope.clone();
    let chapter = named_call(&store, &scope, "9618", &CREATE_CHAPTER, input).await;
    let first = create_chapter(&store, &chapter).await.unwrap();
    run_without_foreign_keys(&admin, &without_order(&chapter.envelope.ids.receipt_id)).await;
    let replayed = create_chapter(&store, &with_new_request_ids(&chapter))
        .await
        .unwrap();
    // The historical acknowledgement gives the order of the Activity record.
    assert_eq!(
        applied(&replayed).0,
        ChapterCreated {
            order: CreateChapterPublicOrder::HistoricalCreateChapterAck(2),
            ..applied(&first).0
        }
    );
}

/// The sibling kind of a reorder whose final rank write fails.
#[derive(Clone, Copy, Debug, PartialEq)]
enum RankedSibling {
    Volume,
    Chapter,
}

/// Drops the test trigger that fails a rank write, and its function.
async fn drop_rank_write_failure(admin: &Client) {
    admin
        .batch_execute(
            "DROP TRIGGER IF EXISTS storyos_test_fail_rank_write ON storyos.manuscript_objects;
             DROP FUNCTION IF EXISTS public.storyos_test_fail_rank_write();",
        )
        .await
        .unwrap();
}

/// Installs a test trigger that fails the final rank write of a `sibling` reorder. The failure
/// message gives the new ranks of that write. The temporary storage-key write of a Volume reorder
/// raises each key, and that of a Chapter reorder moves each key above 1000000, so the trigger
/// does not fail them.
async fn install_rank_write_failure(admin: &Client, sibling: RankedSibling) {
    let final_rank = match sibling {
        RankedSibling::Volume => "n.object_kind = 'volume' AND n.tree_order < o.tree_order",
        RankedSibling::Chapter => "n.object_kind = 'chapter' AND n.tree_order <= 1000000",
    };
    drop_rank_write_failure(admin).await;
    admin
        .batch_execute(&format!(
            "CREATE FUNCTION public.storyos_test_fail_rank_write()
             RETURNS trigger LANGUAGE plpgsql AS $$
             DECLARE ranks text;
             BEGIN
               SELECT string_agg(n.tree_order::text, ',' ORDER BY n.tree_order) INTO ranks
                 FROM new_rows n JOIN old_rows o
                USING (owner_user_id, project_id, manuscript_object_id)
                WHERE n.tree_order IS DISTINCT FROM o.tree_order AND {final_rank};
               IF ranks IS NOT NULL THEN
                 RAISE EXCEPTION 'injected rank write failure: %', ranks;
               END IF;
               RETURN NULL;
             END $$;
             CREATE TRIGGER storyos_test_fail_rank_write
             AFTER UPDATE ON storyos.manuscript_objects
             REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows FOR EACH STATEMENT
             EXECUTE FUNCTION public.storyos_test_fail_rank_write();"
        ))
        .await
        .unwrap();
}

/// The state of a Project that a failed reorder must keep.
#[derive(Debug, PartialEq)]
struct KeptState {
    tree: GetManuscriptTree,
    /// The Receipt, Activity payload, Admission, and settled fence rows of the command kind, and
    /// the Commit, Author Action, and Snapshot rows of the Project.
    rows: [i64; 7],
    durable: Vec<serde_json::Value>,
}

async fn kept_state<E>(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    spec: &CommandSpec<E>,
) -> KeptState {
    let row = admin
        .query_one(
            "SELECT (SELECT count(*) FROM storyos.domain_receipts
                      WHERE project_id = $1::text::uuid AND command_kind = $2),
                    (SELECT count(*) FROM storyos.project_activity_event_payloads
                      WHERE project_id = $1::text::uuid AND event_kind = $3),
                    (SELECT count(*) FROM storyos.author_command_admissions
                      WHERE project_id = $1::text::uuid AND command_kind = $2),
                    (SELECT count(*) FROM storyos.command_idempotency
                      WHERE project_id = $1::text::uuid AND command_kind = $2
                        AND outcome_kind = 'settled'),
                    (SELECT count(*) FROM storyos.authoritative_commits
                      WHERE project_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.author_action_entries
                      WHERE project_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.project_snapshots
                      WHERE project_id = $1::text::uuid)",
            &[
                &scope.project_id.as_ref(),
                &spec.kind.as_str(),
                &spec.activity_kind,
            ],
        )
        .await
        .unwrap();
    KeptState {
        tree: get_manuscript_tree(store, scope).await.unwrap(),
        rows: std::array::from_fn(|idx| row.get(idx)),
        durable: durable_project_rows(admin, scope).await,
    }
}

/// Settles the call while a test trigger fails its final rank write.
///
/// Returns the ranks of the failed write when the command returns the injected failure as a store
/// fault, and the kept state before and after the command.
async fn settle_with_failed_rank_write<C: ProjectCommand<Error = ProjectCommandError>>(
    store: &PostgresProjectReader,
    admin: &Client,
    sibling: RankedSibling,
    call: &CommandCall<C>,
) -> (Option<String>, [KeptState; 2]) {
    let scope = &call.envelope.project_scope;
    let before = kept_state(store, admin, scope, &C::SPEC).await;
    install_rank_write_failure(admin, sibling).await;
    let settled = settle_project_command(store, &call.envelope, &call.input).await;
    drop_rank_write_failure(admin).await;
    let failed_ranks = match &settled {
        Err(ProjectCommandError::Unavailable(source)) => source
            .downcast_ref::<tokio_postgres::Error>()
            .and_then(tokio_postgres::Error::as_db_error)
            .and_then(|error| {
                error
                    .message()
                    .strip_prefix("injected rank write failure: ")
            })
            .map(str::to_owned),
        Err(_) | Ok(_) => None,
    };
    (
        failed_ranks,
        [before, kept_state(store, admin, scope, &C::SPEC).await],
    )
}

/// Seeds three siblings of `sibling` in a new Project, and moves the last sibling to the first
/// place, so that the final rank write changes all three ranks. A test trigger fails that write.
async fn failed_reorder(
    store: &PostgresProjectReader,
    admin: &Client,
    sibling: RankedSibling,
    base: u16,
) -> (Option<String>, [KeptState; 2]) {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let volume_id = new_volume(store, &scope, base + 1, /*expected_tree_revision*/ 1).await;
    match sibling {
        RankedSibling::Volume => {
            new_volume(store, &scope, base + 2, /*expected_tree_revision*/ 2).await;
            let moved = new_volume(store, &scope, base + 3, /*expected_tree_revision*/ 3).await;
            let input = UpdateVolumeInput {
                volume_id: moved,
                title: "Moved".to_owned(),
                order: 1,
                expected_tree_revision: 4,
            };
            let call = issued(store, &scope, base + 9, &UPDATE_VOLUME, input).await;
            settle_with_failed_rank_write(store, admin, sibling, &call).await
        }
        RankedSibling::Chapter => {
            for (suffix, expected_tree_revision) in [(base + 2, 2), (base + 3, 3)] {
                new_chapter(store, &scope, suffix, &volume_id, expected_tree_revision).await;
            }
            let moved = new_chapter(
                store,
                &scope,
                base + 4,
                &volume_id,
                /*expected_tree_revision*/ 4,
            )
            .await;
            let input = UpdateChapterInput {
                chapter_id: moved,
                title: "Moved".to_owned(),
                order: 1,
                expected_tree_revision: 5,
            };
            let call = issued(store, &scope, base + 9, &UPDATE_CHAPTER, input).await;
            settle_with_failed_rank_write(store, admin, sibling, &call).await
        }
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_failed_rank_write_rolls_back_the_entire_reorder() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    let mut expected = Vec::new();
    for (sibling, base) in [
        (RankedSibling::Volume, 0x8c40),
        (RankedSibling::Chapter, 0x8c50),
    ] {
        let (failed_ranks, [before, after]) = failed_reorder(&store, &admin, sibling, base).await;
        observed.push((sibling, failed_ranks, after));
        // The final ranks failed. The same stored rows, tree, and tree revision, no row of the
        // command, and no new Commit, Author Action, or Snapshot.
        let [.., commits, actions, snapshots] = before.rows;
        expected.push((
            sibling,
            Some("1,2,3".to_owned()),
            KeptState {
                rows: [0, 0, 0, 0, commits, actions, snapshots],
                ..before
            },
        ));
    }
    assert_eq!(observed, expected);
}
