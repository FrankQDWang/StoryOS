//! The cancellation of the parent Run while its Subrun works, at the dispatch seam with the fake
//! destination (ADR 0103 section 3A). Test prefix lead `76`.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::{FakeDestination, START_SUBRUN_SCRIPT};
use storyos_application::{
    CancelAgentRunInput, CompleteAgentRunError, ContractFaultObserver, ContractFaultPoint,
    complete_agent_run,
};
use tokio_postgres::Client;

use super::tests::{records, scripted, subrun_calls, subruns};
use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_steering::cancellation;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::{CommandCall, stores};
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::model_gateway::{RuntimeClock, claim_run};

/// The Contract Fault Point observer that cancels the parent Run at the second dispatch claim:
/// the first dispatch claim of its Subrun.
struct CancelAtSubrun<'a> {
    store: &'a PostgresProjectReader,
    cancel: CommandCall<CancelAgentRunInput>,
    reached: std::sync::Mutex<usize>,
}

impl ContractFaultObserver for CancelAtSubrun<'_> {
    async fn reached(&self, point: ContractFaultPoint) {
        if point != ContractFaultPoint::DispatchClaimed {
            return;
        }
        let count = {
            let mut reached = self.reached.lock().unwrap();
            *reached += 1;
            *reached
        };
        if count != 2 {
            return;
        }
        let (envelope, input) = (&self.cancel.envelope, &self.cancel.input);
        self.store.cancel_agent_run(envelope, input).await.unwrap();
    }
}

/// The status, the wake-up, and the count of the decision Model Attempts of the Run.
async fn run_state(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT jsonb_build_array(run.status, run.wakeup_pending,
                  (SELECT count(*) FROM storyos.model_attempts AS attempt
                    WHERE attempt.run_id = run.run_id AND attempt.attempt_role = 'decision'))::text
           FROM storyos.agent_runs AS run WHERE run.run_id = $1::text::uuid",
        &[&run_id],
    )
    .await
}

/// Items 7 and 8 of #436: a cancellation of the parent while its Subrun works refuses the call of
/// the parent with `run_cancelled` and cancels the Subrun in the same transaction. The Model
/// Attempt of the Subrun in flight gets the wake-up.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_cancellation_of_the_parent_cancels_its_subrun() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, run_id) = scripted(&store, &admin, "760", "761", START_SUBRUN_SCRIPT).await;
    let observer = CancelAtSubrun {
        store: &store,
        cancel: cancellation(&store, &scope, 0x76f0, &run_id).await,
        reached: std::sync::Mutex::default(),
    };
    let claim = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let stopped = complete_agent_run(&store, &FakeDestination, &observer, &RuntimeClock, &claim);
    let stopped = matches!(stopped.await, Err(CompleteAgentRunError::StaleFence));
    let subrun = subruns(&admin, &run_id).await.remove(0);

    assert_eq!(
        (
            stopped,
            subrun_calls(&admin, &run_id).await,
            records(&admin, &run_id).await[1].clone(),
            run_state(&admin, &subrun).await,
        ),
        (
            true,
            json!([["refused", "run_cancelled", null]]),
            json!("cancelled"),
            json!(["cancelled", true, 1]),
        )
    );
}
