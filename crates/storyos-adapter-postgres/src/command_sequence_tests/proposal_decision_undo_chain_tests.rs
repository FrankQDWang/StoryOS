//! Author Undo of a Proposal decision after the Undo of later Proposal actions (ADR 0068). Each
//! Undo takes the current Author Undo Frontier, as the Web Client does.

use storyos_application::{
    EditorSessionId, RejectProposalOperationsInput, RejectionNote, ReopenRejectedOperationsInput,
    ReopenWithdrawnProposalInput,
};
use storyos_core::{TransitionOutcome, UndoLatestAuthorActionUnavailable};

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_proposal_decision::{
    REJECT_PROPOSAL_OPERATIONS, REOPEN_REJECTED_OPERATIONS, REOPEN_WITHDRAWN_PROPOSAL,
    reject_proposal_operations, reopen_rejected_operations, reopen_withdrawn_proposal,
};
use crate::shared_tests::command_sequence_proposal_decision_undo_chain::{
    conflict_and_replan, head_state, operation_state, undo_and_retry,
};
use crate::shared_tests::command_sequence_proposal_undo_chain::{
    compensated, edit, head_closure, on, undo_latest, withdraw,
};
use crate::shared_tests::command_sequence_secondary_operation::{
    operation_candidates, proposal_head_revision, two_operation_proposal,
};
use crate::shared_tests::command_sequence_support::{issued, stores};
use crate::shared_tests::command_sequence_undo::latest_forward;

/// Replan R, candidate edit E, Undo E, and then Undo R restores the Proposal before R.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_restores_a_replan_after_the_undo_of_a_later_candidate_edit() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4c00).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let candidates = operation_candidates(&admin, &proposal).await;
    let (replanned, before_replan) =
        conflict_and_replan(&store, &admin, &proposal, /*suffix*/ 0x4c09).await;
    let edited = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 0, /*suffix*/ 0x4c0a, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    let undo_edit = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4c0b).await);
    let (undo_replan, retried) = undo_and_retry(&store, &admin, &proposal, /*suffix*/ 0x4c0c).await;

    assert_eq!(
        (
            undo_edit,
            compensated(&undo_replan),
            &retried,
            head_state(&admin, &proposal.proposal_id).await,
            operation_candidates(&admin, &proposal).await
        ),
        (
            (edited, Some(replanned)),
            (replanned, Some(prior)),
            &undo_replan,
            before_replan,
            candidates
        )
    );
}

/// Rejection X, reopen D of the rejected Operation, candidate edit E of that Operation, Undo E,
/// and then Undo D rejects the Operation again. The Undo after it stops at the Barrier X.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_rejects_a_reopened_operation_after_the_undo_of_its_candidate_edit() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4c40).await;
    let candidates = operation_candidates(&admin, &proposal).await;
    let operation_id = proposal.operations[1].0.clone();
    let rejection = issued(
        &store,
        &proposal.scope,
        /*suffix*/ 0x4c49,
        &REJECT_PROPOSAL_OPERATIONS,
        RejectProposalOperationsInput {
            editor_session_id: EditorSessionId::new(proposal.editor_session_id.clone()),
            proposal_id: proposal.proposal_id.clone(),
            proposal_revision_id: proposal_head_revision(&admin, &proposal.proposal_id).await,
            selected_pending_operation_ids: vec![operation_id.clone()],
            expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
            rejection_note: RejectionNote::Omitted,
        },
    )
    .await;
    reject_proposal_operations(&store, &rejection)
        .await
        .unwrap();
    let barrier = latest_forward(&admin, &proposal.scope).await;
    let rejected = operation_state(&admin, &operation_id).await;
    let rejection_event_id: String = admin
        .query_one(
            "SELECT resolution_event_id::text FROM storyos.proposal_operation_resolutions
              WHERE operation_id = $1::text::uuid",
            &[&operation_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let reopen = issued(
        &store,
        &proposal.scope,
        /*suffix*/ 0x4c4a,
        &REOPEN_REJECTED_OPERATIONS,
        ReopenRejectedOperationsInput {
            editor_session_id: EditorSessionId::new(proposal.editor_session_id.clone()),
            proposal_id: proposal.proposal_id.clone(),
            proposal_revision_id: proposal_head_revision(&admin, &proposal.proposal_id).await,
            selected_rejected_operation_id: operation_id.clone(),
            rejection_event_id,
            expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
        },
    )
    .await;
    reopen_rejected_operations(&store, &reopen).await.unwrap();
    let reopened = latest_forward(&admin, &proposal.scope).await;
    let edited = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4c4b, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    let undo_edit = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4c4c).await);
    let (undo_reopen, retried) = undo_and_retry(&store, &admin, &proposal, /*suffix*/ 0x4c4d).await;
    let stopped = undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4c4e)
        .await
        .outcome;

    assert_eq!(
        (
            undo_edit,
            compensated(&undo_reopen),
            &retried,
            stopped,
            operation_state(&admin, &operation_id).await,
            operation_candidates(&admin, &proposal).await
        ),
        (
            (edited, Some(reopened)),
            (reopened, Some(barrier)),
            &undo_reopen,
            TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::Barrier),
            rejected,
            candidates
        )
    );
}

/// Withdrawal W, reopen D, candidate edit E, Undo E, and then Undo D withdraws the Proposal
/// again. The Undo of W after it goes through the Compensation of D and reopens the Proposal.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_withdraws_a_reopened_proposal_after_the_undo_of_a_later_candidate_edit() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4c80).await;
    let candidates = operation_candidates(&admin, &proposal).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let withdrawn = withdraw(&store, &admin, &proposal, /*suffix*/ 0x4c89).await;
    let withdrawal_event_id: String = admin
        .query_one(
            "SELECT withdrawal_event_id::text FROM storyos.proposal_withdrawals
              WHERE proposal_id = $1::text::uuid",
            &[&proposal.proposal_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let reopen = issued(
        &store,
        &proposal.scope,
        /*suffix*/ 0x4c8a,
        &REOPEN_WITHDRAWN_PROPOSAL,
        ReopenWithdrawnProposalInput {
            editor_session_id: EditorSessionId::new(proposal.editor_session_id.clone()),
            proposal_id: proposal.proposal_id.clone(),
            proposal_revision_id: proposal_head_revision(&admin, &proposal.proposal_id).await,
            withdrawal_event_id,
            expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
        },
    )
    .await;
    reopen_withdrawn_proposal(&store, &reopen).await.unwrap();
    let reopened = latest_forward(&admin, &proposal.scope).await;
    let edited = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4c8b, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    let undo_edit = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4c8c).await);
    let (undo_reopen, retried) = undo_and_retry(&store, &admin, &proposal, /*suffix*/ 0x4c8d).await;
    let closure_after_undo = head_closure(&admin, &proposal.proposal_id).await;
    let undo_withdrawal =
        compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4c8e).await);

    assert_eq!(
        (
            [undo_edit, compensated(&undo_reopen), undo_withdrawal],
            &retried,
            [
                closure_after_undo,
                head_closure(&admin, &proposal.proposal_id).await
            ],
            operation_candidates(&admin, &proposal).await
        ),
        (
            [
                (edited, Some(reopened)),
                (reopened, Some(withdrawn)),
                (withdrawn, Some(prior))
            ],
            &undo_reopen,
            ["withdrawn".to_owned(), "open".to_owned()],
            candidates
        )
    );
}

/// The target of a Replan Undo is two Compensations below the head. Undo E appended C1, and the
/// author withdrew C1 in place. Undo W appended C2. Undo R still restores the Proposal.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_restores_a_replan_two_compensations_below_the_head() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4cc0).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let candidates = operation_candidates(&admin, &proposal).await;
    let (replanned, before_replan) =
        conflict_and_replan(&store, &admin, &proposal, /*suffix*/ 0x4cc9).await;
    let edited = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 0, /*suffix*/ 0x4cca, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    let mut steps = vec![compensated(
        &undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4ccb).await,
    )];
    let withdrawn = withdraw(&store, &admin, &proposal, /*suffix*/ 0x4ccc).await;
    for suffix in [0x4ccd, 0x4cce] {
        steps.push(compensated(
            &undo_latest(&store, &admin, &proposal, suffix).await,
        ));
    }

    assert_eq!(
        (
            steps,
            head_state(&admin, &proposal.proposal_id).await,
            operation_candidates(&admin, &proposal).await
        ),
        (
            vec![
                (edited, Some(replanned)),
                (withdrawn, Some(replanned)),
                (replanned, Some(prior))
            ],
            before_replan,
            candidates
        )
    );
}
