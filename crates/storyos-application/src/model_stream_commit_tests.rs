use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use storyos_core::{ModelAdapter, NativeStreamItem, StreamItemRole, StreamItemState};

use crate::model_gateway_ports::{
    DestinationRequest, DispatchClaim, DispatchRecord, GatewayClock, ModelDispatchStore,
    ModelProviderAdapter, ModelStreamSink, NextDispatchWork, Observation, PreDispatchRefusal,
    PreparedRequest, ReferenceRetrieval, ReportedBinding, RequestAttempt, RequestRoute,
    ResponseReference, RetrievePurpose, RetrieveRequest, StreamControl, StreamStop,
    WirePayloadProjection,
};
use crate::{
    ClaimedAgentRun, CompleteAgentRun, CompleteAgentRunError, NoContractFaults, ProjectId,
    ProjectScope, UserId, complete_agent_run,
};

/// One durable step that the store saw, in order.
#[derive(Clone, Debug, PartialEq)]
enum Step {
    Append {
        items: Vec<String>,
        reference: Option<String>,
    },
    Record(Observation),
}

/// The result of one append transaction.
#[derive(Clone, Copy)]
enum Append {
    Committed,
    Contended,
    Unavailable,
}

/// A store with one claimed Retrieve. Each append transaction takes the next scripted result,
/// and commits when the script is empty.
struct ScriptedStore {
    work: Mutex<VecDeque<NextDispatchWork>>,
    appends: Mutex<VecDeque<Append>>,
    steps: Mutex<Vec<Step>>,
}

impl ScriptedStore {
    fn new(appends: &[Append]) -> Self {
        Self {
            work: Mutex::new(VecDeque::from([NextDispatchWork::Request(
                DestinationRequest::Retrieve(retrieve()),
            )])),
            appends: Mutex::new(appends.iter().copied().collect()),
            steps: Mutex::new(Vec::new()),
        }
    }

    fn steps(&self) -> Vec<Step> {
        self.steps.lock().unwrap().clone()
    }
}

impl ModelDispatchStore for ScriptedStore {
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
        events: &[NativeStreamItem],
        reference: Option<&ResponseReference>,
    ) -> Result<Option<StreamStop>, CompleteAgentRunError> {
        let result = self
            .appends
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Append::Committed);
        let error = || Box::new(std::io::Error::other("scripted"));
        match result {
            Append::Committed => {
                self.steps.lock().unwrap().push(Step::Append {
                    items: events.iter().map(|event| event.item_id.clone()).collect(),
                    reference: reference.map(|reference| reference.reference_id.clone()),
                });
                Ok(None)
            }
            Append::Contended => Err(CompleteAgentRunError::Contended(error())),
            Append::Unavailable => Err(CompleteAgentRunError::Unavailable(error())),
        }
    }

    async fn record(
        &self,
        _claim: &ClaimedAgentRun,
        record: DispatchRecord<'_>,
    ) -> Result<(), CompleteAgentRunError> {
        if let DispatchRecord::Exchange { observation, .. } = record {
            self.steps.lock().unwrap().push(Step::Record(observation));
        }
        Ok(())
    }
}

/// Streams the batches in order, one commit each, until the sink says stop.
struct ScriptedStream {
    batches: Vec<Batch>,
    controls: Mutex<Vec<StreamControl>>,
}

enum Batch {
    Events(&'static [&'static str]),
    Reference(&'static str),
}

impl ScriptedStream {
    fn new(batches: Vec<Batch>) -> Self {
        Self {
            batches,
            controls: Mutex::new(Vec::new()),
        }
    }

    fn controls(&self) -> Vec<StreamControl> {
        self.controls.lock().unwrap().clone()
    }
}

impl ModelProviderAdapter for ScriptedStream {
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

    async fn exchange(&self, _prepared: (), sink: &mut impl ModelStreamSink) -> Observation {
        for batch in &self.batches {
            let control = match batch {
                Batch::Events(ids) => {
                    sink.append(&ids.iter().map(|id| item(id)).collect::<Vec<_>>())
                        .await
                }
                Batch::Reference(id) => sink.retain_reference(&reference(id)).await,
            };
            self.controls.lock().unwrap().push(control);
            if control == StreamControl::Stop {
                break;
            }
        }
        Observation::OutcomeUnknown {
            response_reference: None,
        }
    }
}

/// A Deterministic Test Scheduler: each wait completes at once and is recorded.
#[derive(Default)]
struct RecordedClock(Mutex<Vec<Duration>>);

impl RecordedClock {
    fn waits(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

impl GatewayClock for RecordedClock {
    async fn wait(&self, delay: Duration) {
        self.0.lock().unwrap().push(delay);
    }
}

async fn run(store: &ScriptedStore, stream: &ScriptedStream, clock: &RecordedClock) {
    let claim = ClaimedAgentRun {
        project_scope: ProjectScope::new(UserId::new("user"), ProjectId::new("project")),
        run_id: "run".to_owned(),
        fence_token: 1,
    };
    assert_eq!(
        complete_agent_run(store, stream, &NoContractFaults, clock, &claim)
            .await
            .unwrap(),
        CompleteAgentRun::Settled
    );
}

fn append(items: &[&str], reference: Option<&str>) -> Step {
    Step::Append {
        items: items.iter().map(|item| (*item).to_owned()).collect(),
        reference: reference.map(str::to_owned),
    }
}

fn unknown() -> Step {
    Step::Record(Observation::OutcomeUnknown {
        response_reference: None,
    })
}

#[tokio::test]
async fn a_contended_commit_runs_again_and_the_stream_continues() {
    let store = ScriptedStore::new(&[Append::Contended, Append::Contended]);
    let stream = ScriptedStream::new(vec![Batch::Events(&["a"]), Batch::Events(&["b"])]);
    let clock = RecordedClock::default();

    run(&store, &stream, &clock).await;

    assert_eq!(stream.controls(), vec![StreamControl::Continue; 2]);
    assert_eq!(
        store.steps(),
        vec![append(&["a"], None), append(&["b"], None), unknown()]
    );
    assert_eq!(clock.waits(), 2);
}

#[tokio::test]
async fn exhausted_retries_keep_the_events_for_the_next_commit() {
    let store = ScriptedStore::new(&[Append::Contended; 3]);
    let stream = ScriptedStream::new(vec![
        Batch::Events(&["a", "b"]),
        Batch::Reference("resp_1"),
        Batch::Events(&["c"]),
    ]);
    let clock = RecordedClock::default();

    run(&store, &stream, &clock).await;

    assert_eq!(stream.controls(), vec![StreamControl::Continue; 3]);
    assert_eq!(
        store.steps(),
        vec![
            append(&["a", "b"], Some("resp_1")),
            append(&["c"], None),
            unknown()
        ]
    );
}

#[tokio::test]
async fn the_last_deferred_events_commit_before_the_observation() {
    let store = ScriptedStore::new(&[Append::Contended; 3]);
    let stream = ScriptedStream::new(vec![Batch::Events(&["a"])]);
    let clock = RecordedClock::default();

    run(&store, &stream, &clock).await;

    assert_eq!(stream.controls(), vec![StreamControl::Continue]);
    assert_eq!(store.steps(), vec![append(&["a"], None), unknown()]);
}

#[tokio::test]
async fn contention_that_continues_stops_the_stream() {
    let store = ScriptedStore::new(&[Append::Contended; 64]);
    let stream = ScriptedStream::new((0..8).map(|_| Batch::Events(&["a"])).collect());
    let clock = RecordedClock::default();

    run(&store, &stream, &clock).await;

    let mut controls = vec![StreamControl::Continue; 4];
    controls.push(StreamControl::Stop);
    assert_eq!(stream.controls(), controls);
    assert_eq!(store.steps(), vec![unknown()]);
}

#[tokio::test]
async fn an_error_that_is_not_contention_stops_the_stream_at_once() {
    let store = ScriptedStore::new(&[Append::Unavailable]);
    let stream = ScriptedStream::new(vec![Batch::Events(&["a"]), Batch::Events(&["b"])]);
    let clock = RecordedClock::default();

    run(&store, &stream, &clock).await;

    assert_eq!(stream.controls(), vec![StreamControl::Stop]);
    assert_eq!(store.steps(), vec![unknown()]);
    assert_eq!(clock.waits(), 0);
}

fn item(item_id: &str) -> NativeStreamItem {
    NativeStreamItem {
        item_id: item_id.to_owned(),
        role: StreamItemRole::Assistant,
        state: StreamItemState::Provisional,
        text: Some("text".to_owned()),
        summary: None,
        call_id: None,
        arguments: None,
        refusal: None,
        hosted_report: None,
    }
}

fn reference(reference_id: &str) -> ResponseReference {
    ResponseReference {
        reference_id: reference_id.to_owned(),
        retrieval: ReferenceRetrieval::Unsupported,
        reported_binding: ReportedBinding {
            owner_user_id: None,
            conversation_id: None,
            destination_identity: None,
            mapping_revision: "test".to_owned(),
        },
    }
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
