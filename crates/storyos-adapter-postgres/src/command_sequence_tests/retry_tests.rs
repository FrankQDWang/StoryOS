use storyos_application::TakeOverProjectWriterInput;
use storyos_application::{DeleteVolumeInput, SetCurrentChapterInput, UpdateProjectInput};
use storyos_application::{EditorSessionId, VolumeId};
use storyos_core::ReceiptResult;

use crate::PostgresProjectReader;
use crate::shared_tests::set_current_chapter_authority::open_session;

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_acceptance::accept_proposal_call;
use crate::shared_tests::command_sequence_agent_run::{
    cancel_agent_run_call, create_agent_run_call, pause_agent_run_call,
};
use crate::shared_tests::command_sequence_memory_entry::delete_memory_entry_call;
use crate::shared_tests::command_sequence_project_session::{
    TAKE_OVER_PROJECT_WRITER, UPDATE_PROJECT, take_over_project_writer,
    take_over_project_writer_call, update_project_assistance_call, update_project_call,
    update_project_instruction_call,
};
use crate::shared_tests::command_sequence_proposal_decision::{
    reject_proposal_operations_call, reopen_rejected_operations_call,
    reopen_withdrawn_proposal_call, replan_proposal_call, withdraw_proposal_call,
};
use crate::shared_tests::command_sequence_retry::in_progress_retry;
use crate::shared_tests::command_sequence_steer_agent_run::steer_agent_run_call;
use crate::shared_tests::command_sequence_structure::{
    DELETE_VOLUME, SET_CURRENT_CHAPTER, create_chapter, create_chapter_call, create_volume_call,
    delete_chapter_call, delete_volume, delete_volume_call, new_volume, set_current_chapter_call,
    update_chapter_call, update_volume_call,
};
use crate::shared_tests::command_sequence_support::{
    CommandCall, issued, replayed_outcome, seed_project, stores, two_chapter_writer,
    with_new_request_ids,
};

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_in_progress_exact_retry_conflicts_and_writes_no_row() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let observed = vec![
        in_progress_retry(
            &store,
            &admin,
            &create_volume_call(&store, /*base*/ 0x5e00).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &update_volume_call(&store, /*base*/ 0x5e10).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &delete_volume_call(&store, /*base*/ 0x5e20).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &create_chapter_call(&store, /*base*/ 0x5e30).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &update_chapter_call(&store, /*base*/ 0x5e40).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &delete_chapter_call(&store, /*base*/ 0x5e50).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &update_project_call(&store, /*base*/ 0x5e60).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &set_current_chapter_call(&store, /*base*/ 0x5e80).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &update_project_assistance_call(&store, /*base*/ 0x5e90).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &update_project_instruction_call(&store, /*base*/ 0xef20).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &delete_memory_entry_call(&store, /*base*/ 0xad20).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &take_over_project_writer_call(&store, /*base*/ 0x5ea0).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &reopen_withdrawn_proposal_call(&store, &admin, /*base*/ 0x5eb0).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &reject_proposal_operations_call(&store, &admin, /*base*/ 0x7b20).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &replan_proposal_call(&store, &admin, /*base*/ 0x7240).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &reopen_rejected_operations_call(&store, &admin, /*base*/ 0x7250).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0x7470).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &pause_agent_run_call(&store, &admin, /*base*/ 0xb220).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &cancel_agent_run_call(&store, &admin, /*base*/ 0xd520).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &create_agent_run_call(&store, /*base*/ 0xb330).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &steer_agent_run_call(&store, &admin, /*base*/ 0xc720).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &accept_proposal_call(&store, &admin, /*base*/ 0xa820).await,
        )
        .await,
    ];
    assert_eq!(observed, vec![(true, [0; 5]); 22]);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_exact_retry_ignores_a_later_canonical_snapshot_at_the_same_position() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, _admin) = stores().await;
    let switch = set_current_chapter_call(&store, /*base*/ 0x6ac0).await;
    let scope = switch.envelope.project_scope.clone();
    let switched = store
        .set_current_chapter(&switch.envelope, &switch.input)
        .await
        .unwrap();
    open_session(&store, &scope, "6acb").await;
    let rename_input = UpdateProjectInput {
        title: "Renamed".to_owned(),
        expected_revision: 1,
    };
    let rename = issued(
        &store,
        &scope,
        /*suffix*/ 0x6acc,
        &UPDATE_PROJECT,
        rename_input,
    )
    .await;
    let renamed = store
        .update_project(&rename.envelope, &rename.input)
        .await
        .unwrap();
    open_session(&store, &scope, "6acd").await;
    let switch_retry = with_new_request_ids(&switch);
    let rename_retry = with_new_request_ids(&rename);
    assert_eq!(
        (
            store
                .set_current_chapter(&switch_retry.envelope, &switch_retry.input)
                .await
                .unwrap(),
            store
                .update_project(&rename_retry.envelope, &rename_retry.input)
                .await
                .unwrap(),
        ),
        (switched, renamed)
    );
}

/// The first Editor Session of a Project writes a second canonical Snapshot at the Activity
/// position of the Create Chapter Receipt.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_create_chapter_exact_retry_after_the_first_editor_session_returns_the_first_settlement()
{
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, _admin) = stores().await;
    let call = create_chapter_call(&store, /*base*/ 0xe100).await;
    let first = create_chapter(&store, &call).await.unwrap();
    open_session(&store, &call.envelope.project_scope, "e10a").await;
    assert_eq!(
        create_chapter(&store, &with_new_request_ids(&call))
            .await
            .unwrap(),
        first
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_exact_retry_returns_the_uppercase_client_identities_of_the_first_use() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "a400").await;
    let volume_id = new_volume(
        &store, &scope, /*suffix*/ 0xa401, /*expected_tree_revision*/ 1,
    )
    .await;
    let deletion = DeleteVolumeInput {
        volume_id: VolumeId::new(volume_id.as_ref().to_uppercase()),
        expected_tree_revision: 2,
    };
    let deletion = issued(
        &store,
        &scope,
        /*suffix*/ 0xa409,
        &DELETE_VOLUME,
        deletion,
    )
    .await;
    let (scope, chapter_a, chapter_b, revision_b, editor_session_id) =
        two_chapter_writer(&store, /*base*/ 0xa410).await;
    let selection = SetCurrentChapterInput {
        editor_session_id: EditorSessionId::new(editor_session_id.to_uppercase()),
        chapter_id: chapter_b.to_uppercase(),
        expected_current_chapter_id: chapter_a,
        expected_target_revision_id: revision_b,
    };
    let selection = issued(
        &store,
        &scope,
        /*suffix*/ 0xa419,
        &SET_CURRENT_CHAPTER,
        selection,
    )
    .await;
    let (scope, ..) = two_chapter_writer(&store, /*base*/ 0xa420).await;
    let observer = open_session(&store, &scope, "a425").await;
    let takeover = TakeOverProjectWriterInput {
        editor_session_id: EditorSessionId::new(observer.to_uppercase()),
        observed_writer_generation: 1,
        editor_contract_revision: storyos_contracts::EDITOR_CONTRACT_REVISION.to_owned(),
    };
    let takeover = issued(
        &store,
        &scope,
        /*suffix*/ 0xa429,
        &TAKE_OVER_PROJECT_WRITER,
        takeover,
    )
    .await;
    let observed = vec![
        replayed_outcome(&store, &admin, &deletion, delete_volume)
            .await
            .0,
        replayed_outcome(
            &store,
            &admin,
            &selection,
            async |store: &PostgresProjectReader, call: &CommandCall<SetCurrentChapterInput>| {
                store.set_current_chapter(&call.envelope, &call.input).await
            },
        )
        .await
        .0,
        replayed_outcome(&store, &admin, &takeover, take_over_project_writer)
            .await
            .0,
    ];
    assert_eq!(
        observed,
        vec![
            ReceiptResult::AuthoritativeApplied.code().to_owned(),
            ReceiptResult::AuthoritativeApplied.code().to_owned(),
            ReceiptResult::NoEffect.code().to_owned(),
        ]
    );
}
