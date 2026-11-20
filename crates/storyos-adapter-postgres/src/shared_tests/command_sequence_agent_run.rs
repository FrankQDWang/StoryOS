//! The shared test support from `command_sequence_tests/agent_run_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_project_session::{
    UPDATE_PROJECT_ASSISTANCE, UPDATE_PROJECT_INSTRUCTION,
};
use crate::shared_tests::command_sequence_support::{
    CommandCall, Route, SequenceError, issued, seed_project,
};
use crate::shared_tests::set_current_chapter_authority::seed_two_chapters;
use storyos_application::{
    CancelAgentRunInput, CancelAgentRunSettlement, ConversationSelection, CreateAgentRunInput,
    CreateAgentRunSettlement, PauseAgentRunInput, PauseAgentRunSettlement, ProjectCommandError,
    ProjectScope, UpdateProjectAssistanceInput,
};
use storyos_contracts::CommandKind;
use storyos_core::AssistanceAvailability;
use tokio_postgres::Client;
use uuid::Uuid;

pub(crate) const PAUSE_AGENT_RUN: Route = Route {
    kind: CommandKind::PauseAgentRun,
    method: storyos_contracts::PAUSE_AGENT_RUN_METHOD,
    path: storyos_contracts::PAUSE_AGENT_RUN_PATH,
    schema: storyos_contracts::PAUSE_AGENT_RUN_REQUEST_SCHEMA_ID,
};

pub(crate) const CANCEL_AGENT_RUN: Route = Route {
    kind: CommandKind::CancelAgentRun,
    method: storyos_contracts::CANCEL_AGENT_RUN_METHOD,
    path: storyos_contracts::CANCEL_AGENT_RUN_PATH,
    schema: storyos_contracts::CANCEL_AGENT_RUN_REQUEST_SCHEMA_ID,
};

pub(crate) const CREATE_AGENT_RUN: Route = Route {
    kind: CommandKind::CreateAgentRun,
    method: storyos_contracts::CREATE_AGENT_RUN_METHOD,
    path: storyos_contracts::CREATE_AGENT_RUN_PATH,
    schema: storyos_contracts::CREATE_AGENT_RUN_REQUEST_SCHEMA_ID,
};

pub(crate) async fn create_agent_run(
    store: &PostgresProjectReader,
    call: &CommandCall<CreateAgentRunInput>,
) -> Result<CreateAgentRunSettlement, ProjectCommandError> {
    store
        .create_agent_run(&call.envelope, &call.input)
        .await
        .map_err(SequenceError::sequence)
}

/// A createAgentRun of a new conversation on `chapter_id`.
pub(crate) fn run_input(chapter_id: String) -> CreateAgentRunInput {
    CreateAgentRunInput {
        conversation: ConversationSelection::New,
        author_message: "Help with this passage.".to_owned(),
        chapter_id,
        passage_targets: None,
        selection_references: None,
        run_id: Uuid::now_v7().to_string(),
        conversation_id: Uuid::now_v7().to_string(),
        project_agent_id: Uuid::now_v7().to_string(),
        destination: Some(storyos_core::DeploymentDestination::HostFakeToolLoop),
        research: Some(&storyos_core::RESEARCH_TOOL_OFFER),
        embedding: None,
        skills: Default::default(),
        skill_selections: Vec::new(),
    }
}

/// One applicable createAgentRun in a new Project with a current Chapter and available assistance.
pub(crate) async fn create_agent_run_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<CreateAgentRunInput> {
    let suffix = |offset: u16| format!("{:04x}", base + offset);
    let (scope, _volume_id, chapter_id, _chapter_b) = seed_two_chapters(
        store,
        "018f0000-0000-7001-8000-000000000001",
        &suffix(/*offset*/ 0),
        &suffix(/*offset*/ 1),
        &suffix(/*offset*/ 2),
        &suffix(/*offset*/ 3),
    )
    .await;
    let assistance = UpdateProjectAssistanceInput {
        availability: AssistanceAvailability::Available,
        expected_revision: 0,
        destination: Some(storyos_core::DeploymentDestination::HostFakeToolLoop),
    };
    let call = issued(
        store,
        &scope,
        base + 4,
        &UPDATE_PROJECT_ASSISTANCE,
        assistance,
    )
    .await;
    store
        .update_project_assistance(&call.envelope, &call.input)
        .await
        .unwrap();
    // The Run binds this Project Instruction Revision (ADR 0094 section 2).
    let instruction = storyos_application::UpdateProjectInstructionInput {
        text: "Write in the past tense.".to_owned(),
        expected_revision: 0,
    };
    let call = issued(
        store,
        &scope,
        base + 5,
        &UPDATE_PROJECT_INSTRUCTION,
        instruction,
    )
    .await;
    store
        .update_project_instruction(&call.envelope, &call.input)
        .await
        .unwrap();
    issued(
        store,
        &scope,
        base + 9,
        &CREATE_AGENT_RUN,
        run_input(chapter_id),
    )
    .await
}

/// Pauses the queued Run of one createAgentRun, because later HTTP files drain all queued work.
pub(crate) async fn park_run(admin: &Client, call: &CommandCall<CreateAgentRunInput>) {
    admin
        .execute(
            "UPDATE storyos.agent_runs
                SET status = 'paused', lease_expires_at = NULL, wakeup_pending = false
              WHERE run_id = $1::text::uuid",
            &[&call.input.run_id],
        )
        .await
        .unwrap();
}

/// Counts the Admission, Receipt, and settled fence rows of one idempotency key, and its unused
/// Command Challenges.
pub(crate) async fn request_rows(admin: &Client, key: &str) -> [i64; 4] {
    let written = admin
        .query_one(
            "SELECT (SELECT count(*) FROM storyos.author_command_admissions
                      WHERE idempotency_key = $1::text::uuid),
                    (SELECT count(*) FROM storyos.domain_receipts
                      WHERE idempotency_key = $1::text::uuid),
                    (SELECT count(*) FROM storyos.command_idempotency
                      WHERE idempotency_key = $1::text::uuid AND outcome_kind <> 'pending'),
                    (SELECT count(*) FROM storyos.project_command_challenges
                      WHERE idempotency_key = $1::text::uuid AND consumed_at IS NULL)",
            &[&key],
        )
        .await
        .unwrap();
    [0, 1, 2, 3].map(|index| written.get(index))
}

pub(crate) async fn pause_agent_run(
    store: &PostgresProjectReader,
    call: &CommandCall<PauseAgentRunInput>,
) -> Result<PauseAgentRunSettlement, ProjectCommandError> {
    store
        .pause_agent_run(&call.envelope, &call.input)
        .await
        .map_err(SequenceError::sequence)
}

pub(crate) async fn cancel_agent_run(
    store: &PostgresProjectReader,
    call: &CommandCall<CancelAgentRunInput>,
) -> Result<CancelAgentRunSettlement, ProjectCommandError> {
    store
        .cancel_agent_run(&call.envelope, &call.input)
        .await
        .map_err(SequenceError::sequence)
}

/// Inserts one leased AgentRun with `status` and a pending wakeup, without its foreign keys.
///
/// The status must be one that the Worker never claims, because later HTTP files drain all work.
pub(crate) async fn seed_run(admin: &Client, scope: &ProjectScope, run_id: &str, status: &str) {
    admin
        .batch_execute(&format!(
            "BEGIN;
             SET LOCAL session_replication_role = replica;
             INSERT INTO storyos.agent_runs
               (owner_user_id, project_id, run_id, project_agent_id, conversation_id,
                memory_settings_revision, grant_id, project_model_use_binding_revision,
                chapter_id, author_message, status, receipt_id, lease_expires_at,
                wakeup_pending, model_registration_revision, run_kind)
             VALUES ('{owner}', '{project}', '{run_id}', '{agent}', '{conversation}',
                     '{memory}', '{grant}', '{binding}', '{chapter}', 'Continue',
                     '{status}', '{receipt}', now() + interval '1 minute', true,
                     gen_random_uuid(), 'author');
             COMMIT;",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            agent = Uuid::now_v7(),
            conversation = Uuid::now_v7(),
            memory = Uuid::now_v7(),
            grant = Uuid::now_v7(),
            binding = Uuid::now_v7(),
            chapter = Uuid::now_v7(),
            receipt = Uuid::now_v7(),
        ))
        .await
        .unwrap();
}

/// One applicable pauseAgentRun of a waiting AgentRun in a new Project.
pub(crate) async fn pause_agent_run_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<PauseAgentRunInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let run_id = Uuid::now_v7().to_string();
    seed_run(admin, &scope, &run_id, "waiting").await;
    issued(
        store,
        &scope,
        base + 9,
        &PAUSE_AGENT_RUN,
        PauseAgentRunInput { run_id },
    )
    .await
}

/// One applicable cancelAgentRun of a paused AgentRun in a new Project.
pub(crate) async fn cancel_agent_run_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<CancelAgentRunInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let run_id = Uuid::now_v7().to_string();
    seed_run(admin, &scope, &run_id, "paused").await;
    issued(
        store,
        &scope,
        base + 9,
        &CANCEL_AGENT_RUN,
        CancelAgentRunInput { run_id },
    )
    .await
}
