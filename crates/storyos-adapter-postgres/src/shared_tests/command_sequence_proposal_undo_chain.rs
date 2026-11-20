//! The shared test support from `command_sequence_tests/proposal_undo_chain_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_proposal_decision::{
    WITHDRAW_PROPOSAL, withdraw_proposal,
};
use crate::shared_tests::command_sequence_secondary_operation::{
    CandidateEdit, TwoOperationProposal, edit_operation_candidate, proposal_head_revision,
};
use crate::shared_tests::command_sequence_support::{CommandCall, issued};
use crate::shared_tests::command_sequence_undo::{issued_undo, latest_forward, undo};
use storyos_application::{
    EditorSessionId, ProjectScope, UndoLatestAuthorActionInput, UndoLatestAuthorActionSettlement,
    WithdrawProposalInput, WithdrawalNote,
};
use storyos_core::TransitionOutcome;
use tokio_postgres::Client;

/// Issues one Author Undo of the current Author Undo Frontier.
pub(crate) async fn undo_call(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    session: &str,
    expected_head: &str,
    suffix: u16,
) -> CommandCall<UndoLatestAuthorActionInput> {
    let input = UndoLatestAuthorActionInput {
        editor_session_id: EditorSessionId::new(session),
        expected_author_undo_frontier_sequence: latest_forward(admin, scope).await,
        expected_authoritative_revision_id: expected_head.to_owned(),
    };
    issued_undo(store, scope, suffix, input).await
}

/// One Author Undo of the current Author Undo Frontier of the Proposal fixture.
pub(crate) async fn undo_latest(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &TwoOperationProposal,
    suffix: u16,
) -> UndoLatestAuthorActionSettlement {
    let call = undo_call(
        store,
        admin,
        &proposal.scope,
        &proposal.editor_session_id,
        &proposal.chapter_revision_id,
        suffix,
    )
    .await;
    undo(store, &call).await.unwrap()
}

/// The compensated source of an applied Author Undo and the Frontier after it.
pub(crate) fn compensated(settlement: &UndoLatestAuthorActionSettlement) -> (u64, Option<u64>) {
    match &settlement.outcome {
        TransitionOutcome::Applied(applied) => (
            applied.source_sequence,
            applied.author_undo_frontier_sequence,
        ),
        other => panic!("the Undo must compensate, got {other:?}"),
    }
}

/// Prepends `text` to the candidate of one Operation and returns the Forward action.
pub(crate) async fn edit(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &TwoOperationProposal,
    edit: CandidateEdit<'_>,
) -> u64 {
    edit_operation_candidate(store, admin, proposal, edit).await;
    latest_forward(admin, &proposal.scope).await
}

/// The closure of the current head of one Proposal.
pub(crate) async fn head_closure(admin: &Client, proposal_id: &str) -> String {
    admin
        .query_one(
            "SELECT revision.closure
               FROM storyos.proposal_heads AS head
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.proposal_id, revision.revision_id) =
                    (head.proposal_id, head.current_revision_id)
              WHERE head.proposal_id = $1::text::uuid",
            &[&proposal_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// The author Withdrawal of the current head of the Proposal fixture.
pub(crate) async fn withdraw(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &TwoOperationProposal,
    suffix: u16,
) -> u64 {
    let input = WithdrawProposalInput {
        editor_session_id: EditorSessionId::new(proposal.editor_session_id.clone()),
        proposal_id: proposal.proposal_id.clone(),
        proposal_revision_id: proposal_head_revision(admin, &proposal.proposal_id).await,
        expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
        withdrawal_note: WithdrawalNote::Omitted,
    };
    let call = issued(store, &proposal.scope, suffix, &WITHDRAW_PROPOSAL, input).await;
    withdraw_proposal(store, &call).await.unwrap();
    latest_forward(admin, &proposal.scope).await
}

pub(crate) const fn on(
    operation: usize,
    suffix: u16,
    local_intent_sequence: u64,
) -> CandidateEdit<'static> {
    CandidateEdit {
        operation,
        suffix,
        local_intent_sequence,
        text: "Edited ",
    }
}
