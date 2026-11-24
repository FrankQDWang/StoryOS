use crate::statement_cache::Client;
use storyos_application::{
    AgentRunControlRefusal, CancelAgentRunInput, PauseAgentRunInput, RefusableCommandError,
};
use storyos_core::{CreateAgentRunRefusal, ReceiptResult};
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_sequence::{ProjectCommand, settle_project_command};

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_agent_run::{
    CANCEL_AGENT_RUN, CREATE_AGENT_RUN, PAUSE_AGENT_RUN, create_agent_run_call, request_rows,
    run_input, seed_run,
};
use crate::shared_tests::command_sequence_damaged_evidence::{ReplayError, damaged_replay};
use crate::shared_tests::command_sequence_support::{
    Route, SequenceError, issued, seed_project, stores,
};

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
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
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
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
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
    // A deployment with no Skill refuses each selection before Admission (ADR 0089 section 7).
    let mut selecting = create_agent_run_call(&store, /*base*/ 0x4600).await;
    selecting.input.skill_selections = vec![storyos_core::SkillPackageReference {
        name: "internal-comms".to_owned(),
        package_digest: format!("sha256:{}", "3c".repeat(32)),
    }];
    let selection_refused = store
        .create_agent_run(&selecting.envelope, &selecting.input)
        .await;
    let call_without_run = create_agent_run_call(&store, /*base*/ 0xb310).await;
    let missing_run = damaged_replay(&store, &admin, &call_without_run, |receipt_id| {
        format!("DELETE FROM storyos.agent_runs WHERE receipt_id = '{receipt_id}'::uuid")
    })
    .await;
    assert!(matches!(
        refused,
        Err(RefusableCommandError::RefusedBeforeAdmission(
            CreateAgentRunRefusal::InvalidChapterJoin
        ))
    ));
    assert_eq!(request_rows(&admin, key).await, [0, 0, 0, 1]);
    assert!(matches!(
        selection_refused,
        Err(RefusableCommandError::RefusedBeforeAdmission(
            CreateAgentRunRefusal::SkillSelectionUnavailable
        ))
    ));
    let selecting_key = &selecting.envelope.challenge_binding.idempotency_key;
    assert_eq!(request_rows(&admin, selecting_key).await, [0, 0, 0, 1]);
    assert_eq!(missing_run, ReplayError::Unavailable);
}
