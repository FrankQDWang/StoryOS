//! The shared test support from `command_sequence_tests/proposal_decision_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_support::{
    CommandCall, Route, issued, run_without_foreign_keys, two_chapter_writer,
};
use storyos_application::EditorSessionId;
use storyos_application::{
    ProjectCommandError, ProjectScope, RejectProposalOperationsInput,
    RejectProposalOperationsSettlement, RejectionNote, WithdrawProposalInput,
    WithdrawProposalSettlement, WithdrawalNote,
};
use storyos_contracts::CommandKind;
use tokio_postgres::Client;
use uuid::Uuid;

pub(crate) const REJECT_PROPOSAL_OPERATIONS: Route = Route {
    kind: CommandKind::RejectProposalOperations,
    method: storyos_contracts::REJECT_PROPOSAL_OPERATIONS_METHOD,
    path: storyos_contracts::REJECT_PROPOSAL_OPERATIONS_PATH,
    schema: storyos_contracts::REJECT_PROPOSAL_OPERATIONS_REQUEST_SCHEMA_ID,
};

pub(crate) const WITHDRAW_PROPOSAL: Route = Route {
    kind: CommandKind::WithdrawProposal,
    method: storyos_contracts::WITHDRAW_PROPOSAL_METHOD,
    path: storyos_contracts::WITHDRAW_PROPOSAL_PATH,
    schema: storyos_contracts::WITHDRAW_PROPOSAL_REQUEST_SCHEMA_ID,
};

/// Moves the Chapter head away from `revision`, so that a command that expects it conflicts.
pub(crate) async fn move_head_away(admin: &Client, scope: &ProjectScope, revision: &str) {
    run_without_foreign_keys(
        admin,
        &format!(
            "UPDATE storyos.authoritative_heads AS head
                SET current_revision_id = other.current_revision_id
               FROM storyos.authoritative_heads AS other
              WHERE (head.owner_user_id, head.project_id) = (other.owner_user_id, other.project_id)
                AND head.current_revision_id = '{revision}'
                AND other.current_revision_id <> '{revision}'
                AND head.project_id = '{project}'",
            project = scope.project_id.as_ref(),
        ),
    )
    .await;
}

pub(crate) async fn reject_proposal_operations(
    store: &PostgresProjectReader,
    call: &CommandCall<RejectProposalOperationsInput>,
) -> Result<RejectProposalOperationsSettlement, ProjectCommandError> {
    store
        .reject_proposal_operations(&call.envelope, &call.input)
        .await
}

/// A new Project with a writer Editor Session and one open Proposal with one pending Operation.
///
/// Returns the Scope and the input that rejects the Operation. The fixture skips the foreign
/// keys of the AgentRun and the Manuscript Block.
pub(crate) async fn pending_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> (ProjectScope, RejectProposalOperationsInput) {
    let (scope, _chapter_a, chapter_b, revision_b, editor_session_id) =
        two_chapter_writer(store, base).await;
    let input = RejectProposalOperationsInput {
        editor_session_id: EditorSessionId::new(editor_session_id),
        proposal_id: Uuid::now_v7().to_string(),
        proposal_revision_id: Uuid::now_v7().to_string(),
        selected_pending_operation_ids: vec![Uuid::now_v7().to_string()],
        expected_authoritative_revision_id: revision_b,
        rejection_note: RejectionNote::Present {
            text: "Too formal".to_owned(),
        },
    };
    admin
        .batch_execute(&format!(
            "BEGIN;
             SET LOCAL session_replication_role = replica;
             INSERT INTO storyos.proposals
               (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                source_run_id, source_decision_id)
             VALUES ('{owner}', '{project}', '{proposal}', 'block_edit', '{chapter}',
                     '{block}', '{run}', '{decision}');
             INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}', 'ready', 'valid',
                     'open', 'Candidate', '{base_revision}');
             INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}');
             INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             VALUES ('{owner}', '{project}', '{proposal}', '{operation}', '{block}',
                     'pending', 'unresolved', 'Candidate');
             COMMIT;",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            proposal = input.proposal_id,
            chapter = chapter_b,
            block = Uuid::now_v7(),
            run = Uuid::now_v7(),
            decision = Uuid::now_v7(),
            revision = input.proposal_revision_id,
            base_revision = input.expected_authoritative_revision_id,
            operation = input.selected_pending_operation_ids[0],
        ))
        .await
        .unwrap();
    (scope, input)
}

/// One applicable Reject Proposal Operations in a new Project.
pub(crate) async fn reject_proposal_operations_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<RejectProposalOperationsInput> {
    let (scope, input) = pending_proposal(store, admin, base).await;
    issued(store, &scope, base + 9, &REJECT_PROPOSAL_OPERATIONS, input).await
}

pub(crate) async fn withdraw_proposal(
    store: &PostgresProjectReader,
    call: &CommandCall<WithdrawProposalInput>,
) -> Result<WithdrawProposalSettlement, ProjectCommandError> {
    store.withdraw_proposal(&call.envelope, &call.input).await
}

/// The identities of one open Proposal on Chapter B with one Proposal Operation.
struct OpenProposal {
    editor_session_id: String,
    proposal_id: String,
    revision_id: String,
    chapter_head: String,
}

/// A new Project with a writer Editor Session and one open, valid Proposal on Chapter B with
/// one pending Proposal Operation.
///
/// The fixture skips the foreign keys of the AgentRun and the Manuscript Block.
async fn open_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> (ProjectScope, OpenProposal) {
    let (scope, _chapter_a, chapter_b, chapter_head, editor_session_id) =
        two_chapter_writer(store, base).await;
    let proposal = OpenProposal {
        editor_session_id,
        proposal_id: Uuid::now_v7().to_string(),
        revision_id: Uuid::now_v7().to_string(),
        chapter_head,
    };
    run_without_foreign_keys(
        admin,
        &format!(
            "INSERT INTO storyos.proposals
               (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                source_run_id, source_decision_id)
             VALUES ('{owner}', '{project}', '{proposal}', 'block_edit', '{chapter_b}',
                     '{block}', '{run}', '{decision}');
             INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}', 'ready', 'valid',
                     'open', 'Candidate', '{chapter_head}');
             INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}');
             INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             VALUES ('{owner}', '{project}', '{proposal}', '{operation}', '{block}',
                     'pending', 'resolved', 'Candidate')",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            proposal = proposal.proposal_id,
            block = Uuid::now_v7(),
            run = Uuid::now_v7(),
            decision = Uuid::now_v7(),
            revision = proposal.revision_id,
            operation = Uuid::now_v7(),
            chapter_head = proposal.chapter_head,
        ),
    )
    .await;
    (scope, proposal)
}

/// A new Project with one open Proposal and the input that withdraws it.
pub(crate) async fn withdrawable_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> (ProjectScope, WithdrawProposalInput) {
    let (scope, proposal) = open_proposal(store, admin, base).await;
    let input = WithdrawProposalInput {
        editor_session_id: EditorSessionId::new(proposal.editor_session_id),
        proposal_id: proposal.proposal_id,
        proposal_revision_id: proposal.revision_id,
        expected_authoritative_revision_id: proposal.chapter_head,
        withdrawal_note: WithdrawalNote::Present {
            text: "Not this one".to_owned(),
        },
    };
    (scope, input)
}

/// One applicable author Withdrawal in a new Project.
pub(crate) async fn withdraw_proposal_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<WithdrawProposalInput> {
    let (scope, input) = withdrawable_proposal(store, admin, base).await;
    issued(store, &scope, base + 9, &WITHDRAW_PROPOSAL, input).await
}
