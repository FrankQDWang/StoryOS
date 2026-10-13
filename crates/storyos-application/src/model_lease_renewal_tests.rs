use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use storyos_core::{ModelAdapter, NativeStreamItem};

use crate::model_gateway_ports::{
    DestinationRequest, DispatchClaim, DispatchRecord, GatewayClock, ModelDispatchStore,
    ModelProviderAdapter, ModelStreamSink, NextDispatchWork, Observation, PreDispatchRefusal,
    PreparedRequest, RequestAttempt, RequestRoute, ResponseReference, RetrievePurpose,
    RetrieveRequest, StreamStop, WirePayloadProjection,
};
use crate::{
    ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError, NoContractFaults, ProjectId,
    ProjectScope, UserId, complete_agent_run,
};

/// The lease period of the store, and the renewal delay that it returns.
const LEASE: Duration = Duration::from_secs(/*secs*/ 30);
const RENEWAL: Duration = Duration::from_secs(/*secs*/ 10);
/// How long the destination holds the response. It is longer than the lease.
const HELD: Duration = Duration::from_secs(/*secs*/ 45);

/// One durable step, with the virtual time of a renewal in seconds.
#[derive(Debug, PartialEq)]
enum Step {
    Renew(u64),
    Record,
}

/// A Deterministic Test Scheduler. Time moves only when the held exchange advances it.
#[derive(Default)]
struct VirtualClock(Mutex<Duration>);

impl VirtualClock {
    fn now(&self) -> Duration {
        *self.0.lock().unwrap()
    }

    fn advance(&self, by: Duration) {
        *self.0.lock().unwrap() += by;
    }
}

impl GatewayClock for VirtualClock {
    async fn wait(&self, delay: Duration) {
        let deadline = self.now() + delay;
        while self.now() < deadline {
            tokio::task::yield_now().await;
        }
    }
}

/// A store with one claimed Retrieve. A renewal takes the next scripted lease state, and holds
/// the lease when the script is empty.
struct LeaseStore<'a> {
    clock: &'a VirtualClock,
    work: Mutex<VecDeque<NextDispatchWork>>,
    held: Mutex<VecDeque<bool>>,
    steps: Mutex<Vec<Step>>,
}

impl<'a> LeaseStore<'a> {
    fn new(clock: &'a VirtualClock, held: &[bool]) -> Self {
        Self {
            clock,
            work: Mutex::new(VecDeque::from([NextDispatchWork::Request(
                DestinationRequest::Retrieve(retrieve()),
            )])),
            held: Mutex::new(held.iter().copied().collect()),
            steps: Mutex::new(Vec::new()),
        }
    }

    fn take_steps(&self) -> Vec<Step> {
        std::mem::take(&mut self.steps.lock().unwrap())
    }
}

impl ModelDispatchStore for LeaseStore<'_> {
    async fn next_dispatch_work(
        &self,
        _claim: &ClaimedAgentRun,
    ) -> Result<NextDispatchWork, CompleteAgentRunError> {
        Ok(self
            .work
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(NextDispatchWork::Settled(CompleteAgentRun::Settled)))
    }

    async fn commit_dispatch_claim(
        &self,
        _claim: &ClaimedAgentRun,
        _request: &DestinationRequest,
        _projection: &WirePayloadProjection,
    ) -> Result<Option<DispatchClaim>, CompleteAgentRunError> {
        unreachable!("the Retrieve is already claimed")
    }

    async fn append_model_stream_events(
        &self,
        _claim: &ClaimedAgentRun,
        _dispatch: &DispatchClaim,
        _events: &[NativeStreamItem],
        _reference: Option<&ResponseReference>,
    ) -> Result<Option<StreamStop>, CompleteAgentRunError> {
        Ok(None)
    }

    async fn renew_agent_run_lease(
        &self,
        _claim: &ClaimedAgentRun,
    ) -> Result<Option<Duration>, CompleteAgentRunError> {
        self.steps
            .lock()
            .unwrap()
            .push(Step::Renew(self.clock.now().as_secs()));
        let held = self
            .held
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(/*default*/ true);
        Ok(held.then_some(RENEWAL))
    }

    async fn record(
        &self,
        _claim: &ClaimedAgentRun,
        _record: DispatchRecord<'_>,
    ) -> Result<(), CompleteAgentRunError> {
        self.steps.lock().unwrap().push(Step::Record);
        Ok(())
    }
}

/// A destination that holds its response for `HELD` of virtual time, one second at each poll.
struct HeldResponse<'a>(&'a VirtualClock);

impl ModelProviderAdapter for HeldResponse<'_> {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::HostFake];
    type Prepared = ();

    async fn prepare(
        &self,
        _request: &DestinationRequest,
    ) -> Result<PreparedRequest<()>, PreDispatchRefusal> {
        Ok(PreparedRequest {
            projection: WirePayloadProjection {
                execution_profile: "test".to_owned(),
                mapping_revision: "test".to_owned(),
                digest: "test".to_owned(),
                serialized_payload: None,
                cache_setting: None,
            },
            prepared: (),
        })
    }

    async fn exchange(&self, _prepared: (), _sink: &mut impl ModelStreamSink) -> Observation {
        while self.0.now() < HELD {
            tokio::task::yield_now().await;
            self.0.advance(Duration::from_secs(/*secs*/ 1));
        }
        Observation::OutcomeUnknown {
            response_reference: None,
        }
    }
}

fn claim() -> ClaimedAgentRun {
    ClaimedAgentRun {
        project_scope: ProjectScope::new(UserId::new("user"), ProjectId::new("project")),
        run_id: "run".to_owned(),
        fence_token: 1,
    }
}

async fn complete(store: &LeaseStore<'_>, clock: &VirtualClock) {
    let completed = complete_agent_run(
        store,
        &HeldResponse(clock),
        &NoContractFaults,
        clock,
        &claim(),
    )
    .await;
    assert_eq!(completed.unwrap(), CompleteAgentRun::Settled);
}

#[tokio::test]
async fn a_request_longer_than_the_lease_keeps_its_claim() {
    let clock = VirtualClock::default();
    let store = LeaseStore::new(&clock, &[]);

    complete(&store, &clock).await;

    // Each renewal comes before the lease of the one before it ends, and the last one covers the
    // end of the request at 45 seconds. The renewal stops when the request settles.
    assert_eq!(
        store.take_steps(),
        vec![
            Step::Renew(0),
            Step::Renew(10),
            Step::Renew(20),
            Step::Renew(30),
            Step::Renew(40),
            Step::Record
        ]
    );
}

#[tokio::test]
async fn a_lost_lease_stops_the_renewal() {
    let clock = VirtualClock::default();
    let store = LeaseStore::new(&clock, &[true, false]);

    complete(&store, &clock).await;

    assert_eq!(
        store.take_steps(),
        vec![Step::Renew(0), Step::Renew(10), Step::Record]
    );
}

#[tokio::test]
async fn a_stopped_worker_stops_the_renewal() {
    let clock = VirtualClock::default();
    let store = LeaseStore::new(&clock, &[]);
    let adapter = HeldResponse(&clock);
    let claim = claim();

    // The Worker stops at 15 seconds, while the request is in progress.
    tokio::select! {
        _ = complete_agent_run(&store, &adapter, &NoContractFaults, &clock, &claim) => {
            panic!("the request is still held");
        }
        () = clock.wait(Duration::from_secs(/*secs*/ 15)) => {}
    }
    clock.advance(LEASE * 2);
    tokio::task::yield_now().await;

    assert_eq!(store.take_steps(), vec![Step::Renew(0), Step::Renew(10)]);
}

fn retrieve() -> RetrieveRequest {
    RetrieveRequest {
        attempt: RequestAttempt::Claimed(DispatchClaim {
            model_attempt_id: "attempt".to_owned(),
        }),
        route: RequestRoute {
            adapter: ModelAdapter::HostFake,
            provider_model_id: "test".to_owned(),
            mapping_revision: "test".to_owned(),
            endpoint: None,
            credential_reference: None,
            bounds: None,
        },
        purpose: RetrievePurpose::OriginalResult,
        original_model_attempt_id: "attempt".to_owned(),
        response_reference: "resp_0".to_owned(),
        declared_targets: Vec::new(),
    }
}
