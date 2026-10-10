use storyos_application::EditorSessionId;
use storyos_application::{
    CompleteReadyPartialProposalInput, CompleteReadyPartialProposalSettlement,
    ContinueProposalGenerationInput, ContinueProposalGenerationSettlement,
};
use storyos_application::{ProjectCommandError, ProjectScope};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_sequence::{ProjectCommand, settle_project_command};

use super::damaged_evidence::{ReplayError, damaged_replay, replay_past_checks};
use super::proposal_decision::{
    reject_proposal_operations_call, reopen_rejected_operations_call,
    reopen_withdrawn_proposal_call, replan_proposal_call, withdraw_proposal_call,
};
use super::support::{CommandCall, Route, SequenceError, issued, stores, two_chapter_writer};

pub(super) const COMPLETE_READY_PARTIAL_PROPOSAL: Route = Route {
    kind: "completeReadyPartialProposal",
    method: storyos_contracts::COMPLETE_READY_PARTIAL_PROPOSAL_METHOD,
    path: storyos_contracts::COMPLETE_READY_PARTIAL_PROPOSAL_PATH,
    schema: storyos_contracts::COMPLETE_READY_PARTIAL_PROPOSAL_REQUEST_SCHEMA_ID,
};

pub(super) const CONTINUE_PROPOSAL_GENERATION: Route = Route {
    kind: "continueProposalGeneration",
    method: storyos_contracts::CONTINUE_PROPOSAL_GENERATION_METHOD,
    path: storyos_contracts::CONTINUE_PROPOSAL_GENERATION_PATH,
    schema: storyos_contracts::CONTINUE_PROPOSAL_GENERATION_REQUEST_SCHEMA_ID,
};

pub(super) async fn complete_ready_partial_proposal(
    store: &PostgresProjectReader,
    call: &CommandCall<CompleteReadyPartialProposalInput>,
) -> Result<CompleteReadyPartialProposalSettlement, ProjectCommandError> {
    store
        .complete_ready_partial_proposal(&call.envelope, &call.input)
        .await
}

pub(super) async fn continue_proposal_generation(
    store: &PostgresProjectReader,
    call: &CommandCall<ContinueProposalGenerationInput>,
) -> Result<ContinueProposalGenerationSettlement, ProjectCommandError> {
    store
        .continue_proposal_generation(&call.envelope, &call.input)
        .await
}

/// The applicable completion and continuation of one ready-partial Proposal Generation.
pub(super) struct GenerationFixture {
    pub(super) complete: CompleteReadyPartialProposalInput,
    pub(super) continuation: ContinueProposalGenerationInput,
    /// The head of Chapter A, which is not the head of the Proposal Chapter B.
    pub(super) other_chapter_revision_id: String,
}

/// A new Project with a writer Editor Session and one open ready-partial Proposal on Chapter B.
///
/// The Proposal Generation belongs to a paused AgentRun. The fixture skips the foreign keys of
/// the AgentRun, the Manuscript Block, and the AgentRun Receipt.
pub(super) async fn ready_partial_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> (ProjectScope, GenerationFixture) {
    let (scope, chapter_a, chapter_b, revision_b, editor_session_id) =
        two_chapter_writer(store, base).await;
    let candidate_text = "Candidate";
    let proposal_id = Uuid::now_v7().to_string();
    let revision_id = Uuid::now_v7().to_string();
    let generation_id = Uuid::now_v7().to_string();
    let operation_id = Uuid::now_v7().to_string();
    admin
        .batch_execute(&format!(
            "BEGIN;
             SET LOCAL session_replication_role = replica;
             INSERT INTO storyos.agent_runs
               (owner_user_id, project_id, run_id, project_agent_id, conversation_id,
                memory_settings_revision, grant_id, project_model_use_binding_revision,
                chapter_id, author_message, status, receipt_id, model_registration_revision)
             VALUES ('{owner}', '{project}', '{run}', '{agent}', '{conversation}',
                     '{memory}', '{grant}', '{binding}', '{chapter}', 'Continue',
                     'paused', '{run_receipt}', gen_random_uuid());
             INSERT INTO storyos.proposals
               (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                source_run_id, source_decision_id)
             VALUES ('{owner}', '{project}', '{proposal}', 'block_edit', '{chapter}',
                     '{block}', '{run}', '{decision}');
             INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}', 'ready_partial',
                     'valid', 'open', '{candidate_text}', '{base_revision}');
             INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}');
             INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             VALUES ('{owner}', '{project}', '{proposal}', '{operation}', '{block}',
                     'pending', 'resolved', '{candidate_text}');
             INSERT INTO storyos.proposal_generations
               (owner_user_id, project_id, generation_id, proposal_id, last_applied_stream_seq,
                run_id)
             VALUES ('{owner}', '{project}', '{generation}', '{proposal}', 3, '{run}');
             INSERT INTO storyos.proposal_generation_heads
               (owner_user_id, project_id, proposal_id, generation_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{generation}');
             COMMIT;",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            run = Uuid::now_v7(),
            agent = Uuid::now_v7(),
            conversation = Uuid::now_v7(),
            memory = Uuid::now_v7(),
            grant = Uuid::now_v7(),
            binding = Uuid::now_v7(),
            chapter = chapter_b,
            run_receipt = Uuid::now_v7(),
            proposal = proposal_id,
            block = Uuid::now_v7(),
            decision = Uuid::now_v7(),
            revision = revision_id,
            base_revision = revision_b,
            operation = operation_id,
            generation = generation_id,
        ))
        .await
        .unwrap();
    let other_chapter_revision_id = admin
        .query_one(
            "SELECT current_revision_id::text FROM storyos.authoritative_heads
              WHERE manuscript_object_id = $1::text::uuid",
            &[&chapter_a],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let editor_session_id = EditorSessionId::new(editor_session_id);
    let expected_candidate_digest = storyos_core::hex_sha256(candidate_text.as_bytes());
    let fixture = GenerationFixture {
        complete: CompleteReadyPartialProposalInput {
            editor_session_id: editor_session_id.clone(),
            proposal_id: proposal_id.clone(),
            proposal_revision_id: revision_id.clone(),
            generation_id: generation_id.clone(),
            expected_candidate_digest: expected_candidate_digest.clone(),
            last_applied_stream_seq: 3,
            expected_authoritative_revision_id: revision_b.clone(),
        },
        continuation: ContinueProposalGenerationInput {
            editor_session_id,
            proposal_id,
            proposal_revision_id: revision_id,
            prior_generation_id: generation_id,
            expected_generation_state: "ready_partial".to_owned(),
            expected_candidate_digest,
            selected_pending_operation_ids: vec![operation_id],
            expected_authoritative_revision_id: revision_b,
        },
        other_chapter_revision_id,
    };
    (scope, fixture)
}

/// One applicable Complete Ready Partial Proposal in a new Project.
pub(super) async fn complete_ready_partial_proposal_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<CompleteReadyPartialProposalInput> {
    let (scope, fixture) = ready_partial_proposal(store, admin, base).await;
    issued(
        store,
        &scope,
        base + 9,
        &COMPLETE_READY_PARTIAL_PROPOSAL,
        fixture.complete,
    )
    .await
}

/// One applicable Continue Proposal Generation in a new Project.
pub(super) async fn continue_proposal_generation_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<ContinueProposalGenerationInput> {
    let (scope, fixture) = ready_partial_proposal(store, admin, base).await;
    issued(
        store,
        &scope,
        base + 9,
        &CONTINUE_PROPOSAL_GENERATION,
        fixture.continuation,
    )
    .await
}

fn without_transition_record(receipt_id: &str) -> String {
    format!("DELETE FROM storyos.proposal_generation_transitions WHERE receipt_id = '{receipt_id}'")
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_generation_decision_without_its_transition_record_is_damaged_evidence() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let observed = [
        damaged_replay(
            &store,
            &admin,
            &complete_ready_partial_proposal_call(&store, &admin, /*base*/ 0x7570).await,
            without_transition_record,
        )
        .await,
        damaged_replay(
            &store,
            &admin,
            &continue_proposal_generation_call(&store, &admin, /*base*/ 0x7580).await,
            without_transition_record,
        )
        .await,
    ];
    assert_eq!(
        observed,
        [ReplayError::Unavailable, ReplayError::Unavailable]
    );
}

/// Settles the call and replays it once with an unknown value in one preserved column of its
/// effect row, which `receipt_column` binds to the Receipt.
async fn replay_with_unknown_preserved_value<C: ProjectCommand<Error: SequenceError> + Clone>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
    table: &str,
    receipt_column: &str,
    column: &str,
) -> ReplayError {
    let Ok(_) = settle_project_command(store, &call.envelope, &call.input).await else {
        panic!("the project command must settle");
    };
    let filter = format!(
        "WHERE {receipt_column} = '{}'",
        call.envelope.ids.receipt_id
    );
    let value: String = admin
        .query_one(
            &format!("SELECT {column} FROM storyos.{table} {filter}"),
            &[],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    replay_past_checks(
        admin,
        store,
        call,
        table,
        column,
        &format!("UPDATE storyos.{table} SET {column} = 'damaged' {filter}"),
        &format!("UPDATE storyos.{table} SET {column} = '{value}' {filter}"),
    )
    .await
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_applied_replay_refuses_an_unknown_preserved_proposal_state() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let observed = vec![
        replay_with_unknown_preserved_value(
            &store,
            &admin,
            &reject_proposal_operations_call(&store, &admin, /*base*/ 0x9d00).await,
            "proposal_rejection_receipts",
            "rejection_receipt_id",
            "preserved_closure",
        )
        .await,
        replay_with_unknown_preserved_value(
            &store,
            &admin,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0x9d10).await,
            "proposal_withdrawals",
            "withdrawal_receipt_id",
            "preserved_validation",
        )
        .await,
        replay_with_unknown_preserved_value(
            &store,
            &admin,
            &replan_proposal_call(&store, &admin, /*base*/ 0x9d20).await,
            "proposal_replans",
            "replan_receipt_id",
            "preserved_generation",
        )
        .await,
        replay_with_unknown_preserved_value(
            &store,
            &admin,
            &reopen_withdrawn_proposal_call(&store, &admin, /*base*/ 0x9d30).await,
            "proposal_withdrawal_reopenings",
            "reopen_receipt_id",
            "preserved_operation_resolution",
        )
        .await,
        replay_with_unknown_preserved_value(
            &store,
            &admin,
            &reopen_rejected_operations_call(&store, &admin, /*base*/ 0x9d40).await,
            "proposal_operation_reopenings",
            "reopen_receipt_id",
            "preserved_closure",
        )
        .await,
        replay_with_unknown_preserved_value(
            &store,
            &admin,
            &complete_ready_partial_proposal_call(&store, &admin, /*base*/ 0x9d50).await,
            "proposal_generation_transitions",
            "receipt_id",
            "preserved_validation",
        )
        .await,
        replay_with_unknown_preserved_value(
            &store,
            &admin,
            &continue_proposal_generation_call(&store, &admin, /*base*/ 0x9d60).await,
            "proposal_generation_transitions",
            "receipt_id",
            "preserved_operation_resolution",
        )
        .await,
    ];
    assert_eq!(observed, vec![ReplayError::Unavailable; 7]);
}
