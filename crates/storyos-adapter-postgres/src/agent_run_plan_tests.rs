//! The plan of a Run at the dispatch seam and in the Run read (ADR 0103 section 2). Test prefix
//! lead `6d`, and the challenge suffix `0x6dc0`.

use std::sync::Mutex;

use storyos_adapter_fake_destination::{FakeDestination, UPDATE_PLAN_SCRIPT, UPDATE_PLAN_STEPS};
use storyos_application::{
    AgentRunReadSelection, AgentRunReadStore, DestinationRequest, ModelProviderAdapter,
    ModelStreamSink, Observation, PreDispatchRefusal, PreparedRequest, ProjectScope,
};
use storyos_core::{AgentRunStatus, PlanStep, PlanStepStatus};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::agent_run_steering::{
    AuthorAction, AuthorAt, Moment, cancellation, settle,
};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::model_gateway::{Admission, assisted_chapter};

/// The status and the plan of the Run read of `run_id`.
async fn plan_of(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    run_id: &str,
) -> (AgentRunStatus, Option<Vec<PlanStep>>) {
    let run = store
        .read_agent_run(scope, run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    (run.status, run.plan)
}

/// The steps of the plan script with `statuses`.
fn script_plan(statuses: [PlanStepStatus; 3]) -> Option<Vec<PlanStep>> {
    Some(
        (UPDATE_PLAN_STEPS.iter().zip(statuses))
            .map(|(text, status)| PlanStep {
                text: (*text).to_owned(),
                status,
            })
            .collect(),
    )
}

/// The function name, the RunStep position, the state, and the result of each ToolCall of the Run.
async fn calls(admin: &Client, run_id: &str) -> Vec<(String, i64, String, Option<String>)> {
    admin
        .query(
            "SELECT function_name, decision_position::bigint, state, result_text
               FROM storyos.tool_calls WHERE run_id = $1::text::uuid
              ORDER BY decision_position, call_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3)))
        .collect()
}

/// The fake model of the tool loop profile. Before each Create request, it reads the plan of the
/// Run read, so it sees the plan after each RunStep.
struct PlanReads<'a> {
    store: &'a PostgresProjectReader,
    scope: &'a ProjectScope,
    run_id: &'a str,
    plans: Mutex<Vec<Option<Vec<PlanStep>>>>,
}

impl ModelProviderAdapter for PlanReads<'_> {
    const ADAPTERS: &'static [storyos_core::ModelAdapter] = FakeDestination::ADAPTERS;
    type Prepared = <FakeDestination as ModelProviderAdapter>::Prepared;

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal> {
        if let DestinationRequest::Create(_) = request {
            let (_, plan) = plan_of(self.store, self.scope, self.run_id).await;
            self.plans.lock().unwrap().push(plan);
        }
        FakeDestination.prepare(request).await
    }

    async fn exchange(
        &self,
        prepared: Self::Prepared,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        FakeDestination.exchange(prepared, sink).await
    }
}

/// ADR 0103 section 2 and the fake message of #435 item 8: each `update_plan` call of the three
/// RunSteps settles with "The plan is recorded.", and the Run read gives the plan of the newest
/// call after each RunStep and after the end of the Run. The Host changes no step.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_run_step_records_the_plan_and_the_run_read_gives_the_newest_one() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id) = assisted_chapter(&store, "6d1").await;
    let run_id = Admission {
        message: UPDATE_PLAN_SCRIPT,
        ..Default::default()
    }
    .admit(&store, &scope, &chapter_id, "6d2")
    .await;
    let reads = PlanReads {
        store: &store,
        scope: &scope,
        run_id: &run_id,
        plans: Mutex::default(),
    };
    settle(&store, &admin, &scope, &run_id, &reads)
        .await
        .unwrap();
    let recorded = |position: i64| {
        (
            "update_plan".to_owned(),
            position,
            "settled".to_owned(),
            Some("The plan is recorded.".to_owned()),
        )
    };
    let first = script_plan([
        PlanStepStatus::InProgress,
        PlanStepStatus::Pending,
        PlanStepStatus::Pending,
    ]);
    let second = script_plan([
        PlanStepStatus::Completed,
        PlanStepStatus::InProgress,
        PlanStepStatus::Pending,
    ]);
    let last = script_plan([PlanStepStatus::Completed; 3]);

    assert_eq!(
        (
            calls(&admin, &run_id).await,
            reads.plans.into_inner().unwrap(),
            plan_of(&store, &scope, &run_id).await,
        ),
        (
            vec![
                recorded(0),
                recorded(1),
                recorded(2),
                (
                    "finish".to_owned(),
                    2,
                    "settled".to_owned(),
                    Some(String::new())
                ),
            ],
            vec![None, first, second, last.clone()],
            (AgentRunStatus::Completed, last),
        )
    );
}

/// ADR 0103 section 2: the Run read of a Run with no `update_plan` call and of a Run with only a
/// refused call has no plan, and a Run that the author cancels after one accepted call keeps
/// that plan.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_with_no_accepted_call_has_no_plan_and_a_cancelled_run_keeps_its_plan() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let mut ended = Vec::new();
    for (prefixes, message) in [
        (["6d3", "6d4"], "Help with this passage."),
        (["6d5", "6d6"], r#"SCRIPT:refuse update_plan {"steps":[]}"#),
    ] {
        let (scope, chapter_id) = assisted_chapter(&store, prefixes[0]).await;
        let run_id = Admission {
            message,
            ..Default::default()
        }
        .admit(&store, &scope, &chapter_id, prefixes[1])
        .await;
        complete(&store, &admin, &scope, &run_id).await;
        ended.push((
            plan_of(&store, &scope, &run_id).await,
            calls(&admin, &run_id).await,
        ));
    }
    let (scope, chapter_id) = assisted_chapter(&store, "6d7").await;
    let run_id = Admission {
        message: UPDATE_PLAN_SCRIPT,
        ..Default::default()
    }
    .admit(&store, &scope, &chapter_id, "6d8")
    .await;
    let cancel = cancellation(&store, &scope, 0x6dc0, &run_id).await;
    let author = AuthorAt::new(
        &store,
        Moment::BeforeRequest(2),
        AuthorAction::Cancel(Box::new(cancel)),
    );
    settle(&store, &admin, &scope, &run_id, &author)
        .await
        .unwrap();
    let finish = |position: i64| ("finish".to_owned(), position, "settled".to_owned());
    let names = |calls: &[(String, i64, String, Option<String>)]| -> Vec<(String, i64, String)> {
        (calls.iter())
            .map(|(name, position, state, _)| (name.clone(), *position, state.clone()))
            .collect()
    };

    assert_eq!(
        (
            ended[0].0.clone(),
            names(&ended[0].1),
            ended[1].0.clone(),
            ended[1].1[0].clone(),
            plan_of(&store, &scope, &run_id).await,
        ),
        (
            (AgentRunStatus::Completed, None),
            vec![finish(0)],
            (AgentRunStatus::Completed, None),
            (
                "update_plan".to_owned(),
                0,
                "refused".to_owned(),
                Some("The plan must have 1 through 12 steps.".to_owned()),
            ),
            (
                AgentRunStatus::Cancelled,
                script_plan([
                    PlanStepStatus::InProgress,
                    PlanStepStatus::Pending,
                    PlanStepStatus::Pending,
                ]),
            ),
        )
    );
}
