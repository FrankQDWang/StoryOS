use std::sync::Mutex;
use std::time::Duration;

use storyos_application::{
    ArchiveExportWorkStore, ClaimedAgentRun, ClaimedArchiveExport, ClaimedReadableExport,
    CompleteAgentRun, CompleteAgentRunError, CompleteArchiveExport, CompleteArchiveExportError,
    CompleteReadableExport, CompleteReadableExportError, DestinationRequest, DispatchClaim,
    DispatchRecord, ModelStreamSink, NextDispatchWork, NoContractFaults, Observation,
    PreDispatchRefusal, PreparedRequest, ProjectId, ProjectScope, ReadableExportWorkStore,
    ResponseReference, StreamStop, UserId, WirePayloadProjection,
};
use storyos_core::{ModelAdapter, NativeStreamItem};

use super::*;

#[derive(Debug)]
struct SerializationFailure;

impl std::fmt::Display for SerializationFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("could not serialize access")
    }
}

impl std::error::Error for SerializationFailure {}

fn serialization_failure_state(error: &(dyn std::error::Error + 'static)) -> Option<String> {
    error
        .downcast_ref::<SerializationFailure>()
        .map(|_| "40001".to_owned())
}

#[derive(Debug)]
struct ConnectionLost;

impl std::fmt::Display for ConnectionLost {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("connection lost")
    }
}

impl std::error::Error for ConnectionLost {}

fn serialization_failure() -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(SerializationFailure))
}

fn connection_lost() -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(ConnectionLost))
}

/// A store with one queued AgentRun. The first claims fail with a serialization failure, and the
/// first completions fail with `completion_failure`.
struct RaceStore {
    claim_failures: Mutex<u64>,
    completion_failures: Mutex<u64>,
    completion_failure: fn() -> CompleteAgentRunError,
    settled: Mutex<Vec<ClaimedAgentRun>>,
}

impl ExportWorkStore for RaceStore {
    async fn claim_next_export_work(&self) -> Result<Option<ClaimedExportWork>, ProjectReadError> {
        Ok(None)
    }
}

impl ReadableExportWorkStore for RaceStore {
    async fn claim_next_readable_export(
        &self,
    ) -> Result<Option<ClaimedReadableExport>, ProjectReadError> {
        unreachable!("the Worker claims export work as one claim")
    }

    async fn complete_readable_export(
        &self,
        _claim: &ClaimedReadableExport,
    ) -> Result<CompleteReadableExport, CompleteReadableExportError> {
        unreachable!("the store has no export work")
    }
}

impl ArchiveExportWorkStore for RaceStore {
    async fn claim_next_archive_export(
        &self,
    ) -> Result<Option<ClaimedArchiveExport>, ProjectReadError> {
        unreachable!("the Worker claims export work as one claim")
    }

    async fn complete_archive_export(
        &self,
        _claim: &ClaimedArchiveExport,
    ) -> Result<CompleteArchiveExport, CompleteArchiveExportError> {
        unreachable!("the store has no export work")
    }
}

impl AgentRunWorkStore for RaceStore {
    async fn claim_next_agent_run(
        &self,
        _adapters: &[ModelAdapter],
    ) -> Result<Option<ClaimedAgentRun>, ProjectReadError> {
        let mut failures = self.claim_failures.lock().unwrap();
        if *failures > 0 {
            *failures -= 1;
            return Err(ProjectReadError::unavailable(SerializationFailure));
        }
        Ok(Some(claim()))
    }
}

impl storyos_application::ToolDispatchStore for RaceStore {
    async fn next_tool_call(
        &self,
        _claim: &ClaimedAgentRun,
    ) -> Result<Option<storyos_application::ToolCallRequest>, CompleteAgentRunError> {
        unreachable!("the AgentRun settles without a ToolCall")
    }

    async fn commit_tool_dispatch_claim(
        &self,
        _claim: &ClaimedAgentRun,
        _request: &storyos_application::ToolCallRequest,
        _projection: &WirePayloadProjection,
    ) -> Result<Option<storyos_application::ToolDispatchClaim>, CompleteAgentRunError> {
        unreachable!("the AgentRun settles without a ToolCall")
    }

    async fn record_tool_call(
        &self,
        _claim: &ClaimedAgentRun,
        _record: storyos_application::ToolDispatchRecord<'_>,
    ) -> Result<(), CompleteAgentRunError> {
        unreachable!("the AgentRun settles without a ToolCall")
    }
}

impl ModelDispatchStore for RaceStore {
    async fn next_dispatch_work(
        &self,
        claim: &ClaimedAgentRun,
    ) -> Result<NextDispatchWork, CompleteAgentRunError> {
        let mut failures = self.completion_failures.lock().unwrap();
        if *failures > 0 {
            *failures -= 1;
            return Err((self.completion_failure)());
        }
        self.settled.lock().unwrap().push(claim.clone());
        Ok(NextDispatchWork::Settled(CompleteAgentRun::Settled))
    }

    async fn commit_dispatch_claim(
        &self,
        _claim: &ClaimedAgentRun,
        _request: &DestinationRequest,
        _projection: &WirePayloadProjection,
    ) -> Result<Option<DispatchClaim>, CompleteAgentRunError> {
        unreachable!("the AgentRun settles without a request")
    }

    async fn append_model_stream_events(
        &self,
        _claim: &ClaimedAgentRun,
        _dispatch: &DispatchClaim,
        _events: &[NativeStreamItem],
        _reference: Option<&ResponseReference>,
    ) -> Result<Option<StreamStop>, CompleteAgentRunError> {
        unreachable!("the AgentRun settles without a request")
    }

    async fn renew_agent_run_lease(
        &self,
        _claim: &ClaimedAgentRun,
    ) -> Result<Option<Duration>, CompleteAgentRunError> {
        unreachable!("the AgentRun settles without a request")
    }

    async fn record(
        &self,
        _claim: &ClaimedAgentRun,
        _record: DispatchRecord<'_>,
    ) -> Result<(), CompleteAgentRunError> {
        unreachable!("the AgentRun settles without a request")
    }
}

struct NoDestination;

impl ModelProviderAdapter for NoDestination {
    const ADAPTERS: &'static [ModelAdapter] = &[ModelAdapter::HostFake];
    type Prepared = ();

    async fn prepare(
        &self,
        _request: &DestinationRequest,
    ) -> Result<PreparedRequest<()>, PreDispatchRefusal> {
        unreachable!("the AgentRun settles without a request")
    }

    async fn exchange(&self, _prepared: (), _sink: &mut impl ModelStreamSink) -> Observation {
        unreachable!("the AgentRun settles without a request")
    }
}

fn claim() -> ClaimedAgentRun {
    ClaimedAgentRun {
        project_scope: ProjectScope::new(UserId::new("user"), ProjectId::new("project")),
        run_id: "run".to_owned(),
        fence_token: 1,
    }
}

#[tokio::test(start_paused = true)]
async fn run_once_claims_again_after_a_serialization_failure() {
    storyos_application::register_sql_state(serialization_failure_state);
    let store = RaceStore {
        claim_failures: Mutex::new(1),
        completion_failures: Mutex::new(0),
        completion_failure: serialization_failure,
        settled: Mutex::new(Vec::new()),
    };
    let destination = ModelDestination {
        adapter: NoDestination,
        observer: NoContractFaults,
        tools: storyos_application::NoToolDestination,
    };

    run_once(&store, &destination)
        .await
        .expect("the second claim should complete the AgentRun");

    assert_eq!(*store.settled.lock().unwrap(), vec![claim()]);
}

#[tokio::test(start_paused = true)]
async fn run_once_completes_again_after_a_serialization_failure() {
    storyos_application::register_sql_state(serialization_failure_state);
    let store = RaceStore {
        claim_failures: Mutex::new(0),
        completion_failures: Mutex::new(1),
        completion_failure: serialization_failure,
        settled: Mutex::new(Vec::new()),
    };
    let destination = ModelDestination {
        adapter: NoDestination,
        observer: NoContractFaults,
        tools: storyos_application::NoToolDestination,
    };

    run_once(&store, &destination)
        .await
        .expect("the second completion should settle the AgentRun");

    assert_eq!(*store.settled.lock().unwrap(), vec![claim()]);
}

#[tokio::test(start_paused = true)]
async fn run_once_stops_after_one_failure_that_is_not_a_serialization_failure() {
    storyos_application::register_sql_state(serialization_failure_state);
    let store = RaceStore {
        claim_failures: Mutex::new(0),
        completion_failures: Mutex::new(2),
        completion_failure: connection_lost,
        settled: Mutex::new(Vec::new()),
    };
    let destination = ModelDestination {
        adapter: NoDestination,
        observer: NoContractFaults,
        tools: storyos_application::NoToolDestination,
    };

    let result = run_once(&store, &destination).await;

    assert_eq!(
        (
            result.map_err(|error| error.to_string()),
            *store.completion_failures.lock().unwrap(),
            store.settled.lock().unwrap().clone(),
        ),
        (
            Err("AgentRun work is unavailable".to_owned()),
            1,
            Vec::new()
        )
    );
}
