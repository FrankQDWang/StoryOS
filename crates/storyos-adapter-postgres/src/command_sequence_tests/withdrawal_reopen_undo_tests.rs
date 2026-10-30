//! Author Undo of an author Withdrawal and of its explicit reopen (ADR 0069). The Undo of a
//! reopen gives back the Withdrawal, so the author can reopen it again. The Undo of an author
//! Withdrawal keeps a later producer Withdrawal.

use storyos_core::{ReopenWithdrawnProposalNoEffect, TransitionOutcome};
use tokio_postgres::Client;

use super::producer_withdrawal_undo::{producer_withdraw, reopen};
use super::proposal_undo_chain::{compensated, head_closure, undo_latest, withdraw};
use super::secondary_operation::two_operation_proposal;
use super::support::stores;
use super::undo::latest_forward;

/// The Withdrawal event of the author Withdrawal of one Proposal at `author_action_sequence`.
async fn author_withdrawal_event(
    admin: &Client,
    proposal_id: &str,
    author_action_sequence: u64,
) -> String {
    admin
        .query_one(
            "SELECT withdrawal_event_id::text FROM storyos.proposal_withdrawals
              WHERE proposal_id = $1::text::uuid
                AND author_action_sequence = $2::text::numeric",
            &[&proposal_id, &author_action_sequence.to_string()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// W, reopen D with W, and Undo D withdraw the Proposal again. A second reopen with W reopens
/// it. The Undo of the second reopen and the Undo of W then open the Proposal.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_author_reopens_a_withdrawal_again_after_the_undo_of_its_reopen() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x9e00).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let withdrawn = withdraw(&store, &admin, &proposal, /*suffix*/ 0x9e09).await;
    let withdrawal = author_withdrawal_event(&admin, &proposal.proposal_id, withdrawn).await;
    let first_reopen = reopen(
        &store,
        &admin,
        &proposal,
        withdrawal.clone(),
        /*suffix*/ 0x9e0a,
    )
    .await;
    let reopened = latest_forward(&admin, &proposal.scope).await;
    let undo_reopen = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x9e0b).await);
    let second_reopen = reopen(
        &store, &admin, &proposal, withdrawal, /*suffix*/ 0x9e0c,
    )
    .await;
    let reopened_again = latest_forward(&admin, &proposal.scope).await;
    let closure_after_reopen = head_closure(&admin, &proposal.proposal_id).await;
    let undo_second_reopen =
        compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x9e0d).await);
    let undo_withdrawal =
        compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x9e0e).await);

    assert_eq!(
        (
            [first_reopen, second_reopen],
            closure_after_reopen,
            [undo_reopen, undo_second_reopen, undo_withdrawal],
            head_closure(&admin, &proposal.proposal_id).await
        ),
        (
            [
                TransitionOutcome::Applied(()),
                TransitionOutcome::Applied(())
            ],
            "open".to_owned(),
            [
                (reopened, Some(withdrawn)),
                (reopened_again, Some(withdrawn)),
                (withdrawn, Some(prior))
            ],
            "open".to_owned()
        )
    );
}

/// W, reopen D with W, producer Withdrawal P of the Revision of D, Undo D, and Undo W keep the
/// Proposal withdrawn. The undone W does not reopen the Proposal, and the reopen with P does.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn author_undo_of_an_author_withdrawal_keeps_a_later_producer_withdrawal() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let proposal = two_operation_proposal(&store, &admin, /*base*/ 0x9e20).await;
    let prior = latest_forward(&admin, &proposal.scope).await;
    let withdrawn = withdraw(&store, &admin, &proposal, /*suffix*/ 0x9e29).await;
    let author_withdrawal = author_withdrawal_event(&admin, &proposal.proposal_id, withdrawn).await;
    let author_reopen = reopen(
        &store,
        &admin,
        &proposal,
        author_withdrawal.clone(),
        /*suffix*/ 0x9e2a,
    )
    .await;
    let reopened = latest_forward(&admin, &proposal.scope).await;
    let producer_withdrawal = producer_withdraw(&store, &admin, &proposal, /*suffix*/ 0x9e2b).await;
    let undo_reopen = compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x9e2c).await);
    let undo_withdrawal =
        compensated(&undo_latest(&store, &admin, &proposal, /*suffix*/ 0x9e2d).await);
    let closure_after_undo = head_closure(&admin, &proposal.proposal_id).await;
    let undone_reopen = reopen(
        &store,
        &admin,
        &proposal,
        author_withdrawal,
        /*suffix*/ 0x9e2e,
    )
    .await;
    let producer_reopen = reopen(
        &store,
        &admin,
        &proposal,
        producer_withdrawal,
        /*suffix*/ 0x9e2f,
    )
    .await;

    assert_eq!(
        (
            author_reopen,
            [undo_reopen, undo_withdrawal],
            closure_after_undo,
            undone_reopen,
            producer_reopen,
            head_closure(&admin, &proposal.proposal_id).await
        ),
        (
            TransitionOutcome::Applied(()),
            [(reopened, Some(withdrawn)), (withdrawn, Some(prior))],
            "withdrawn".to_owned(),
            TransitionOutcome::NoEffect(ReopenWithdrawnProposalNoEffect::WithdrawalEventMismatch),
            TransitionOutcome::Applied(()),
            "open".to_owned()
        )
    );
}
