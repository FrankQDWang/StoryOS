//! The shared test support from `command_sequence_tests/proposal_decision_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_support::{
    CommandCall, Route, issued, run_without_foreign_keys, two_chapter_writer,
};
use storyos_application::EditorSessionId;
use storyos_application::{
    ProjectCommandError, ProjectScope, RejectProposalOperationsInput,
    RejectProposalOperationsSettlement, RejectionNote, ReopenRejectedOperationsInput,
    ReopenRejectedOperationsSettlement, ReopenWithdrawnProposalInput,
    ReopenWithdrawnProposalSettlement, ReplanProposalInput, ReplanProposalSettlement,
    WithdrawProposalInput, WithdrawProposalSettlement, WithdrawalNote,
};
use storyos_contracts::CommandKind;
use tokio_postgres::Client;
use uuid::Uuid;

pub(crate) const REOPEN_WITHDRAWN_PROPOSAL: Route = Route {
    kind: CommandKind::ReopenWithdrawnProposal,
    method: storyos_contracts::REOPEN_WITHDRAWN_PROPOSAL_METHOD,
    path: storyos_contracts::REOPEN_WITHDRAWN_PROPOSAL_PATH,
    schema: storyos_contracts::REOPEN_WITHDRAWN_PROPOSAL_REQUEST_SCHEMA_ID,
};

pub(crate) const REJECT_PROPOSAL_OPERATIONS: Route = Route {
    kind: CommandKind::RejectProposalOperations,
    method: storyos_contracts::REJECT_PROPOSAL_OPERATIONS_METHOD,
    path: storyos_contracts::REJECT_PROPOSAL_OPERATIONS_PATH,
    schema: storyos_contracts::REJECT_PROPOSAL_OPERATIONS_REQUEST_SCHEMA_ID,
};

pub(crate) const REPLAN_PROPOSAL: Route = Route {
    kind: CommandKind::ReplanProposal,
    method: storyos_contracts::REPLAN_PROPOSAL_METHOD,
    path: storyos_contracts::REPLAN_PROPOSAL_PATH,
    schema: storyos_contracts::REPLAN_PROPOSAL_REQUEST_SCHEMA_ID,
};

pub(crate) const REOPEN_REJECTED_OPERATIONS: Route = Route {
    kind: CommandKind::ReopenRejectedOperations,
    method: storyos_contracts::REOPEN_REJECTED_OPERATIONS_METHOD,
    path: storyos_contracts::REOPEN_REJECTED_OPERATIONS_PATH,
    schema: storyos_contracts::REOPEN_REJECTED_OPERATIONS_REQUEST_SCHEMA_ID,
};

pub(crate) const WITHDRAW_PROPOSAL: Route = Route {
    kind: CommandKind::WithdrawProposal,
    method: storyos_contracts::WITHDRAW_PROPOSAL_METHOD,
    path: storyos_contracts::WITHDRAW_PROPOSAL_PATH,
    schema: storyos_contracts::WITHDRAW_PROPOSAL_REQUEST_SCHEMA_ID,
};

pub(crate) async fn reopen_withdrawn_proposal(
    store: &PostgresProjectReader,
    call: &CommandCall<ReopenWithdrawnProposalInput>,
) -> Result<ReopenWithdrawnProposalSettlement, ProjectCommandError> {
    store
        .reopen_withdrawn_proposal(&call.envelope, &call.input)
        .await
}

/// A new Project with a writer Editor Session and one withdrawn Proposal on Chapter B.
///
/// Returns the Scope and the input that reopens the Proposal. The fixture skips the foreign
/// keys of the AgentRun, the Manuscript Block, and the withdrawal Receipt.
pub(crate) async fn withdrawn_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> (ProjectScope, ReopenWithdrawnProposalInput) {
    let (scope, _chapter_a, chapter_b, revision_b, editor_session_id) =
        two_chapter_writer(store, base).await;
    let input = ReopenWithdrawnProposalInput {
        editor_session_id: EditorSessionId::new(editor_session_id),
        proposal_id: Uuid::now_v7().to_string(),
        proposal_revision_id: Uuid::now_v7().to_string(),
        withdrawal_event_id: Uuid::now_v7().to_string(),
        expected_authoritative_revision_id: revision_b,
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
                     'withdrawn', 'Candidate', '{base_revision}');
             INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}');
             INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             VALUES ('{owner}', '{project}', '{proposal}', '{operation}', '{block}',
                     'pending', 'resolved', 'Candidate');
             INSERT INTO storyos.proposal_withdrawals
               (owner_user_id, project_id, withdrawal_event_id, proposal_id,
                proposal_revision_id, withdrawal_reason, withdrawal_receipt_id)
             VALUES ('{owner}', '{project}', '{event}', '{proposal}', '{revision}',
                     'current_producer_withdrew', '{receipt}');
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
            operation = Uuid::now_v7(),
            event = input.withdrawal_event_id,
            receipt = Uuid::now_v7(),
        ))
        .await
        .unwrap();
    (scope, input)
}

/// One applicable Reopen Withdrawn Proposal in a new Project.
pub(crate) async fn reopen_withdrawn_proposal_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<ReopenWithdrawnProposalInput> {
    let (scope, input) = withdrawn_proposal(store, admin, base).await;
    issued(store, &scope, base + 9, &REOPEN_WITHDRAWN_PROPOSAL, input).await
}

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

pub(crate) async fn replan_proposal(
    store: &PostgresProjectReader,
    call: &CommandCall<ReplanProposalInput>,
) -> Result<ReplanProposalSettlement, ProjectCommandError> {
    store.replan_proposal(&call.envelope, &call.input).await
}

pub(crate) async fn reopen_rejected_operations(
    store: &PostgresProjectReader,
    call: &CommandCall<ReopenRejectedOperationsInput>,
) -> Result<ReopenRejectedOperationsSettlement, ProjectCommandError> {
    store
        .reopen_rejected_operations(&call.envelope, &call.input)
        .await
}

/// The identities of one open Proposal on Chapter B with one Proposal Operation.
pub(crate) struct OpenProposal {
    editor_session_id: String,
    proposal_id: String,
    revision_id: String,
    operation_id: String,
    chapter_head: String,
}

/// A new Project with a writer Editor Session and one open Proposal on Chapter B.
///
/// The fixture skips the foreign keys of the AgentRun and the Manuscript Block.
pub(crate) async fn open_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
    validation: &str,
    resolution: &str,
) -> (ProjectScope, OpenProposal) {
    let (scope, _chapter_a, chapter_b, chapter_head, editor_session_id) =
        two_chapter_writer(store, base).await;
    let proposal = OpenProposal {
        editor_session_id,
        proposal_id: Uuid::now_v7().to_string(),
        revision_id: Uuid::now_v7().to_string(),
        operation_id: Uuid::now_v7().to_string(),
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
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}', 'ready', '{validation}',
                     'open', 'Candidate', '{chapter_head}');
             INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}');
             INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             VALUES ('{owner}', '{project}', '{proposal}', '{operation}', '{block}',
                     '{resolution}', 'resolved', 'Candidate')",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            proposal = proposal.proposal_id,
            block = Uuid::now_v7(),
            run = Uuid::now_v7(),
            decision = Uuid::now_v7(),
            revision = proposal.revision_id,
            operation = proposal.operation_id,
            chapter_head = proposal.chapter_head,
        ),
    )
    .await;
    (scope, proposal)
}

/// A conflicted open Proposal and the input that replans it.
pub(crate) async fn conflicted_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> (ProjectScope, ReplanProposalInput) {
    let (scope, proposal) = open_proposal(store, admin, base, "conflicted", "pending").await;
    let conflict_id = Uuid::now_v7().to_string();
    run_without_foreign_keys(
        admin,
        &format!(
            "INSERT INTO storyos.proposal_validation_conditions
               (owner_user_id, project_id, proposal_id, proposal_revision_id,
                acceptance_receipt_id, validation, conflict_id, condition_kind)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}', '{receipt}',
                     'conflicted', '{conflict_id}', 'proposal_conflict')",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            proposal = proposal.proposal_id,
            revision = proposal.revision_id,
            receipt = Uuid::now_v7(),
        ),
    )
    .await;
    let input = ReplanProposalInput {
        editor_session_id: EditorSessionId::new(proposal.editor_session_id),
        proposal_id: proposal.proposal_id,
        conflicted_proposal_revision_id: proposal.revision_id.clone(),
        expected_current_proposal_head: proposal.revision_id,
        expected_authoritative_revision_id: proposal.chapter_head,
        replacement_operation_id: proposal.operation_id,
        source_condition: storyos_contracts::ReplanSourceCondition::ProposalConflict {
            proposal_conflict_ref: conflict_id,
        },
    };
    (scope, input)
}

/// An open Proposal with one rejected Proposal Operation and the input that reopens it.
pub(crate) async fn rejected_operation(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> (ProjectScope, ReopenRejectedOperationsInput) {
    let (scope, proposal) = open_proposal(store, admin, base, "valid", "rejected").await;
    let rejection_event_id = Uuid::now_v7().to_string();
    run_without_foreign_keys(
        admin,
        &format!(
            "INSERT INTO storyos.proposal_operation_resolutions
               (owner_user_id, project_id, resolution_event_id, proposal_id,
                proposal_revision_id, operation_id, prior_resolution, resulting_resolution,
                rejection_receipt_id, author_action_sequence)
             VALUES ('{owner}', '{project}', '{rejection_event_id}', '{proposal}',
                     '{revision}', '{operation}', 'pending', 'rejected', '{receipt}', 1)",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            proposal = proposal.proposal_id,
            revision = proposal.revision_id,
            operation = proposal.operation_id,
            receipt = Uuid::now_v7(),
        ),
    )
    .await;
    let input = ReopenRejectedOperationsInput {
        editor_session_id: EditorSessionId::new(proposal.editor_session_id),
        proposal_id: proposal.proposal_id,
        proposal_revision_id: proposal.revision_id,
        selected_rejected_operation_id: proposal.operation_id,
        rejection_event_id,
        expected_authoritative_revision_id: proposal.chapter_head,
    };
    (scope, input)
}

/// One applicable Replan Proposal in a new Project.
pub(crate) async fn replan_proposal_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<ReplanProposalInput> {
    let (scope, input) = conflicted_proposal(store, admin, base).await;
    issued(store, &scope, base + 9, &REPLAN_PROPOSAL, input).await
}

/// One applicable Reopen Rejected Operations in a new Project.
pub(crate) async fn reopen_rejected_operations_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<ReopenRejectedOperationsInput> {
    let (scope, input) = rejected_operation(store, admin, base).await;
    issued(store, &scope, base + 9, &REOPEN_REJECTED_OPERATIONS, input).await
}

pub(crate) async fn withdraw_proposal(
    store: &PostgresProjectReader,
    call: &CommandCall<WithdrawProposalInput>,
) -> Result<WithdrawProposalSettlement, ProjectCommandError> {
    store.withdraw_proposal(&call.envelope, &call.input).await
}

/// A new Project with one open Proposal and the input that withdraws it.
pub(crate) async fn withdrawable_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> (ProjectScope, WithdrawProposalInput) {
    let (scope, proposal) = open_proposal(store, admin, base, "valid", "pending").await;
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
