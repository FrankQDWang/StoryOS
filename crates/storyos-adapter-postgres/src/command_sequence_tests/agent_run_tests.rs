use storyos_application::{
    AgentRunControlRefusal, CancelAgentRunInput, CancelAgentRunSettlement, ConversationSelection,
    CreateAgentRunInput, CreateAgentRunSettlement, PauseAgentRunInput, PauseAgentRunSettlement,
    ProjectCommandError, ProjectScope, RefusableCommandError, UpdateProjectAssistanceInput,
};
use storyos_core::{AssistanceAvailability, CreateAgentRunRefusal, ReceiptResult};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_sequence::{ProjectCommand, settle_project_command};
use crate::set_current_chapter_authority_tests::seed_two_chapters;
use crate::update_volume_tests::seed_project;

use super::damaged_evidence::{ReplayError, damaged_replay};
use super::project_session::UPDATE_PROJECT_ASSISTANCE;
use super::support::{CommandCall, Route, SequenceError, issued, stores};

pub(super) const PAUSE_AGENT_RUN: Route = Route {
    kind: "pauseAgentRun",
    method: storyos_contracts::PAUSE_AGENT_RUN_METHOD,
    path: storyos_contracts::PAUSE_AGENT_RUN_PATH,
    schema: storyos_contracts::PAUSE_AGENT_RUN_REQUEST_SCHEMA_ID,
};

pub(super) const CANCEL_AGENT_RUN: Route = Route {
    kind: "cancelAgentRun",
    method: storyos_contracts::CANCEL_AGENT_RUN_METHOD,
    path: storyos_contracts::CANCEL_AGENT_RUN_PATH,
    schema: storyos_contracts::CANCEL_AGENT_RUN_REQUEST_SCHEMA_ID,
};

pub(super) const CREATE_AGENT_RUN: Route = Route {
    kind: "createAgentRun",
    method: storyos_contracts::CREATE_AGENT_RUN_METHOD,
    path: storyos_contracts::CREATE_AGENT_RUN_PATH,
    schema: storyos_contracts::CREATE_AGENT_RUN_REQUEST_SCHEMA_ID,
};

pub(super) async fn create_agent_run(
    store: &PostgresProjectReader,
    call: &CommandCall<CreateAgentRunInput>,
) -> Result<CreateAgentRunSettlement, ProjectCommandError> {
    store
        .create_agent_run(&call.envelope, &call.input)
        .await
        .map_err(SequenceError::sequence)
}

/// A createAgentRun of a new conversation on `chapter_id`.
fn run_input(chapter_id: String) -> CreateAgentRunInput {
    CreateAgentRunInput {
        conversation: ConversationSelection::New,
        author_message: "Help with this passage.".to_owned(),
        chapter_id,
        passage_targets: None,
        candidate_target: None,
        run_id: Uuid::now_v7().to_string(),
        conversation_id: Uuid::now_v7().to_string(),
        project_agent_id: Uuid::now_v7().to_string(),
    }
}

/// One applicable createAgentRun in a new Project with a current Chapter and available assistance.
pub(super) async fn create_agent_run_call(
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
pub(super) async fn park_run(admin: &Client, call: &CommandCall<CreateAgentRunInput>) {
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
async fn request_rows(admin: &Client, key: &str) -> [i64; 4] {
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

pub(super) async fn pause_agent_run(
    store: &PostgresProjectReader,
    call: &CommandCall<PauseAgentRunInput>,
) -> Result<PauseAgentRunSettlement, ProjectCommandError> {
    store
        .pause_agent_run(&call.envelope, &call.input)
        .await
        .map_err(SequenceError::sequence)
}

pub(super) async fn cancel_agent_run(
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
pub(super) async fn seed_run(admin: &Client, scope: &ProjectScope, run_id: &str, status: &str) {
    admin
        .batch_execute(&format!(
            "BEGIN;
             SET LOCAL session_replication_role = replica;
             INSERT INTO storyos.agent_runs
               (owner_user_id, project_id, run_id, project_agent_id, conversation_id,
                memory_settings_revision, grant_id, project_model_use_binding_revision,
                chapter_id, author_message, status, receipt_id, lease_expires_at,
                wakeup_pending)
             VALUES ('{owner}', '{project}', '{run_id}', '{agent}', '{conversation}',
                     '{memory}', '{grant}', '{binding}', '{chapter}', 'Continue',
                     '{status}', '{receipt}', now() + interval '1 minute', true);
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
pub(super) async fn pause_agent_run_call(
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
pub(super) async fn cancel_agent_run_call(
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

/// The rows of a refusal before Admission, and then the Run and the Admission after the same
/// call settles against a waiting AgentRun.
#[derive(Debug, PartialEq)]
struct RefusedThenSettled {
    refused_rows: [i64; 4],
    settled: ReceiptResult,
    run: (String, i64, bool, bool, String),
}

/// Settles `input` of `route` for a missing AgentRun, then seeds that AgentRun and settles the
/// same call again.
async fn refused_then_settled<C>(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
    route: &Route,
    input: impl FnOnce(String) -> C,
) -> RefusedThenSettled
where
    C: ProjectCommand<Error = RefusableCommandError<AgentRunControlRefusal>>,
{
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let run_id = Uuid::now_v7().to_string();
    let call = issued(store, &scope, base + 9, route, input(run_id.clone())).await;
    let refused = settle_project_command(store, &call.envelope, &call.input).await;
    assert!(matches!(
        refused,
        Err(RefusableCommandError::RefusedBeforeAdmission(
            AgentRunControlRefusal::MissingRun
        ))
    ));
    let key = &call.envelope.challenge_binding.idempotency_key;
    let written = request_rows(admin, key).await;
    seed_run(admin, &scope, &run_id, "waiting").await;
    let settled = settle_project_command(store, &call.envelope, &call.input)
        .await
        .map_err(SequenceError::sequence)
        .unwrap();
    let run = admin
        .query_one(
            "SELECT run.status, run.fence_token, run.lease_expires_at IS NULL, run.wakeup_pending,
                    admission.action_class
               FROM storyos.agent_runs AS run, storyos.author_command_admissions AS admission
              WHERE run.run_id = $1::text::uuid AND admission.idempotency_key = $2::text::uuid",
            &[&run_id, key],
        )
        .await
        .unwrap();
    RefusedThenSettled {
        refused_rows: written,
        settled: settled.outcome.receipt_result(),
        run: (
            run.get(/*idx*/ 0),
            run.get(/*idx*/ 1),
            run.get(/*idx*/ 2),
            run.get(/*idx*/ 3),
            run.get(/*idx*/ 4),
        ),
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_refusal_before_admission_writes_no_row_and_keeps_the_challenge_unused() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let observed = vec![
        refused_then_settled(
            &store,
            &admin,
            /*base*/ 0xb200,
            &PAUSE_AGENT_RUN,
            |run_id| PauseAgentRunInput { run_id },
        )
        .await,
        refused_then_settled(
            &store,
            &admin,
            /*base*/ 0xd500,
            &CANCEL_AGENT_RUN,
            |run_id| CancelAgentRunInput { run_id },
        )
        .await,
    ];
    // Without an in-flight Model Attempt, cancellation clears the wakeup.
    let settled = |status: &str| RefusedThenSettled {
        refused_rows: [0, 0, 0, 1],
        settled: ReceiptResult::AuthoritativeApplied,
        run: (
            status.to_owned(),
            1,
            true,
            false,
            "agent_run_control".to_owned(),
        ),
    };
    assert_eq!(observed, vec![settled("paused"), settled("cancelled")]);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_create_agent_run_refusal_writes_no_row_and_a_missing_run_is_damaged() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "b300").await;
    let call = issued(
        &store,
        &scope,
        /*suffix*/ 0xb309,
        &CREATE_AGENT_RUN,
        run_input(Uuid::now_v7().to_string()),
    )
    .await;
    let refused = store.create_agent_run(&call.envelope, &call.input).await;
    let key = &call.envelope.challenge_binding.idempotency_key;
    let call_without_run = create_agent_run_call(&store, /*base*/ 0xb310).await;
    let missing_run = damaged_replay(&store, &admin, &call_without_run, |receipt_id| {
        format!("DELETE FROM storyos.agent_runs WHERE receipt_id = '{receipt_id}'::uuid")
    })
    .await;
    assert!(matches!(
        refused,
        Err(RefusableCommandError::RefusedBeforeAdmission(
            CreateAgentRunRefusal::AssistanceUnavailable
        ))
    ));
    assert_eq!(request_rows(&admin, key).await, [0, 0, 0, 1]);
    assert_eq!(missing_run, ReplayError::Unavailable);
}
