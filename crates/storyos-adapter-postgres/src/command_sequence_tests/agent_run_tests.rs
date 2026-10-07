use storyos_application::{
    AgentRunControlRefusal, PauseAgentRunInput, PauseAgentRunSettlement, ProjectCommandError,
    ProjectScope, RefusableCommandError,
};
use storyos_core::ReceiptResult;
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::update_volume_tests::seed_project;

use super::support::{CommandCall, Route, SequenceError, issued, stores};

pub(super) const PAUSE_AGENT_RUN: Route = Route {
    kind: "pauseAgentRun",
    method: storyos_contracts::PAUSE_AGENT_RUN_METHOD,
    path: storyos_contracts::PAUSE_AGENT_RUN_PATH,
    schema: storyos_contracts::PAUSE_AGENT_RUN_REQUEST_SCHEMA_ID,
};

pub(super) async fn pause_agent_run(
    store: &PostgresProjectReader,
    call: &CommandCall<PauseAgentRunInput>,
) -> Result<PauseAgentRunSettlement, ProjectCommandError> {
    store
        .pause_agent_run(&call.envelope, &call.input)
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

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_refusal_before_admission_writes_no_row_and_keeps_the_challenge_unused() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "b200").await;
    let run_id = Uuid::now_v7().to_string();
    let call = issued(
        &store,
        &scope,
        /*suffix*/ 0xb209,
        &PAUSE_AGENT_RUN,
        PauseAgentRunInput {
            run_id: run_id.clone(),
        },
    )
    .await;
    let refused = store.pause_agent_run(&call.envelope, &call.input).await;
    let key = &call.envelope.challenge_binding.idempotency_key;
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
            &[key],
        )
        .await
        .unwrap();
    seed_run(&admin, &scope, &run_id, "waiting").await;
    let settled = pause_agent_run(&store, &call).await.unwrap();
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
    assert!(matches!(
        refused,
        Err(RefusableCommandError::RefusedBeforeAdmission(
            AgentRunControlRefusal::MissingRun
        ))
    ));
    assert_eq!(
        [0, 1, 2, 3].map(|index| written.get::<_, i64>(index)),
        [0, 0, 0, 1]
    );
    assert_eq!(
        settled.outcome.receipt_result(),
        ReceiptResult::AuthoritativeApplied
    );
    assert_eq!(
        (
            run.get::<_, String>(/*idx*/ 0),
            run.get::<_, i64>(/*idx*/ 1),
            run.get::<_, bool>(/*idx*/ 2),
            run.get::<_, bool>(/*idx*/ 3),
            run.get::<_, String>(/*idx*/ 4),
        ),
        (
            "paused".to_owned(),
            1,
            true,
            false,
            "agent_run_control".to_owned()
        )
    );
}
