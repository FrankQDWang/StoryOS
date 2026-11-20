//! Author Undo after a Withdrawal by the current producer (ADR 0069). The Undo restores the
//! content and keeps the Proposal `withdrawn`. The producer cannot withdraw a Proposal that the
//! author withdrew (ADR 0098).

use storyos_core::{TransitionOutcome, WithdrawProposalNoEffect};

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_producer_withdrawal_undo::{
    head_state, producer_withdraw, producer_withdrawal, undo_and_retry,
};
use crate::shared_tests::command_sequence_proposal_undo_chain::{
    compensated, edit, head_closure, on, undo_latest, withdraw,
};
use crate::shared_tests::command_sequence_secondary_operation::{
    operation_candidates, two_operation_proposal,
};
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::command_sequence_undo::latest_forward;

/// `state` with the closure `closure` and the validation `validation`.
fn with_axes(mut state: [String; 5], validation: &str, closure: &str) -> [String; 5] {
    state[1] = validation.to_owned();
    state[2] = closure.to_owned();
    state
}

/// Candidate edit E1, producer Withdrawal P, and Undo E1 restore the candidate and keep the
/// Proposal withdrawn.
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
    producer_withdraw(&store, &admin, &proposal, /*suffix*/ 0x4f0a).await;
    let (undo_edit, retried) = undo_and_retry(&store, &admin, &proposal, /*suffix*/ 0x4f0b).await;
    let after_undo = head_state(&admin, &proposal.proposal_id).await;

    assert_eq!(
        (
            compensated(&undo_edit),
            &retried,
            after_undo,
            operation_candidates(&admin, &proposal).await
        ),
        (
            (edited, Some(prior)),
            &undo_edit,
            with_axes(before_edit, "valid", "withdrawn"),
            candidates
        )
    );
}

/// E0, E1, P, Undo E1, and Undo E0 keep the Proposal withdrawn at each step.
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
    producer_withdraw(&store, &admin, &proposal, /*suffix*/ 0x4f3b).await;
    let undo_second = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4f3c).await);
    let closure_after_second = head_closure(&admin, &proposal.proposal_id).await;
    let undo_first = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x4f3d).await);
    let after_undo = head_state(&admin, &proposal.proposal_id).await;

    assert_eq!(
        (
            [undo_second, undo_first],
            closure_after_second,
            after_undo,
            operation_candidates(&admin, &proposal).await
        ),
        (
            [(second, Some(first)), (first, Some(prior))],
            "withdrawn".to_owned(),
            with_axes(before_edits, "valid", "withdrawn"),
            candidates
        )
    );
}

/// The producer cannot withdraw a Proposal that the author withdrew: each Withdrawal needs the
/// closure `open`. Thus no producer Withdrawal comes after an author Withdrawal, and the Undo of
/// the author Withdrawal opens the Proposal (ADR 0098).
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_producer_cannot_withdraw_a_proposal_that_the_author_withdrew() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x4f60).await;
    withdraw(&store, &admin, &proposal, /*suffix*/ 0x4f69).await;
    let producer = producer_withdrawal(&store, &admin, &proposal, /*suffix*/ 0x4f6a).await;
    let producer_withdrawals: i64 = admin
        .query_one(
            "SELECT count(*) FROM storyos.proposal_withdrawals
              WHERE proposal_id = $1::text::uuid
                AND withdrawal_reason = 'current_producer_withdrew'",
            &[&proposal.proposal_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);

    assert_eq!(
        (
            producer.outcome,
            producer_withdrawals,
            head_closure(&admin, &proposal.proposal_id).await,
        ),
        (
            TransitionOutcome::NoEffect(WithdrawProposalNoEffect::ClosureNotOpen),
            0,
            "withdrawn".to_owned(),
        )
    );
}
