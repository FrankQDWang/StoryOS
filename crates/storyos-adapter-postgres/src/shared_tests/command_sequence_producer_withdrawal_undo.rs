//! The shared test support from `command_sequence_tests/producer_withdrawal_undo_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_proposal_decision::WITHDRAW_PROPOSAL;
use crate::shared_tests::command_sequence_proposal_undo_chain::undo_call;
use crate::shared_tests::command_sequence_secondary_operation::{
    TwoOperationProposal, proposal_head_revision,
};
use crate::shared_tests::command_sequence_support::{
    challenge_issue, command_call, with_new_request_ids,
};
use crate::shared_tests::command_sequence_undo::undo;
use storyos_application::{
    CurrentProducerWithdrawal, CurrentProducerWithdrawalSettlement,
    UndoLatestAuthorActionSettlement,
};
use storyos_core::TransitionOutcome;
use tokio_postgres::Client;

/// The Withdrawal of the current head by the AgentRun decision that produced the Proposal.
/// Returns its Withdrawal event.
pub(crate) async fn producer_withdraw(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &TwoOperationProposal,
    suffix: u16,
) -> String {
    match producer_withdrawal(store, admin, proposal, suffix)
        .await
        .outcome
    {
        TransitionOutcome::Applied(withdrawn) => withdrawn.withdrawal_event_id,
        other => panic!("the producer must withdraw the Proposal, got {other:?}"),
    }
}

/// The settlement of a Withdrawal of the current head by the AgentRun decision that produced
/// the Proposal.
pub(crate) async fn producer_withdrawal(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &TwoOperationProposal,
    suffix: u16,
) -> CurrentProducerWithdrawalSettlement {
    let producer = admin
        .query_one(
            "SELECT source_run_id::text, source_decision_id::text FROM storyos.proposals
              WHERE proposal_id = $1::text::uuid",
            &[&proposal.proposal_id],
        )
        .await
        .unwrap();
    let withdrawal = CurrentProducerWithdrawal {
        run_id: producer.get(/*idx*/ 0),
        decision_id: producer.get(/*idx*/ 1),
        proposal_id: proposal.proposal_id.clone(),
        proposal_revision_id: proposal_head_revision(admin, &proposal.proposal_id).await,
        expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
    };
    // The producer form takes no Command Challenge, so the call issues none. Each issued
    // Challenge counts toward the rate limit of the Project.
    let issue = challenge_issue(
        &proposal.scope,
        suffix,
        &WITHDRAW_PROPOSAL,
        format!("sha256:producer-withdrawal:{suffix:04x}"),
    );
    let call = command_call(
        issue.binding,
        &issue.nonce_digest,
        &format!("{suffix:04x}"),
        b"{}",
        withdrawal,
    );
    store
        .withdraw_proposal_as_current_producer(&call.envelope, &call.input)
        .await
        .unwrap()
}

/// The state axes, the candidate, and the base of the current head of one Proposal.
pub(crate) async fn head_state(admin: &Client, proposal_id: &str) -> [String; 5] {
    let row = admin
        .query_one(
            "SELECT revision.generation, revision.validation, revision.closure,
                    revision.candidate_text, revision.base_authoritative_revision_id::text
               FROM storyos.proposal_heads AS head
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.proposal_id, revision.revision_id) =
                    (head.proposal_id, head.current_revision_id)
              WHERE head.proposal_id = $1::text::uuid",
            &[&proposal_id],
        )
        .await
        .unwrap();
    std::array::from_fn(|idx| row.get(idx))
}

/// Undo of the current Author Undo Frontier and an exact retry of the same Undo.
pub(crate) async fn undo_and_retry(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &TwoOperationProposal,
    suffix: u16,
) -> (
    UndoLatestAuthorActionSettlement,
    UndoLatestAuthorActionSettlement,
) {
    let call = undo_call(
        store,
        admin,
        &proposal.scope,
        &proposal.editor_session_id,
        &proposal.chapter_revision_id,
        suffix,
    )
    .await;
    let settled = undo(store, &call).await.unwrap();
    let retried = undo(store, &with_new_request_ids(&call)).await.unwrap();
    (settled, retried)
}
