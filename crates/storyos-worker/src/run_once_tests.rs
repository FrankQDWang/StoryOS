use std::sync::Mutex;
use std::time::Duration;

use storyos_adapter_fake_destination::FakeEmbeddings;
use storyos_application::{
    ArchiveExportWorkStore, ChapterPassageSource, ClaimedAgentRun, ClaimedArchiveExport,
    ClaimedReadableExport, CompleteAgentRun, CompleteAgentRunError, CompleteArchiveExport,
    CompleteArchiveExportError, CompleteReadableExport, CompleteReadableExportError,
    DestinationRequest, DispatchClaim, DispatchRecord, ManuscriptSearchBlockFact, ModelStreamSink,
    NextDispatchWork, NoContractFaults, Observation, PassageIndexFacts, PassageVectorKey,
    PassageVectorRow, PassageVectorStore, PreDispatchRefusal, PreparedRequest, ProjectId,
    ProjectScope, ReadableExportWorkStore, ResponseReference, StreamStop, UserId,
    WirePayloadProjection,
};
use storyos_core::{HOST_FAKE_EMBEDDING_REGISTRATION, ModelAdapter, NativeStreamItem};

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

/// A store with `runs` queued AgentRuns, and the schedule rows and the index work of `work`. The
/// first claims fail with a/// serialization failure, and the first completions fail with `completion_failure`.
struct RaceStore {
    runs: Mutex<u64>,
    schedule_claims: Mutex<u64>,
    claim_failures: Mutex<u64>,
    completion_failures: Mutex<u64>,
    completion_failure: fn() -> CompleteAgentRunError,
    settled: Mutex<Vec<ClaimedAgentRun>>,
    work: Mutex<Work>,
}

/// The queued work of the store and the order of what the Worker did. The first embedding request
/// is the barrier: an AgentRun becomes ready during it.
struct Work {
    /// The due schedule rows of background Memory work.
    schedule_rows: u64,
    /// The text of the one Chapter with index work, and the passages with a row.
    index: Option<(ChapterPassageSource, Vec<PassageVectorKey>)>,
    index_claimed: bool,
    /// The reads of waiting index work and the calls of the index claim.
    index_reads: u64,
    index_claim_calls: u64,
    /// The Embedding Registrations of each index claim.
    claimed_registrations: Vec<Vec<String>>,
    events: Vec<&'static str>,
}

impl Default for Work {
    fn default() -> Self {
        Self {
            schedule_rows: 1,
            index: None,
            index_claimed: false,
            index_reads: 0,
            index_claim_calls: 0,
            claimed_registrations: Vec::new(),
            events: Vec::new(),
        }
    }
}

/// A store with no queued AgentRun, `schedule_rows` due schedule rows, and the index work of one
/// Chapter with two passages.
fn index_store(schedule_rows: u64) -> RaceStore {
    let block = |id: &str, text: String| ManuscriptSearchBlockFact {
        manuscript_block_id: id.to_owned(),
        text,
    };
    let chapter = ChapterPassageSource {
        head_revision_id: "revision".to_owned(),
        volume_title: "第一卷".to_owned(),
        title: "第一章".to_owned(),
        blocks: vec![
            block("block-1", "潮水退去，灯塔亮了。".repeat(30)),
            block("block-2", "＊＊＊".to_owned()),
            block("block-3", "风从海上吹来，带着盐。".repeat(30)),
        ],
    };
    RaceStore {
        runs: Mutex::new(0),
        schedule_claims: Mutex::new(0),
        claim_failures: Mutex::new(0),
        completion_failures: Mutex::new(0),
        completion_failure: connection_lost,
        settled: Mutex::new(Vec::new()),
        work: Mutex::new(Work {
            schedule_rows,
            index: Some((chapter, Vec::new())),
            ..Work::default()
        }),
    }
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
        let mut runs = self.runs.lock().unwrap();
        if *runs == 0 {
            return Ok(None);
        }
        *runs -= 1;
        self.work.lock().unwrap().events.push("agent_run");
        Ok(Some(claim()))
    }
}

impl MemoryWorkScheduleStore for RaceStore {
    async fn memory_work_due(&self) -> Result<bool, ProjectReadError> {
        Ok(self.work.lock().unwrap().schedule_rows > 0)
    }

    async fn claim_due_memory_work(
        &self,
    ) -> Result<Option<storyos_application::ClaimedMemoryWork>, ProjectReadError> {
        *self.schedule_claims.lock().unwrap() += 1;
        let mut work = self.work.lock().unwrap();
        work.schedule_rows -= 1;
        work.events.push("schedule");
        Ok(Some(storyos_application::ClaimedMemoryWork {
            project_scope: claim().project_scope,
            source: storyos_application::MemoryWorkSource::Conversation("conversation".to_owned()),
            outcome: MemoryWorkOutcome::Skipped(
                storyos_core::ConversationCaptureSkip::NothingUnread,
            ),
        }))
    }
}

impl storyos_application::ToolDispatchStore for RaceStore {
    async fn next_tool_call(
        &self,
        _claim: &ClaimedAgentRun,
    ) -> Result<Option<storyos_application::ToolWork>, CompleteAgentRunError> {
        unreachable!("the AgentRun settles without a ToolCall")
    }

    async fn record_question_search(
        &self,
        _claim: &ClaimedAgentRun,
        _search: &storyos_application::QuestionSearch,
        _result: storyos_application::EmbeddingResult,
    ) -> Result<(), CompleteAgentRunError> {
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

impl storyos_application::EmbeddingDispatchStore for RaceStore {
    async fn embedding_route(
        &self,
        _scope: &ProjectScope,
    ) -> Result<Option<storyos_application::EmbeddingRoute>, ProjectReadError> {
        Ok(Some(storyos_application::EmbeddingRoute {
            registration_revision: HOST_FAKE_EMBEDDING_REGISTRATION.revision.to_owned(),
            endpoint: None,
            credential_reference: None,
        }))
    }

    async fn commit_embedding_claim(
        &self,
        _scope: &ProjectScope,
        _registration: Option<&'static storyos_core::EmbeddingRegistration>,
        _input: &storyos_core::EmbeddingInput,
        _projection: &storyos_application::EmbeddingProjection,
    ) -> Result<
        Result<storyos_application::EmbeddingClaim, storyos_core::EmbeddingRefusal>,
        ProjectReadError,
    > {
        let mut work = self.work.lock().unwrap();
        if !work.events.contains(&"request") {
            *self.runs.lock().unwrap() += 1;
        }
        work.events.push("request");
        Ok(Ok(storyos_application::EmbeddingClaim {
            embedding_attempt_id: "attempt".to_owned(),
        }))
    }

    async fn record_embedding(
        &self,
        _scope: &ProjectScope,
        _claim: &storyos_application::EmbeddingClaim,
        _record: &storyos_application::EmbeddingRecord<'_>,
    ) -> Result<(), ProjectReadError> {
        Ok(())
    }
}

impl PassageVectorStore for RaceStore {
    async fn read_passage_index_facts(
        &self,
        _scope: &ProjectScope,
        _chapter_id: &str,
    ) -> Result<PassageIndexFacts, ProjectReadError> {
        let work = self.work.lock().unwrap();
        let (chapter, stored) = work.index.clone().expect("the store has index work");
        Ok(PassageIndexFacts {
            chapter: Some(chapter),
            stored,
        })
    }
}

impl PassageIndexWorkStore for RaceStore {
    async fn passage_index_work_waits(
        &self,
        _registration_revisions: &[&str],
        _cut_revision: &str,
    ) -> Result<bool, ProjectReadError> {
        let mut work = self.work.lock().unwrap();
        work.index_reads += 1;
        Ok(work.index.is_some() && !work.index_claimed)
    }

    async fn claim_next_passage_index_work(
        &self,
        registration_revisions: &[&str],
        _cut_revision: &str,
        lease: Duration,
    ) -> Result<Option<ClaimedPassageIndex>, ProjectReadError> {
        let mut work = self.work.lock().unwrap();
        work.index_claim_calls += 1;
        if work.index.is_none() || work.index_claimed {
            return Ok(None);
        }
        work.index_claimed = true;
        let registrations = registration_revisions
            .iter()
            .map(|&revision| revision.to_owned());
        work.claimed_registrations.push(registrations.collect());
        work.events.push("index_claim");
        Ok(Some(ClaimedPassageIndex {
            project_scope: claim().project_scope,
            chapter_id: "chapter".to_owned(),
            embedding_registration_revision: HOST_FAKE_EMBEDDING_REGISTRATION.revision.to_owned(),
            mark_generation: 1,
            fence_token: 1,
            lease,
        }))
    }

    async fn write_fenced_passage_vector(
        &self,
        _claim: &ClaimedPassageIndex,
        row: &PassageVectorRow<'_>,
    ) -> Result<bool, ProjectReadError> {
        let mut work = self.work.lock().unwrap();
        work.index.as_mut().unwrap().1.push(row.key.clone());
        Ok(true)
    }

    async fn remove_fenced_passage_vectors(
        &self,
        _claim: &ClaimedPassageIndex,
        _keys: &[PassageVectorKey],
    ) -> Result<bool, ProjectReadError> {
        unreachable!("the Chapter does not change")
    }

    async fn complete_passage_index_work(
        &self,
        _claim: &ClaimedPassageIndex,
        _cut_revision: &str,
    ) -> Result<bool, ProjectReadError> {
        self.work.lock().unwrap().events.push("completed");
        Ok(true)
    }

    async fn release_passage_index_work(
        &self,
        _claim: &ClaimedPassageIndex,
        _wait: Duration,
    ) -> Result<bool, ProjectReadError> {
        unreachable!("each request gives a vector")
    }

    async fn mark_each_passage_index_work(&self) -> Result<u64, ProjectReadError> {
        unreachable!("the Worker loop does not rebuild")
    }
}

impl ModelDispatchStore for RaceStore {
    async fn next_dispatch_work(
        &self,
        claim: &ClaimedAgentRun,
        _adapters: &'static [ModelAdapter],
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
        runs: Mutex::new(1),
        schedule_claims: Mutex::new(0),
        claim_failures: Mutex::new(1),
        completion_failures: Mutex::new(0),
        completion_failure: serialization_failure,
        settled: Mutex::new(Vec::new()),
        work: Mutex::default(),
    };
    let destination = ModelDestination {
        adapter: NoDestination,
        observer: NoContractFaults,
        tools: storyos_application::NoToolDestination,
        embeddings: NoEmbeddings,
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
        runs: Mutex::new(1),
        schedule_claims: Mutex::new(0),
        claim_failures: Mutex::new(0),
        completion_failures: Mutex::new(1),
        completion_failure: serialization_failure,
        settled: Mutex::new(Vec::new()),
        work: Mutex::default(),
    };
    let destination = ModelDestination {
        adapter: NoDestination,
        observer: NoContractFaults,
        tools: storyos_application::NoToolDestination,
        embeddings: NoEmbeddings,
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
        runs: Mutex::new(1),
        schedule_claims: Mutex::new(0),
        claim_failures: Mutex::new(0),
        completion_failures: Mutex::new(2),
        completion_failure: connection_lost,
        settled: Mutex::new(Vec::new()),
        work: Mutex::default(),
    };
    let destination = ModelDestination {
        adapter: NoDestination,
        observer: NoContractFaults,
        tools: storyos_application::NoToolDestination,
        embeddings: NoEmbeddings,
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

fn fake_destination() -> ModelDestination<
    NoDestination,
    NoContractFaults,
    storyos_application::NoToolDestination,
    FakeEmbeddings,
> {
    ModelDestination {
        adapter: NoDestination,
        observer: NoContractFaults,
        tools: storyos_application::NoToolDestination,
        embeddings: FakeEmbeddings,
    }
}

/// A due schedule row and the index work at the same time: the schedule row goes first. An
/// AgentRun that becomes ready during the first embedding request goes before the next request.
#[tokio::test(start_paused = true)]
async fn the_loop_claims_the_schedule_row_and_a_ready_agent_run_before_the_index_work() {
    let store = index_store(1);
    // The destination of the Worker inside the Server process.
    let destination = fake_destination();
    let mut index = None;
    let mut turns = Vec::new();
    for _ in 0..6 {
        turns.push(step(&store, &destination, &mut index).await);
    }

    let work = store.work.lock().unwrap();
    assert_eq!(
        (
            turns,
            work.events.clone(),
            store.settled.lock().unwrap().len()
        ),
        (
            vec![true, true, true, true, true, false],
            vec![
                "schedule",
                "index_claim",
                "request",
                "agent_run",
                "request",
                "completed"
            ],
            1
        )
    );
    // The Worker inside the Server process claims only the work of the fake adapter.
    assert_eq!(
        work.claimed_registrations,
        [[HOST_FAKE_EMBEDDING_REGISTRATION.revision]]
    );
}

/// A due schedule row and the index work at the same time: `run_once` claims the schedule row, and
/// the next `run_once` completes one index claim.
#[tokio::test(start_paused = true)]
async fn run_once_claims_the_schedule_row_before_the_index_work_and_completes_the_claim() {
    let store = index_store(1);
    run_once(&store, &fake_destination())
        .await
        .expect("the schedule row is claimed");
    let after_schedule = store.work.lock().unwrap().events.clone();
    run_once(&store, &fake_destination())
        .await
        .expect("the index work completes");

    assert_eq!(
        (after_schedule, store.work.lock().unwrap().events.clone()),
        (
            vec!["schedule"],
            vec!["schedule", "index_claim", "request", "request", "completed"]
        )
    );
}

/// The Worker claims a due schedule row only when the AgentRun claim finds no Run (ADR 0094
/// section 5): a Run of the author does not wait for background Memory work.
#[tokio::test(start_paused = true)]
async fn run_once_claims_a_schedule_row_only_after_the_agent_run_claim_finds_no_run() {
    let store = RaceStore {
        runs: Mutex::new(1),
        schedule_claims: Mutex::new(0),
        claim_failures: Mutex::new(0),
        completion_failures: Mutex::new(0),
        completion_failure: serialization_failure,
        settled: Mutex::new(Vec::new()),
        work: Mutex::default(),
    };
    let destination = ModelDestination {
        adapter: NoDestination,
        observer: NoContractFaults,
        tools: storyos_application::NoToolDestination,
        embeddings: NoEmbeddings,
    };

    run_once(&store, &destination).await.unwrap();
    let after_run = *store.schedule_claims.lock().unwrap();
    run_once(&store, &destination).await.unwrap();

    assert_eq!(
        (
            after_run,
            *store.schedule_claims.lock().unwrap(),
            store.settled.lock().unwrap().clone()
        ),
        (0, 1, vec![claim()])
    );
}

/// ADR 0047: an idle turn of the loop reads one time whether index work waits, and it makes no
/// claim, so it opens no claim span. A Worker with no Embedding Registration makes no store call
/// for index work. A turn with waiting index work claims it, as before.
#[tokio::test(start_paused = true)]
async fn an_idle_turn_reads_once_and_makes_no_claim_of_the_index_work() {
    let idle = RaceStore {
        work: Mutex::new(Work {
            schedule_rows: 0,
            ..Work::default()
        }),
        ..index_store(0)
    };
    let no_embeddings = ModelDestination {
        adapter: NoDestination,
        observer: NoContractFaults,
        tools: storyos_application::NoToolDestination,
        embeddings: NoEmbeddings,
    };
    let mut index = None;
    let reads_and_claims = |store: &RaceStore| {
        let work = store.work.lock().unwrap();
        (work.index_reads, work.index_claim_calls)
    };

    let fake_turn = step(&idle, &fake_destination(), &mut index).await;
    let after_fake = reads_and_claims(&idle);
    let no_embeddings_turn = step(&idle, &no_embeddings, &mut index).await;
    let after_no_embeddings = reads_and_claims(&idle);
    let work = index_store(0);
    let claim_turn = step(&work, &fake_destination(), &mut index).await;

    assert_eq!(
        (
            (fake_turn, after_fake),
            (no_embeddings_turn, after_no_embeddings),
            (claim_turn, reads_and_claims(&work)),
        ),
        ((false, (1, 0)), (false, (1, 0)), (true, (1, 1)))
    );
}

/// The embedding destination of a process with no embedding adapter. Each request refuses before
/// the claim.
struct NoEmbeddings;

impl storyos_application::EmbeddingProviderAdapter for NoEmbeddings {
    const REGISTRATIONS: &'static [&'static storyos_core::EmbeddingRegistration] = &[];
    type Prepared = std::convert::Infallible;

    async fn prepare(
        &self,
        _registration: &'static storyos_core::EmbeddingRegistration,
        _route: &storyos_application::EmbeddingRoute,
        _input: &storyos_core::EmbeddingInput,
    ) -> Result<
        storyos_application::PreparedEmbeddingRequest<Self::Prepared>,
        storyos_application::PreDispatchRefusal,
    > {
        Err(storyos_application::PreDispatchRefusal::CredentialUnavailable)
    }

    async fn exchange(
        &self,
        prepared: Self::Prepared,
    ) -> storyos_application::EmbeddingObservation {
        match prepared {}
    }
}
