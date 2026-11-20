//! The shared test support from `command_sequence_tests/producer_withdrawal_undo_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_proposal_decision::{
    REOPEN_WITHDRAWN_PROPOSAL, WITHDRAW_PROPOSAL, reopen_withdrawn_proposal,
};
use crate::shared_tests::command_sequence_secondary_operation::{
    TwoOperationProposal, proposal_head_revision,
};
use crate::shared_tests::command_sequence_support::{challenge_issue, command_call, issued};
use storyos_application::{
    CurrentProducerWithdrawal, EditorSessionId, ReopenWithdrawnProposalInput,
};
use storyos_core::{ReopenWithdrawnProposalNoEffect, TransitionOutcome};
use tokio_postgres::Client;

/// The Withdrawal of the current head by the AgentRun decision that produced the Proposal.
/// Returns its Withdrawal event.
pub(crate) async fn producer_withdraw(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &TwoOperationProposal,
    suffix: u16,
) -> String {
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
    let settlement = store
        .withdraw_proposal_as_current_producer(&call.envelope, &call.input)
        .await
        .unwrap();
    match settlement.outcome {
        TransitionOutcome::Applied(withdrawn) => withdrawn.withdrawal_event_id,
        other => panic!("the producer must withdraw the Proposal, got {other:?}"),
    }
}

/// The explicit reopen of the current head with `withdrawal_event_id`. Returns the outcome
/// without its values.
pub(crate) async fn reopen(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &TwoOperationProposal,
    withdrawal_event_id: String,
    suffix: u16,
) -> TransitionOutcome<(), ReopenWithdrawnProposalNoEffect, String, String> {
    let call = issued(
        store,
        &proposal.scope,
        suffix,
        &REOPEN_WITHDRAWN_PROPOSAL,
        ReopenWithdrawnProposalInput {
            editor_session_id: EditorSessionId::new(proposal.editor_session_id.clone()),
            proposal_id: proposal.proposal_id.clone(),
            proposal_revision_id: proposal_head_revision(admin, &proposal.proposal_id).await,
            withdrawal_event_id,
            expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
        },
    )
    .await;
    match reopen_withdrawn_proposal(store, &call)
        .await
        .unwrap()
        .outcome
    {
        TransitionOutcome::Applied(_) => TransitionOutcome::Applied(()),
        TransitionOutcome::NoEffect(reason) => TransitionOutcome::NoEffect(reason),
        TransitionOutcome::Conflicted(reason) => {
            TransitionOutcome::Conflicted(format!("{reason:?}"))
        }
        TransitionOutcome::Refused(reason) => TransitionOutcome::Refused(format!("{reason:?}")),
    }
}
