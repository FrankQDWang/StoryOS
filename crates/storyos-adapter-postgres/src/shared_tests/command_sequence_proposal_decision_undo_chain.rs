//! The shared test support from `command_sequence_tests/proposal_decision_undo_chain_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_proposal_decision::{REPLAN_PROPOSAL, replan_proposal};
use crate::shared_tests::command_sequence_proposal_undo_chain::undo_call;
use crate::shared_tests::command_sequence_secondary_operation::{
    TwoOperationProposal, proposal_head_revision,
};
use crate::shared_tests::command_sequence_support::{
    issued, run_without_foreign_keys, with_new_request_ids,
};
use crate::shared_tests::command_sequence_undo::{latest_forward, undo};
use storyos_application::{EditorSessionId, ReplanProposalInput, UndoLatestAuthorActionSettlement};
use tokio_postgres::Client;
use uuid::Uuid;

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

/// The resolution and the reservation state of one Proposal Operation.
pub(crate) async fn operation_state(admin: &Client, operation_id: &str) -> (String, String) {
    let row = admin
        .query_one(
            "SELECT resolution, reservation_state FROM storyos.proposal_operations
              WHERE operation_id = $1::text::uuid",
            &[&operation_id],
        )
        .await
        .unwrap();
    (row.get(/*idx*/ 0), row.get(/*idx*/ 1))
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

/// Gives the current head of the Proposal a Proposal Conflict condition, as data from before
/// ADR 0064 has. Then replans it on the same Chapter head. Returns the Replan and the head state
/// before it.
pub(crate) async fn conflict_and_replan(
    store: &PostgresProjectReader,
    admin: &Client,
    proposal: &TwoOperationProposal,
    suffix: u16,
) -> (u64, [String; 5]) {
    let head = proposal_head_revision(admin, &proposal.proposal_id).await;
    let conflict_id = Uuid::now_v7().to_string();
    run_without_foreign_keys(
        admin,
        &format!(
            "UPDATE storyos.proposal_revisions SET validation = 'conflicted'
              WHERE revision_id = '{head}';
             INSERT INTO storyos.proposal_validation_conditions
               (owner_user_id, project_id, proposal_id, proposal_revision_id,
                acceptance_receipt_id, validation, conflict_id, condition_kind)
             VALUES ('{owner}', '{project}', '{proposal}', '{head}', '{receipt}',
                     'conflicted', '{conflict_id}', 'proposal_conflict')",
            owner = proposal.scope.owner_user_id.as_ref(),
            project = proposal.scope.project_id.as_ref(),
            proposal = proposal.proposal_id,
            receipt = Uuid::now_v7(),
        ),
    )
    .await;
    let conflicted = head_state(admin, &proposal.proposal_id).await;
    let input = ReplanProposalInput {
        editor_session_id: EditorSessionId::new(proposal.editor_session_id.clone()),
        proposal_id: proposal.proposal_id.clone(),
        conflicted_proposal_revision_id: head.clone(),
        expected_current_proposal_head: head,
        expected_authoritative_revision_id: proposal.chapter_revision_id.clone(),
        replacement_operation_id: proposal.operations[0].0.clone(),
        source_condition: storyos_contracts::ReplanSourceCondition::ProposalConflict {
            proposal_conflict_ref: conflict_id,
        },
    };
    let call = issued(store, &proposal.scope, suffix, &REPLAN_PROPOSAL, input).await;
    replan_proposal(store, &call).await.unwrap();
    (latest_forward(admin, &proposal.scope).await, conflicted)
}
