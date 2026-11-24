//! The Runs of the author go first (ADR 0094 section 5): the Worker claims a queued Run of the
//! author before a queued capture, and a capture gives its claim back between two RunSteps when a
//! Run of the author is queued. The capture continues later at the same RunStep.

use crate::statement_cache::Client;
use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{NoContractFaults, complete_agent_run};

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_background_run::claim_in_project;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::memory_capture::{project, run_capture, start};
use crate::shared_tests::memory_capture_loop::{script, settlement};
use crate::shared_tests::model_gateway::{Admission, RuntimeClock};

fn newer_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// The kind and the RunStep of each decision request of the Project, in the order of the
/// requests.
async fn request_order(admin: &Client, project_id: &str) -> Vec<Value> {
    admin
        .query(
            "SELECT jsonb_build_array(run.run_kind, attempt.decision_position)::text
               FROM storyos.model_attempts AS attempt
               JOIN storyos.agent_runs AS run USING (owner_user_id, project_id, run_id)
              WHERE attempt.project_id = $1::text::uuid AND attempt.attempt_role = 'decision'
              ORDER BY attempt.model_attempt_id",
            &[&project_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
        .collect()
}

/// A Run of the author that the author queued after the capture started gets the next claim. A
/// Run of the author that is queued between two RunSteps of the capture takes the claim of the
/// capture, so it waits for one request of the capture only. Then the capture continues at the
/// same RunStep and settles.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_of_the_author_goes_first_and_the_capture_continues_at_the_same_run_step() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "fde").await;
    let refused = ("propose_edit", json!({"reason": "改写"}));
    let started = start(
        &store,
        &scope,
        &source,
        &script(&[refused.clone(), refused]),
    )
    .await
    .unwrap();
    // The identity of the Run of the author is newer than the identity of the capture Run.
    let first = Admission {
        message: "Help.",
        run_id: Some(newer_id()),
        ..Default::default()
    }
    .admit(&store, &scope, &chapter_id, "fdf")
    .await;
    let claimed = claim_in_project(&store, &scope).await.unwrap();
    complete_agent_run(
        &store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &claimed,
    )
    .await
    .unwrap();
    let capture = claim_in_project(&store, &scope).await.unwrap();
    let second = Admission {
        message: "Help.",
        run_id: Some(newer_id()),
        ..Default::default()
    }
    .admit(&store, &scope, &chapter_id, "fd7")
    .await;
    complete_agent_run(
        &store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &capture,
    )
    .await
    .unwrap();
    let given_back = json_of(
        &admin,
        "SELECT jsonb_build_array(status, active_decision_position::text)::text
               FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
        &[&started.run_id],
    )
    .await;
    let next = claim_in_project(&store, &scope).await.unwrap();
    complete_agent_run(
        &store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &next,
    )
    .await
    .unwrap();
    run_capture(&store, &admin, &scope, &started.run_id).await;

    assert_eq!(
        (claimed.run_id, capture.run_id, given_back, next.run_id),
        (
            first,
            started.run_id.clone(),
            json!(["queued", "1"]),
            second
        )
    );
    let order = request_order(&admin, scope.project_id.as_ref()).await;
    let capture_steps: Vec<Value> = order
        .iter()
        .filter(|request| request[0] == "memory_capture")
        .map(|request| request[1].clone())
        .collect();
    let author_between = order
        .iter()
        .skip_while(|request| request[0] == "author")
        .skip(1)
        .take_while(|request| request[0] == "author")
        .count();
    assert_eq!(capture_steps, [json!(0), json!(1), json!(2)]);
    assert!(author_between > 0, "{order:?}");
    assert_eq!(
        settlement(&admin, &started.run_id).await,
        json!(["completed", "settled", "no_observation", null, []])
    );
}
