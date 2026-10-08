use storyos_application::EditorSessionId;
use storyos_application::{
    ProjectCommandError, ProjectScope, RejectProposalOperationsInput,
    RejectProposalOperationsSettlement, RejectionNote, ReopenRejectedOperationsInput,
    ReopenRejectedOperationsSettlement, ReopenWithdrawnProposalInput,
    ReopenWithdrawnProposalSettlement, ReplanProposalInput, ReplanProposalSettlement,
    WithdrawProposalInput, WithdrawProposalSettlement, WithdrawalNote,
};
use tokio_postgres::Client;
use tokio_postgres::error::SqlState;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_sequence::{ProjectCommand, settle_project_command};

use super::damaged_evidence::{ReplayError, damaged_replay};
use super::draft::compensation_action;
use super::support::{
    CommandCall, Route, SequenceError, issued, replayed_outcome, run_without_foreign_keys, stores,
    two_chapter_writer, with_new_request_ids,
};

pub(super) const REOPEN_WITHDRAWN_PROPOSAL: Route = Route {
    kind: "reopenWithdrawnProposal",
    method: storyos_contracts::REOPEN_WITHDRAWN_PROPOSAL_METHOD,
    path: storyos_contracts::REOPEN_WITHDRAWN_PROPOSAL_PATH,
    schema: storyos_contracts::REOPEN_WITHDRAWN_PROPOSAL_REQUEST_SCHEMA_ID,
};

pub(super) const REJECT_PROPOSAL_OPERATIONS: Route = Route {
    kind: "rejectProposalOperations",
    method: storyos_contracts::REJECT_PROPOSAL_OPERATIONS_METHOD,
    path: storyos_contracts::REJECT_PROPOSAL_OPERATIONS_PATH,
    schema: storyos_contracts::REJECT_PROPOSAL_OPERATIONS_REQUEST_SCHEMA_ID,
};

pub(super) const REPLAN_PROPOSAL: Route = Route {
    kind: "replanProposal",
    method: storyos_contracts::REPLAN_PROPOSAL_METHOD,
    path: storyos_contracts::REPLAN_PROPOSAL_PATH,
    schema: storyos_contracts::REPLAN_PROPOSAL_REQUEST_SCHEMA_ID,
};

pub(super) const REOPEN_REJECTED_OPERATIONS: Route = Route {
    kind: "reopenRejectedOperations",
    method: storyos_contracts::REOPEN_REJECTED_OPERATIONS_METHOD,
    path: storyos_contracts::REOPEN_REJECTED_OPERATIONS_PATH,
    schema: storyos_contracts::REOPEN_REJECTED_OPERATIONS_REQUEST_SCHEMA_ID,
};

pub(super) const WITHDRAW_PROPOSAL: Route = Route {
    kind: "withdrawProposal",
    method: storyos_contracts::WITHDRAW_PROPOSAL_METHOD,
    path: storyos_contracts::WITHDRAW_PROPOSAL_PATH,
    schema: storyos_contracts::WITHDRAW_PROPOSAL_REQUEST_SCHEMA_ID,
};

pub(super) async fn reopen_withdrawn_proposal(
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
pub(super) async fn withdrawn_proposal(
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
pub(super) async fn reopen_withdrawn_proposal_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<ReopenWithdrawnProposalInput> {
    let (scope, input) = withdrawn_proposal(store, admin, base).await;
    issued(store, &scope, base + 9, &REOPEN_WITHDRAWN_PROPOSAL, input).await
}

/// Moves the Chapter head away from `revision`, so that a command that expects it conflicts.
pub(super) async fn move_head_away(admin: &Client, scope: &ProjectScope, revision: &str) {
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

pub(super) async fn reject_proposal_operations(
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
pub(super) async fn pending_proposal(
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
pub(super) async fn reject_proposal_operations_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<RejectProposalOperationsInput> {
    let (scope, input) = pending_proposal(store, admin, base).await;
    issued(store, &scope, base + 9, &REJECT_PROPOSAL_OPERATIONS, input).await
}

pub(super) async fn replan_proposal(
    store: &PostgresProjectReader,
    call: &CommandCall<ReplanProposalInput>,
) -> Result<ReplanProposalSettlement, ProjectCommandError> {
    store.replan_proposal(&call.envelope, &call.input).await
}

pub(super) async fn reopen_rejected_operations(
    store: &PostgresProjectReader,
    call: &CommandCall<ReopenRejectedOperationsInput>,
) -> Result<ReopenRejectedOperationsSettlement, ProjectCommandError> {
    store
        .reopen_rejected_operations(&call.envelope, &call.input)
        .await
}

/// The identities of one open Proposal on Chapter B with one Proposal Operation.
struct OpenProposal {
    editor_session_id: String,
    proposal_id: String,
    revision_id: String,
    operation_id: String,
    chapter_head: String,
}

/// A new Project with a writer Editor Session and one open Proposal on Chapter B.
///
/// The fixture skips the foreign keys of the AgentRun and the Manuscript Block.
async fn open_proposal(
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
pub(super) async fn conflicted_proposal(
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
pub(super) async fn rejected_operation(
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
pub(super) async fn replan_proposal_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<ReplanProposalInput> {
    let (scope, input) = conflicted_proposal(store, admin, base).await;
    issued(store, &scope, base + 9, &REPLAN_PROPOSAL, input).await
}

/// One applicable Reopen Rejected Operations in a new Project.
pub(super) async fn reopen_rejected_operations_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<ReopenRejectedOperationsInput> {
    let (scope, input) = rejected_operation(store, admin, base).await;
    issued(store, &scope, base + 9, &REOPEN_REJECTED_OPERATIONS, input).await
}

pub(super) async fn withdraw_proposal(
    store: &PostgresProjectReader,
    call: &CommandCall<WithdrawProposalInput>,
) -> Result<WithdrawProposalSettlement, ProjectCommandError> {
    store.withdraw_proposal(&call.envelope, &call.input).await
}

/// A new Project with one open Proposal and the input that withdraws it.
pub(super) async fn withdrawable_proposal(
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
pub(super) async fn withdraw_proposal_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<WithdrawProposalInput> {
    let (scope, input) = withdrawable_proposal(store, admin, base).await;
    issued(store, &scope, base + 9, &WITHDRAW_PROPOSAL, input).await
}

/// The effect row of one applied Proposal decision and its preserved Proposal State Axes.
struct PreservedEffect {
    table: &'static str,
    receipt_column: &'static str,
    preserved_columns: &'static [&'static str],
}

/// Settles the call and clears one preserved value, which the all-or-none check refuses. Then
/// replays the call once without its preserved values and once without its effect row.
async fn effect_replays<C: ProjectCommand<Error: SequenceError> + Clone>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
    effect: PreservedEffect,
) -> (Option<SqlState>, [ReplayError; 2]) {
    let Ok(_) = settle_project_command(store, &call.envelope, &call.input).await else {
        panic!("the Proposal decision must settle");
    };
    let PreservedEffect {
        table,
        receipt_column,
        preserved_columns,
    } = effect;
    let filter = format!(
        "WHERE {receipt_column} = '{}'",
        call.envelope.ids.receipt_id
    );
    let cleared = preserved_columns
        .iter()
        .map(|column| format!("{column} = NULL"))
        .collect::<Vec<_>>()
        .join(", ");
    let partial = admin
        .batch_execute(&format!(
            "UPDATE storyos.{table} SET {} = NULL {filter}",
            preserved_columns[0]
        ))
        .await
        .err()
        .and_then(|error| error.code().cloned());
    let mut errors = Vec::new();
    for statement in [
        format!("UPDATE storyos.{table} SET {cleared} {filter}"),
        format!("DELETE FROM storyos.{table} {filter}"),
    ] {
        run_without_foreign_keys(admin, &statement).await;
        let retry = with_new_request_ids(call);
        let Err(error) = settle_project_command(store, &retry.envelope, &retry.input).await else {
            panic!("a replay without its preserved values must fail");
        };
        errors.push(error.sequence().into());
    }
    (partial, errors.try_into().unwrap())
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_proposal_decision_replay_separates_pre_capture_from_damaged_effect_rows() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let observed = vec![
        effect_replays(
            &store,
            &admin,
            &reject_proposal_operations_call(&store, &admin, /*base*/ 0x7c00).await,
            PreservedEffect {
                table: "proposal_rejection_receipts",
                receipt_column: "rejection_receipt_id",
                preserved_columns: &[
                    "preserved_generation",
                    "preserved_validation",
                    "preserved_closure",
                ],
            },
        )
        .await,
        effect_replays(
            &store,
            &admin,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0x7c10).await,
            PreservedEffect {
                table: "proposal_withdrawals",
                receipt_column: "withdrawal_receipt_id",
                preserved_columns: &["preserved_generation", "preserved_validation"],
            },
        )
        .await,
        effect_replays(
            &store,
            &admin,
            &replan_proposal_call(&store, &admin, /*base*/ 0x7c20).await,
            PreservedEffect {
                table: "proposal_replans",
                receipt_column: "replan_receipt_id",
                preserved_columns: &["preserved_generation", "preserved_closure"],
            },
        )
        .await,
        effect_replays(
            &store,
            &admin,
            &reopen_withdrawn_proposal_call(&store, &admin, /*base*/ 0x7c30).await,
            PreservedEffect {
                table: "proposal_withdrawal_reopenings",
                receipt_column: "reopen_receipt_id",
                preserved_columns: &["preserved_generation", "preserved_operation_resolution"],
            },
        )
        .await,
        effect_replays(
            &store,
            &admin,
            &reopen_rejected_operations_call(&store, &admin, /*base*/ 0x7c40).await,
            PreservedEffect {
                table: "proposal_operation_reopenings",
                receipt_column: "reopen_receipt_id",
                preserved_columns: &["preserved_generation", "preserved_closure"],
            },
        )
        .await,
    ];
    let separated = (
        Some(SqlState::CHECK_VIOLATION),
        [
            ReplayError::HistoricalAcknowledgementUnavailable,
            ReplayError::Unavailable,
        ],
    );
    assert_eq!(observed, vec![separated; 5]);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_multi_operation_rejection_replays_the_resolution_of_its_first_selected_operation() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut call = reject_proposal_operations_call(&store, &admin, /*base*/ 0x7c50).await;
    let first_operation = call.input.selected_pending_operation_ids[0].clone();
    let later_operation = Uuid::now_v7().to_string();
    run_without_foreign_keys(
        &admin,
        &format!(
            "INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             SELECT owner_user_id, project_id, proposal_id, '{later_operation}',
                    gen_random_uuid(), resolution, reservation_state, candidate_text
               FROM storyos.proposal_operations WHERE operation_id = '{first_operation}'"
        ),
    )
    .await;
    call.input.selected_pending_operation_ids = vec![later_operation, first_operation];
    let (result_kind, _rows) =
        replayed_outcome(&store, &admin, &call, reject_proposal_operations).await;
    assert_eq!(result_kind, "proposal_operations_resolved");
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_pre_capture_effect_row_does_not_hide_a_damaged_author_action() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let observed = vec![
        damaged_replay(
            &store,
            &admin,
            &reject_proposal_operations_call(&store, &admin, /*base*/ 0xa600).await,
            |receipt_id| {
                format!(
                    "UPDATE storyos.proposal_rejection_receipts
                        SET preserved_generation = NULL, preserved_validation = NULL,
                            preserved_closure = NULL
                      WHERE rejection_receipt_id = '{receipt_id}';
                     DELETE FROM storyos.author_action_entries WHERE receipt_id = '{receipt_id}'"
                )
            },
        )
        .await,
        damaged_replay(
            &store,
            &admin,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0xa610).await,
            |receipt_id| {
                format!(
                    "UPDATE storyos.proposal_withdrawals
                        SET preserved_generation = NULL, preserved_validation = NULL
                      WHERE withdrawal_receipt_id = '{receipt_id}';
                     {}",
                    compensation_action(receipt_id)
                )
            },
        )
        .await,
    ];
    assert_eq!(observed, vec![ReplayError::Unavailable; 2]);
}
