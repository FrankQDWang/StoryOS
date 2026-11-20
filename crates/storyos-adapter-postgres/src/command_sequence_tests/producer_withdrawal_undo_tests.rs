//! Author Undo after a Withdrawal by the current producer (ADR 0069). The Undo restores the
//! content and keeps the Proposal `withdrawn`. The explicit reopen with the producer Withdrawal
//! then reopens the restored content.

use storyos_application::{
    EditorSessionId, RejectProposalOperationsInput, RejectionNote, ReopenRejectedOperationsInput,
};
use storyos_core::{TransitionOutcome, UndoLatestAuthorActionUnavailable};

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_producer_withdrawal_undo::{producer_withdraw, reopen};
use crate::shared_tests::command_sequence_proposal_decision::{
    REJECT_PROPOSAL_OPERATIONS, REOPEN_REJECTED_OPERATIONS, reject_proposal_operations,
    reopen_rejected_operations,
};
use crate::shared_tests::command_sequence_proposal_decision_undo_chain::{
    conflict_and_replan, head_state, operation_state, undo_and_retry,
};
use crate::shared_tests::command_sequence_proposal_undo_chain::{
    compensated, edit, head_closure, on, undo_latest,
};
use crate::shared_tests::command_sequence_secondary_operation::{
    operation_candidates, proposal_head_revision, two_operation_proposal,
};
use crate::shared_tests::command_sequence_support::{issued, stores};
use crate::shared_tests::command_sequence_undo::latest_forward;

/// `state` with the closure `closure` and the validation `validation`.
fn with_axes(mut state: [String; 5], validation: &str, closure: &str) -> [String; 5] {
    state[1] = validation.to_owned();
    state[2] = closure.to_owned();
    state
}

/// Candidate edit E1, producer Withdrawal P, and Undo E1 restore the candidate and keep the
/// Proposal withdrawn. The explicit reopen with P then reopens the restored candidate.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_of_a_candidate_edit_keeps_a_producer_withdrawal() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4f00).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let candidates = operation_candidates(&admin, &proposal).await;
    let before_edit = head_state(&admin, &proposal.proposal_id).await;
    let edited = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 0, /*suffix*/ 0x4f09, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    let withdrawal = producer_withdraw(&store, &admin, &proposal, /*suffix*/ 0x4f0a).await;
    let (undo_edit, retried) = undo_and_retry(&store, &admin, &proposal, /*suffix*/ 0x4f0b).await;
    let after_undo = head_state(&admin, &proposal.proposal_id).await;
    let reopened = reopen(
        &store, &admin, &proposal, withdrawal, /*suffix*/ 0x4f0c,
    )
    .await;

    assert_eq!(
        (
            compensated(&undo_edit),
            &retried,
            after_undo,
            reopened,
            head_state(&admin, &proposal.proposal_id).await,
            operation_candidates(&admin, &proposal).await
        ),
        (
            (edited, Some(prior)),
            &undo_edit,
            with_axes(before_edit.clone(), "valid", "withdrawn"),
            TransitionOutcome::Applied(()),
            with_axes(before_edit, "pending", "open"),
            candidates
        )
    );
}

/// E0, E1, P, Undo E1, and Undo E0 keep the Proposal withdrawn at each step. The explicit
/// reopen with P goes through the two kept Compensations.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_of_an_earlier_candidate_edit_keeps_a_producer_withdrawal() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4f30).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let candidates = operation_candidates(&admin, &proposal).await;
    let before_edits = head_state(&admin, &proposal.proposal_id).await;
    let first = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 0, /*suffix*/ 0x4f39, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    let second = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4f3a, /*local_intent_sequence*/ 4,
        ),
    )
    .await;
    let withdrawal = producer_withdraw(&store, &admin, &proposal, /*suffix*/ 0x4f3b).await;
    let undo_second = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4f3c).await);
    let closure_after_second = head_closure(&admin, &proposal.proposal_id).await;
    let undo_first = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4f3d).await);
    let after_undo = head_state(&admin, &proposal.proposal_id).await;
    let reopened = reopen(
        &store, &admin, &proposal, withdrawal, /*suffix*/ 0x4f3e,
    )
    .await;

    assert_eq!(
        (
            [undo_second, undo_first],
            closure_after_second,
            after_undo,
            reopened,
            head_closure(&admin, &proposal.proposal_id).await,
            operation_candidates(&admin, &proposal).await
        ),
        (
            [(second, Some(first)), (first, Some(prior))],
            "withdrawn".to_owned(),
            with_axes(before_edits, "valid", "withdrawn"),
            TransitionOutcome::Applied(()),
            "open".to_owned(),
            candidates
        )
    );
}

/// E0, E1, P, Undo E1, reopen D with P, Undo D, and Undo E0. The Undo of D gives back the kept
/// withdrawn Revision, so Undo E0 keeps the Proposal withdrawn. A second reopen with P goes
/// through the Compensation of D and reopens the Proposal.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_keeps_a_producer_withdrawal_after_the_undo_of_its_reopen() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4f60).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let candidates = operation_candidates(&admin, &proposal).await;
    let first = edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 0, /*suffix*/ 0x4f69, /*local_intent_sequence*/ 3,
        ),
    )
    .await;
    edit(
        &store,
        &admin,
        &proposal,
        on(
            /*operation*/ 1, /*suffix*/ 0x4f6a, /*local_intent_sequence*/ 4,
        ),
    )
    .await;
    let withdrawal = producer_withdraw(&store, &admin, &proposal, /*suffix*/ 0x4f6b).await;
    compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4f6c).await);
    let reopened = reopen(
        &store,
        &admin,
        &proposal,
        withdrawal.clone(),
        /*suffix*/ 0x4f6d,
    )
    .await;
    compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4f6e).await);
    let undo_first = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4f6f).await);
    let closure_after_undo = head_closure(&admin, &proposal.proposal_id).await;
    let second_reopen = reopen(
        &store, &admin, &proposal, withdrawal, /*suffix*/ 0x4f70,
    )
    .await;

    assert_eq!(
        (
            reopened,
            undo_first,
            closure_after_undo,
            second_reopen,
            operation_candidates(&admin, &proposal).await
        ),
        (
            TransitionOutcome::Applied(()),
            (first, Some(prior)),
            "withdrawn".to_owned(),
            TransitionOutcome::Applied(()),
            candidates
        )
    );
}

/// Replan R, P, and Undo R restore the Proposal before R and keep it withdrawn.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_of_a_replan_keeps_a_producer_withdrawal() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4f90).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let candidates = operation_candidates(&admin, &proposal).await;
    let (replanned, before_replan) =
        conflict_and_replan(&store, &admin, &proposal, /*suffix*/ 0x4f99).await;
    let withdrawal = producer_withdraw(&store, &admin, &proposal, /*suffix*/ 0x4f9a).await;
    let (undo_replan, retried) = undo_and_retry(&store, &admin, &proposal, /*suffix*/ 0x4f9b).await;
    let after_undo = head_state(&admin, &proposal.proposal_id).await;
    let reopened = reopen(
        &store, &admin, &proposal, withdrawal, /*suffix*/ 0x4f9c,
    )
    .await;
    let validation = before_replan[1].clone();

    assert_eq!(
        (
            compensated(&undo_replan),
            &retried,
            after_undo,
            reopened,
            operation_candidates(&admin, &proposal).await
        ),
        (
            (replanned, Some(prior)),
            &undo_replan,
            with_axes(before_replan, &validation, "withdrawn"),
            TransitionOutcome::Applied(()),
            candidates
        )
    );
}

/// Rejection X, reopen D of the rejected Operation, P, and Undo D reject the Operation again and
/// keep the Proposal withdrawn. The Undo after it stops at the Barrier X.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_of_a_rejected_operation_reopen_keeps_a_producer_withdrawal() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4fc0).await;
    let operation_id = proposal.operations[1].0.clone();
    let rejection = issued(
        &store,
        &proposal.scope,
        /*suffix*/ 0x4fc9,
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
    let reopen_rejected = issued(
        &store,
        &proposal.scope,
        /*suffix*/ 0x4fca,
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
    reopen_rejected_operations(&store, &reopen_rejected)
        .await
        .unwrap();
    let reopened = latest_forward(&admin, &proposal.scope).await;
    let withdrawal = producer_withdraw(&store, &admin, &proposal, /*suffix*/ 0x4fcb).await;
    let (undo_reopen, retried) = undo_and_retry(&store, &admin, &proposal, /*suffix*/ 0x4fcc).await;
    let closure_after_undo = head_closure(&admin, &proposal.proposal_id).await;
    let stopped = undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4fcd)
        .await
        .outcome;
    let explicit_reopen = reopen(
        &store, &admin, &proposal, withdrawal, /*suffix*/ 0x4fce,
    )
    .await;

    assert_eq!(
        (
            compensated(&undo_reopen),
            &retried,
            closure_after_undo,
            stopped,
            operation_state(&admin, &operation_id).await,
            explicit_reopen
        ),
        (
            (reopened, Some(barrier)),
            &undo_reopen,
            "withdrawn".to_owned(),
            TransitionOutcome::Refused(UndoLatestAuthorActionUnavailable::Barrier),
            rejected,
            TransitionOutcome::Applied(())
        )
    );
}
