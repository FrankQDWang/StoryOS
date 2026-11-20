//! Author Undo of a chain of Proposal candidate edits and withdrawals (ADR 0068). Each Undo
//! takes the current Author Undo Frontier, as the Web Client does.

use storyos_application::{EditorSessionId, RejectProposalOperationsInput, RejectionNote};
use storyos_core::{TransitionOutcome, UndoLatestAuthorActionUnavailable};
use tokio_postgres::Client;

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_proposal_decision::{
    REJECT_PROPOSAL_OPERATIONS, reject_proposal_operations,
};
use crate::shared_tests::command_sequence_proposal_undo_chain::{
    compensated, edit, head_closure, on, undo_call, undo_latest, withdraw,
};
use crate::shared_tests::command_sequence_secondary_operation::{
    operation_candidates, proposal_head, proposal_head_revision, two_operation_proposal,
};
use crate::shared_tests::command_sequence_support::{
    issued, run_without_foreign_keys, stores, with_new_request_ids,
};
use crate::shared_tests::command_sequence_undo::{latest_forward, undo};

/// Undo E2 and then Undo E1 restore the candidate before E1. An exact retry of the second Undo
/// replays its first settlement.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_reverses_two_candidate_edits_and_replays_the_second_undo() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4a00).await;
    let before = operation_candidates(&admin, &proposal).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let first = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4a09, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    let second = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4a0a, /*local_intent_sequence*/ 4,
        ),
    )
    .await;
    let undo_second = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4a0b).await);
    let call = undo_call(
        &store,
        &admin,
        &proposal.scope,
        &proposal.editor_session_id,
        &proposal.chapter_revision_id,
        /*suffix*/ 0x4a0c,
    )
    .await;
    let undo_first = undo(&store, &call).await.unwrap();
    let retried = undo(&store, &with_new_request_ids(&call)).await.unwrap();

    assert_eq!(
        (
            undo_second,
            compensated(&undo_first),
            &retried,
            operation_candidates(&admin, &proposal).await
        ),
        (
            (second, Some(first)),
            (first, Some(prior)),
            &undo_first,
            before
        )
    );
}

/// Three candidate edits of both Operations, undone in reverse order, restore both candidates.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_reverses_three_candidate_edits_in_reverse_order() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4a20).await;
    let before = operation_candidates(&admin, &proposal).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let first = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4a29, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    let second = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 0, /*suffix*/ 0x4a2a, /*local_intent_sequence*/ 4,
        ),
    )
    .await;
    let third = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4a2b, /*local_intent_sequence*/ 5,
        ),
    )
    .await;

    let mut steps = Vec::new();
    for suffix in [0x4a2c, 0x4a2d, 0x4a2e] {
        steps.push(compensated(
            &undo_latest(&store, &admin, &proposal, suffix).await,
        ));
    }

    assert_eq!(
        (steps, operation_candidates(&admin, &proposal).await),
        (
            vec![
                (third, Some(second)),
                (second, Some(first)),
                (first, Some(prior)),
            ],
            before
        )
    );
}

/// The Undo of a Withdrawal moves the Proposal head. The Undo of the earlier candidate edit
/// still restores its candidate.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_reverses_a_candidate_edit_after_the_undo_of_a_later_withdrawal() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4a60).await;
    let before = operation_candidates(&admin, &proposal).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let edited = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4a69, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    let withdrawn = withdraw(&store, &admin, &proposal, /*suffix*/ 0x4a6a).await;

    let mut steps = Vec::new();
    for suffix in [0x4a6b, 0x4a6c] {
        steps.push(compensated(
            &undo_latest(&store, &admin, &proposal, suffix).await,
        ));
    }

    assert_eq!(
        (
            steps,
            operation_candidates(&admin, &proposal).await,
            head_closure(&admin, &proposal.proposal_id).await
        ),
        (
            vec![(withdrawn, Some(edited)), (edited, Some(prior))],
            before,
            "open".to_owned()
        )
    );
}

/// A Barrier between candidate edits stops the chain after the later edits.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_barrier_between_candidate_edits_stops_the_chain() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4a80).await;
    edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4a89, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    let after_first = operation_candidates(&admin, &proposal).await;
    let rejection = issued(
        &store,
        &proposal.scope,
        /*suffix*/ 0x4a8a,
        &REJECT_PROPOSAL_OPERATIONS,
        RejectProposalOperationsInput {
            editor_session_id: EditorSessionId::new(proposal.editor_session_id.clone()),
            proposal_id: proposal.proposal_id.clone(),
            proposal_revision_id: proposal_head(&admin, &proposal.proposal_id).await.0,
            selected_pending_operation_ids: vec![proposal.operations[0].0.clone()],
            expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
            rejection_note: RejectionNote::Omitted,
        },
    )
    .await;
    reject_proposal_operations(&store, &rejection)
        .await
        .unwrap();
    let barrier = latest_forward(&admin, &proposal.scope).await;
    let second = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4a8b, /*local_intent_sequence*/ 4,
        ),
    )
    .await;
    let third = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4a8c, /*local_intent_sequence*/ 5,
        ),
    )
    .await;
    let mut steps = Vec::new();
    for suffix in [0x4a8d, 0x4a8e] {
        steps.push(compensated(
            &undo_latest(&store, &admin, &proposal, suffix).await,
        ));
    }
    let stopped = undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4a8f)
        .await
        .outcome;

    assert_eq!(
        (
            steps,
            stopped,
            operation_candidates(&admin, &proposal).await
        ),
        (
            vec![(third, Some(second)), (second, Some(barrier))],
            TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::Barrier),
            after_first
        )
    );
}

/// A Proposal head that a Compensation with no Receipt payload wrote is not explained by the
/// records. Thus the earlier candidate edit is a Barrier.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_proposal_head_that_the_records_cannot_explain_stops_the_chain() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4aa0).await;
    let first = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4aa9, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    let second = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4aaa, /*local_intent_sequence*/ 4,
        ),
    )
    .await;
    let third = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4aab, /*local_intent_sequence*/ 5,
        ),
    )
    .await;
    let undo_third = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4aac).await);
    let call = undo_call(
        &store,
        &admin,
        &proposal.scope,
        &proposal.editor_session_id,
        &proposal.chapter_revision_id,
        /*suffix*/ 0x4aad,
    )
    .await;
    let undo_second = compensated(&undo(&store, &call).await.unwrap());
    let after_first = operation_candidates(&admin, &proposal).await;
    // An earlier Proposal Compensation recorded no Receipt payload.
    run_without_foreign_keys(
        &admin,
        &format!(
            "UPDATE storyos.domain_receipts SET result_payload = '{{}}'::jsonb
              WHERE receipt_id = '{}'",
            call.envelope.ids.receipt_id
        ),
    )
    .await;
    let stopped = undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4aae)
        .await
        .outcome;

    assert_eq!(
        (
            [undo_third, undo_second],
            stopped,
            operation_candidates(&admin, &proposal).await
        ),
        (
            [(third, Some(second)), (second, Some(first))],
            TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::Barrier),
            after_first
        )
    );
}

/// Runs the Proposal compensation identity check of the baseline on one recorded Undo Receipt.
async fn check_undo_receipt(admin: &Client, receipt_id: &str) -> Result<(), String> {
    let checked = admin
        .batch_execute(&format!(
            "BEGIN;
             CREATE TEMPORARY TABLE receipt_probe
               (owner_user_id uuid, project_id uuid, receipt_id uuid) ON COMMIT DROP;
             CREATE TRIGGER receipt_probe_check AFTER INSERT ON receipt_probe
               FOR EACH ROW EXECUTE FUNCTION storyos.require_proposal_compensation_identity();
             INSERT INTO receipt_probe
               SELECT owner_user_id, project_id, receipt_id FROM storyos.domain_receipts
                WHERE receipt_id = '{receipt_id}';
             COMMIT;"
        ))
        .await;
    if checked.is_err() {
        // A failed statement leaves the transaction open.
        admin.batch_execute("ROLLBACK").await.unwrap();
    }
    checked.map_err(|error| {
        let refusal = error
            .as_db_error()
            .map(|db| (db.code().code(), db.message()));
        format!("{refusal:?}")
    })
}

/// The Receipt trigger accepts the Undo of an earlier candidate edit, whose parent is a later
/// Compensation. It refuses a recorded source that is not the parent of the appended Revision.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_receipt_trigger_accepts_a_chained_parent_and_refuses_another_source() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4ac0).await;
    let unedited = proposal_head_revision(&admin, &proposal.proposal_id).await;
    edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4ac9, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4aca, /*local_intent_sequence*/ 4,
        ),
    )
    .await;
    compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4acb).await);
    let call = undo_call(
        &store,
        &admin,
        &proposal.scope,
        &proposal.editor_session_id,
        &proposal.chapter_revision_id,
        /*suffix*/ 0x4acc,
    )
    .await;
    compensated(&undo(&store, &call).await.unwrap());
    let receipt_id = call.envelope.ids.receipt_id.clone();
    let recorded = check_undo_receipt(&admin, &receipt_id).await;
    // The Revision before the edits is in the same Proposal, but it is not the parent.
    run_without_foreign_keys(
        &admin,
        &format!(
            "UPDATE storyos.domain_receipts
                SET result_payload = jsonb_set(result_payload, '{{source_proposal_revision_id}}',
                                               to_jsonb('{unedited}'::text))
              WHERE receipt_id = '{receipt_id}'"
        ),
    )
    .await;

    assert_eq!(
        (recorded, check_undo_receipt(&admin, &receipt_id).await),
        (
            Ok(()),
            Err(r#"Some(("23514", "Incomplete Proposal compensation identity"))"#.to_owned())
        )
    );
}

/// A withdrawal of a head that a Compensation wrote. Each Undo of the chain E1, E2, Undo E2,
/// W, Undo W, Undo E1 compensates.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_reopens_a_withdrawal_of_a_compensation_head_and_continues() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4ae0).await;
    let before = operation_candidates(&admin, &proposal).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let first = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4ae9, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    let second = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4aea, /*local_intent_sequence*/ 4,
        ),
    )
    .await;
    let undo_second = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4aeb).await);
    let withdrawn = withdraw(&store, &admin, &proposal, /*suffix*/ 0x4aec).await;

    let mut steps = vec![undo_second];
    for suffix in [0x4aee, 0x4aef] {
        steps.push(compensated(
            &undo_latest(&store, &admin, &proposal, suffix).await,
        ));
    }

    assert_eq!(
        (
            steps,
            operation_candidates(&admin, &proposal).await,
            head_closure(&admin, &proposal.proposal_id).await
        ),
        (
            vec![
                (second, Some(first)),
                (withdrawn, Some(first)),
                (first, Some(prior)),
            ],
            before,
            "open".to_owned()
        )
    );
}
