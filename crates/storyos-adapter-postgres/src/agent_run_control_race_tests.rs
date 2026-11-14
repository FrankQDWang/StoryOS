//! A control command that loses a serialization race with the Worker (#1484).

use std::fmt::Debug;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    CancelAgentRunInput, DestinationRequest, ModelProviderAdapter, ModelStreamSink,
    NoContractFaults, Observation, PauseAgentRunInput, PreDispatchRefusal, PreparedRequest,
    ProjectCommandChallengeBinding, ProjectCommandSettlement, SteerAgentRunInput,
    complete_agent_run,
};
use storyos_core::TransitionOutcome;
use tokio::sync::Notify;
use tokio_postgres::{Client, NoTls};

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{RuntimeClock, stores};
use crate::agent_run_steering::tests::{cancellation, claim_duties, queued, settle, steering};
use crate::command_sequence::tests::{CommandCall, PAUSE_AGENT_RUN, issued};

/// The control command of the author.
enum Control {
    Cancel(CommandCall<CancelAgentRunInput>),
    Pause(CommandCall<PauseAgentRunInput>),
    Steer(CommandCall<SteerAgentRunInput>),
}

impl Control {
    fn binding(&self) -> &ProjectCommandChallengeBinding {
        match self {
            Self::Cancel(call) => &call.envelope.challenge_binding,
            Self::Pause(call) => &call.envelope.challenge_binding,
            Self::Steer(call) => &call.envelope.challenge_binding,
        }
    }

    /// Settles the command and gives its answer.
    async fn settle(&self, store: &PostgresProjectReader) -> String {
        match self {
            Self::Cancel(call) => answer(store.cancel_agent_run(&call.envelope, &call.input).await),
            Self::Pause(call) => answer(store.pause_agent_run(&call.envelope, &call.input).await),
            Self::Steer(call) => answer(store.steer_agent_run(&call.envelope, &call.input).await),
        }
    }
}

fn answer<A, N: Debug, C: Debug, R: Debug, P, Z>(
    result: Result<ProjectCommandSettlement<A, N, C, R, P, Z>, impl Debug>,
) -> String {
    match result.map(|settlement| settlement.outcome) {
        Ok(TransitionOutcome::Applied(_)) => "applied".to_owned(),
        Ok(TransitionOutcome::NoEffect(reason)) => format!("no_effect {reason:?}"),
        Ok(TransitionOutcome::Conflicted(reason)) => format!("conflicted {reason:?}"),
        Ok(TransitionOutcome::Refused(reason)) => format!("refused {reason:?}"),
        Err(error) => format!("error {error:?}"),
    }
}

/// The fake model of the tool loop profile. While the Create request `race` of the Run is out,
/// a second connection holds the Challenge row of the author command, so the command begins its
/// Serializable transaction and waits. Then the Worker commits the record of that request. The
/// row is released at the prepare of the next request, which waits for the answer, or when the
/// Run settles.
struct RaceAt<'a> {
    holder: &'a Client,
    binding: &'a ProjectCommandChallengeBinding,
    race: usize,
    start: &'a Notify,
    answered: &'a Notify,
    creates: Mutex<usize>,
    released: AtomicBool,
}

impl RaceAt<'_> {
    async fn hold(&self) {
        let scope = &self.binding.project_scope;
        self.holder.batch_execute("BEGIN").await.unwrap();
        self.holder
            .execute(
                "SELECT 1 FROM storyos.project_command_challenges
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND command_kind = $3 AND idempotency_key = $4::text::uuid
                  FOR UPDATE",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.binding.command_kind,
                    &self.binding.idempotency_key,
                ],
            )
            .await
            .unwrap();
        self.start.notify_one();
        let blocked = async {
            loop {
                let waiting: i64 = self
                    .holder
                    .query_one(
                        "SELECT count(*) FROM pg_stat_activity
                          WHERE pg_backend_pid() = ANY (pg_blocking_pids(pid))",
                        &[],
                    )
                    .await
                    .unwrap()
                    .get(/*idx*/ 0);
                if waiting > 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
            }
        };
        tokio::time::timeout(Duration::from_secs(/*secs*/ 30), blocked)
            .await
            .expect("the author command waits for the Challenge row");
    }

    async fn release(&self) {
        if !self.released.swap(true, Ordering::SeqCst) {
            self.holder.batch_execute("ROLLBACK").await.unwrap();
        }
    }
}

impl ModelProviderAdapter for RaceAt<'_> {
    const ADAPTERS: &'static [storyos_core::ModelAdapter] = FakeDestination::ADAPTERS;
    type Prepared = (
        Option<usize>,
        <FakeDestination as ModelProviderAdapter>::Prepared,
    );

    async fn prepare(
        &self,
        request: &DestinationRequest,
    ) -> Result<PreparedRequest<Self::Prepared>, PreDispatchRefusal> {
        let create = match request {
            DestinationRequest::Create(_) => {
                let mut creates = self.creates.lock().unwrap();
                *creates += 1;
                Some(*creates)
            }
            DestinationRequest::Retrieve(_) | DestinationRequest::Abort(_) => None,
        };
        if create == Some(self.race + 1) {
            self.release().await;
            tokio::time::timeout(Duration::from_secs(/*secs*/ 30), self.answered.notified())
                .await
                .expect("the author command answers");
        }
        let prepared = FakeDestination.prepare(request).await?;
        Ok(PreparedRequest {
            prepared: (create, prepared.prepared),
            projection: prepared.projection,
        })
    }

    async fn exchange(
        &self,
        (create, prepared): Self::Prepared,
        sink: &mut impl ModelStreamSink,
    ) -> Observation {
        if create == Some(self.race) {
            self.hold().await;
        }
        FakeDestination.exchange(prepared, sink).await
    }
}

async fn run_status(admin: &Client, run_id: &str) -> String {
    admin
        .query_one(
            "SELECT status FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

#[derive(Clone, Copy)]
enum Kind {
    Cancel,
    Pause,
    Steer,
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_control_command_that_loses_a_race_with_the_worker_answers_the_committed_run() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL").unwrap();
    // Request 1 is the first RunStep, and request 2 is the closing RunStep. The Worker commits
    // the record of the request while the command waits in its transaction.
    let cases = [
        ("f4e", 0xf6a0, Kind::Cancel, 1, "applied", "cancelled"),
        (
            "f4f",
            0xf6a1,
            Kind::Cancel,
            2,
            "conflicted TerminalRun",
            "completed",
        ),
        (
            "f5e",
            0xf6a2,
            Kind::Pause,
            2,
            "conflicted TerminalRun",
            "completed",
        ),
        (
            "f5f",
            0xf6a3,
            Kind::Steer,
            2,
            "conflicted TerminalRun",
            "completed",
        ),
    ];
    for (prefix, suffix, kind, race, expected_answer, expected_status) in cases {
        let (scope, _, run_id) = queued(&store, prefix).await;
        let control = match kind {
            Kind::Cancel => Control::Cancel(cancellation(&store, &scope, suffix, &run_id).await),
            Kind::Pause => {
                let input = PauseAgentRunInput {
                    run_id: run_id.clone(),
                };
                Control::Pause(issued(&store, &scope, suffix, &PAUSE_AGENT_RUN, input).await)
            }
            Kind::Steer => Control::Steer(
                steering(&store, &scope, suffix, &run_id, "Keep the voice.", None).await,
            ),
        };
        let (holder, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
        let connection = tokio::spawn(connection);
        let (start, answered) = (Notify::new(), Notify::new());
        let adapter = RaceAt {
            holder: &holder,
            binding: control.binding(),
            race,
            start: &start,
            answered: &answered,
            creates: Mutex::new(/*value*/ 0),
            released: AtomicBool::new(false),
        };
        let worker = async {
            let _settled = settle(&store, &admin, &scope, &run_id, &adapter).await;
            adapter.release().await;
        };
        let author = async {
            start.notified().await;
            let answer = control.settle(&store).await;
            answered.notify_one();
            answer
        };
        let ((), answer) = tokio::join!(worker, author);
        // A new claim does the cancellation duties of a request that was out.
        if let Some(claim) = claim_duties(&admin, &scope, &run_id).await {
            complete_agent_run(
                &store,
                &FakeDestination,
                &NoContractFaults,
                &RuntimeClock,
                &claim,
            )
            .await
            .unwrap();
        }
        drop(holder);
        connection.abort();

        assert_eq!(
            (answer, run_status(&admin, &run_id).await),
            (expected_answer.to_owned(), expected_status.to_owned()),
            "{prefix}"
        );
    }
}
