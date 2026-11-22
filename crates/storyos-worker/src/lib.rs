//! The StoryOS Worker loop. Composition roots supply the store, the adapter, and the observer.

use std::future::Future;
use std::time::Duration;

use storyos_application::{
    AgentRunWorkStore, ClaimedAgentRun, ClaimedArchiveExport, ClaimedExportWork,
    ClaimedPassageIndex, ClaimedReadableExport, CompleteAgentRun, CompleteAgentRunError,
    CompleteArchiveExport, CompleteArchiveExportError, CompleteReadableExport,
    CompleteReadableExportError, ContractFaultObserver, DiagnosticField, DiagnosticId,
    EmbeddingProviderAdapter, ExportWorkStore, GatewayClock, MemoryWorkOutcome,
    MemoryWorkScheduleStore, ModelDispatchStore, ModelProviderAdapter, PassageIndexStep,
    PassageIndexWorkStore, ProjectReadError, SqlState, ToolDestinationAdapter, ToolDispatchStore,
    claim_next_agent_run, claim_next_export_work, claim_next_passage_index_work,
    complete_agent_run_with_destinations, complete_archive_export, complete_readable_export,
    step_passage_index_work,
};

const ATTEMPTS: u64 = 4;
const SERIALIZATION_FAILURE: &str = "40001";

/// Combined persistence port for the packaged Worker loop. The index work store has the embedding
/// dispatch store.
pub trait WorkerStore:
    ExportWorkStore
    + AgentRunWorkStore
    + ModelDispatchStore
    + ToolDispatchStore
    + MemoryWorkScheduleStore
    + PassageIndexWorkStore
{
}

impl<T> WorkerStore for T where
    T: ExportWorkStore
        + AgentRunWorkStore
        + ModelDispatchStore
        + ToolDispatchStore
        + MemoryWorkScheduleStore
        + PassageIndexWorkStore
{
}

/// The destination side of the Worker loop: one Model Provider Adapter, its fault observer, the
/// Tool destination of the ToolDispatch phase, and the embedding adapter (ADR 0095 section 3).
pub struct ModelDestination<A, O, T, E> {
    pub adapter: A,
    pub observer: O,
    pub tools: T,
    pub embeddings: E,
}

/// The lease of a readable export claim: the `STORYOS_EXPORT_LEASE_TTL_SECS` setting, or 30
/// seconds.
pub fn readable_export_lease_ttl(seconds: Option<u64>) -> Duration {
    Duration::from_secs(seconds.unwrap_or(/*default*/ 30))
}

pub async fn run<
    A: ModelProviderAdapter,
    O: ContractFaultObserver,
    T: ToolDestinationAdapter,
    E: EmbeddingProviderAdapter,
>(
    store: impl WorkerStore,
    destination: ModelDestination<A, O, T, E>,
) {
    let mut index = None;
    loop {
        if !step(&store, &destination, &mut index).await {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

pub async fn run_once<
    A: ModelProviderAdapter,
    O: ContractFaultObserver,
    T: ToolDestinationAdapter,
    E: EmbeddingProviderAdapter,
>(
    store: &impl WorkerStore,
    destination: &ModelDestination<A, O, T, E>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match claim_next_work::<A>(store, Retry::SerializationFailure).await? {
        NextWork::Export(ClaimedExportWork::Readable(claim)) => {
            readable_export_claim(store, &claim, Retry::SerializationFailure).await?;
            Ok(())
        }
        NextWork::Export(ClaimedExportWork::Archive(claim)) => {
            archive_export_claim(store, &claim, Retry::SerializationFailure).await?;
            Ok(())
        }
        NextWork::AgentRun(claim) => {
            match agent_run_claim(store, destination, &claim, Retry::SerializationFailure).await {
                Ok(_) | Err(CompleteAgentRunError::StaleFence) => Ok(()),
                Err(error) => Err(error.into()),
            }
        }
        NextWork::MemoryWork => Ok(()),
        NextWork::None => {
            let mut index = None;
            passage_index_step(store, &destination.embeddings, &mut index).await?;
            while index.is_some() {
                passage_index_step(store, &destination.embeddings, &mut index).await?;
            }
            Ok(())
        }
    }
}

/// The work that one claim of the loop gets. The claim order is export work, then a Run, then a
/// due schedule row of background Memory work. The index work comes after each of them.
enum NextWork {
    Export(ClaimedExportWork),
    AgentRun(ClaimedAgentRun),
    MemoryWork,
    None,
}

/// Claims the next work in the claim order. With `Retry::SerializationFailure`, each claim runs
/// again after a serialization failure (`claim_after_contention`).
async fn claim_next_work<A: ModelProviderAdapter>(
    store: &impl WorkerStore,
    retry: Retry,
) -> Result<NextWork, ProjectReadError> {
    if let Some(claim) = claim_with(retry, || claim_next_export_work(store)).await? {
        return Ok(NextWork::Export(claim));
    }
    if let Some(claim) = claim_with(retry, || claim_next_agent_run(store, A::ADAPTERS)).await? {
        return Ok(NextWork::AgentRun(claim));
    }
    Ok(
        match claim_with(retry, || memory_work_claim(store)).await? {
            true => NextWork::MemoryWork,
            false => NextWork::None,
        },
    )
}

/// Runs one claim, and with `Retry::SerializationFailure` again after a serialization failure.
/// With `Retry::EachFailure` the claim runs one time: the loop claims again in its next turn.
async fn claim_with<T, F>(retry: Retry, mut claim: impl FnMut() -> F) -> Result<T, ProjectReadError>
where
    F: Future<Output = Result<T, ProjectReadError>>,
{
    match retry {
        Retry::SerializationFailure => claim_after_contention(claim).await,
        Retry::EachFailure => claim().await,
    }
}

pub async fn claim_only<A: ModelProviderAdapter, E: EmbeddingProviderAdapter>(
    store: &impl WorkerStore,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let NextWork::None = claim_next_work::<A>(store, Retry::EachFailure).await? {
        claim_next_passage_index_work(store, E::REGISTRATIONS).await?;
    }
    Ok(())
}

async fn step<
    A: ModelProviderAdapter,
    O: ContractFaultObserver,
    T: ToolDestinationAdapter,
    E: EmbeddingProviderAdapter,
>(
    store: &impl WorkerStore,
    destination: &ModelDestination<A, O, T, E>,
    index: &mut Option<ClaimedPassageIndex>,
) -> bool {
    match claim_next_work::<A>(store, Retry::EachFailure).await {
        Ok(NextWork::Export(ClaimedExportWork::Readable(claim))) => matches!(
            readable_export_claim(store, &claim, Retry::EachFailure).await,
            Ok(_) | Err(CompleteReadableExportError::StaleFence)
        ),
        Ok(NextWork::Export(ClaimedExportWork::Archive(claim))) => matches!(
            archive_export_claim(store, &claim, Retry::EachFailure).await,
            Ok(_) | Err(CompleteArchiveExportError::StaleFence)
        ),
        Ok(NextWork::AgentRun(claim)) => matches!(
            agent_run_claim(store, destination, &claim, Retry::EachFailure).await,
            Ok(_) | Err(CompleteAgentRunError::StaleFence)
        ),
        Ok(NextWork::MemoryWork) => true,
        Ok(NextWork::None) => {
            match passage_index_step(store, &destination.embeddings, index).await {
                Ok(worked) => worked,
                Err(error) => claim_failed(&error),
            }
        }
        Err(error) => claim_failed(&error),
    }
}

/// The index work comes after each other claim, also after a schedule row of background Memory
/// work (ADR 0095 section 4). The step does the next step of the held claim, or it claims the next
/// work and does its first step. The claim stays held while its steps write rows, so each turn of
/// the loop sends at most one embedding request, and the next turn claims the other work first.
/// Returns whether it did a step.
async fn passage_index_step<E: EmbeddingProviderAdapter>(
    store: &impl WorkerStore,
    embeddings: &E,
    held: &mut Option<ClaimedPassageIndex>,
) -> Result<bool, ProjectReadError> {
    let claim = match held.take() {
        Some(claim) => claim,
        None => match claim_next_passage_index_work(store, E::REGISTRATIONS).await? {
            Some(claim) => claim,
            None => return Ok(false),
        },
    };
    let step = step_passage_index_work(store, embeddings, E::REGISTRATIONS, &claim).await?;
    if step == PassageIndexStep::Written {
        *held = Some(claim);
    }
    Ok(true)
}

/// Claims one due schedule row of background Memory work after the AgentRun claim found no Run
/// (ADR 0094 section 5). The step asks for a due row first, so that an idle step makes no claim
/// span. `true`: a row was claimed.
async fn memory_work_claim(store: &impl WorkerStore) -> Result<bool, ProjectReadError> {
    if !store.memory_work_due().await? {
        return Ok(false);
    }
    memory_work_claim_span(store).await
}

/// The claim span of one schedule row (ADR 0047). The store decides and starts the capture or the
/// pass in the transaction of the claim, so the span has no completion attempt and no fence token.
#[tracing::instrument(skip_all, fields(
    project_id = tracing::field::Empty,
    kind = tracing::field::Empty,
    run_id = tracing::field::Empty,
    outcome = tracing::field::Empty,
))]
async fn memory_work_claim_span(store: &impl WorkerStore) -> Result<bool, ProjectReadError> {
    let span = tracing::Span::current();
    let claimed = match store.claim_due_memory_work().await {
        Ok(Some(claimed)) => claimed,
        Ok(None) => {
            span.record("outcome", "no_due_row");
            return Ok(false);
        }
        Err(error) => {
            span.record("outcome", error.diagnostic());
            return Err(error);
        }
    };
    span.record("project_id", claimed.project_scope.project_id.diagnostic());
    span.record("kind", claimed.source.kind());
    let run_id = match &claimed.outcome {
        MemoryWorkOutcome::Started(started) => Some(&started.run_id),
        MemoryWorkOutcome::PassStarted(started) => Some(&started.run_id),
        MemoryWorkOutcome::IntentPassStarted(started) => Some(&started.run_id),
        MemoryWorkOutcome::Skipped(_)
        | MemoryWorkOutcome::PassSkipped(_)
        | MemoryWorkOutcome::ChapterSkipped(_)
        | MemoryWorkOutcome::IntentPassSkipped(_) => None,
    };
    if let Some(run_id) = run_id {
        span.record("run_id", DiagnosticId(run_id).diagnostic());
    }
    span.record("outcome", claimed.outcome.diagnostic());
    Ok(true)
}

/// Runs one claim again after a serialization failure, as the loop does, for at most `ATTEMPTS`
/// attempts.
async fn claim_after_contention<T, F>(mut claim: impl FnMut() -> F) -> Result<T, ProjectReadError>
where
    F: Future<Output = Result<T, ProjectReadError>>,
{
    let mut attempt = 1;
    loop {
        match claim().await {
            Err(error)
                if attempt < ATTEMPTS
                    && SqlState(&error).diagnostic().as_deref() == Some(SERIALIZATION_FAILURE) =>
            {
                claim_failed(&error);
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            result => return result,
        }
        attempt += 1;
    }
}

fn claim_failed(error: &ProjectReadError) -> bool {
    tracing::warn!(
        reason = error.diagnostic(),
        sql_state = SqlState(error).diagnostic(),
        "Worker claim failed"
    );
    false
}

#[tracing::instrument(skip_all, fields(
    project_id = claim.project_scope.project_id.diagnostic(),
    export_id = DiagnosticId(&claim.export_id).diagnostic(),
    fence_token = claim.fence_token.diagnostic(),
    attempt = tracing::field::Empty,
    outcome = tracing::field::Empty,
))]
async fn readable_export_claim(
    store: &impl WorkerStore,
    claim: &ClaimedReadableExport,
    policy: Retry,
) -> Result<CompleteReadableExport, CompleteReadableExportError> {
    retry(
        policy,
        || complete_readable_export(store, claim),
        |error| matches!(error, CompleteReadableExportError::StaleFence),
    )
    .await
}

#[tracing::instrument(skip_all, fields(
    project_id = claim.project_scope.project_id.diagnostic(),
    export_id = DiagnosticId(&claim.export_id).diagnostic(),
    fence_token = claim.fence_token.diagnostic(),
    attempt = tracing::field::Empty,
    outcome = tracing::field::Empty,
))]
async fn archive_export_claim(
    store: &impl WorkerStore,
    claim: &ClaimedArchiveExport,
    policy: Retry,
) -> Result<CompleteArchiveExport, CompleteArchiveExportError> {
    retry(
        policy,
        || complete_archive_export(store, claim),
        |error| matches!(error, CompleteArchiveExportError::StaleFence),
    )
    .await
}

#[tracing::instrument(skip_all, fields(
    project_id = claim.project_scope.project_id.diagnostic(),
    run_id = DiagnosticId(&claim.run_id).diagnostic(),
    fence_token = claim.fence_token.diagnostic(),
    attempt = tracing::field::Empty,
    outcome = tracing::field::Empty,
))]
async fn agent_run_claim<
    A: ModelProviderAdapter,
    O: ContractFaultObserver,
    T: ToolDestinationAdapter,
    E: EmbeddingProviderAdapter,
>(
    store: &impl WorkerStore,
    destination: &ModelDestination<A, O, T, E>,
    claim: &ClaimedAgentRun,
    policy: Retry,
) -> Result<CompleteAgentRun, CompleteAgentRunError> {
    retry(
        policy,
        || {
            complete_agent_run_with_destinations(
                store,
                &destination.adapter,
                &destination.tools,
                &destination.embeddings,
                &destination.observer,
                &RuntimeClock,
                claim,
            )
        },
        |error| matches!(error, CompleteAgentRunError::StaleFence),
    )
    .await
}

/// The Model Gateway waits on the runtime timer.
struct RuntimeClock;

impl GatewayClock for RuntimeClock {
    async fn wait(&self, delay: Duration) {
        tokio::time::sleep(delay).await;
    }
}

/// The failed completions that the Worker runs again, for at most `ATTEMPTS` attempts.
#[derive(Clone, Copy)]
enum Retry {
    /// The loop runs each failed completion again.
    EachFailure,
    /// `run_once` runs only a serialization failure again, so that it settles a contended claim as
    /// the loop does.
    SerializationFailure,
}

/// Runs the completion until it succeeds, the fence is stale, or the policy stops the attempts.
/// Records the attempt and the outcome on the current claim span.
async fn retry<T, E, F>(
    policy: Retry,
    mut complete: impl FnMut() -> F,
    stale: impl Fn(&E) -> bool,
) -> Result<T, E>
where
    T: DiagnosticField,
    E: DiagnosticField + std::error::Error + 'static,
    F: Future<Output = Result<T, E>>,
{
    let mut attempt = 1;
    loop {
        tracing::Span::current().record("attempt", attempt.diagnostic());
        let result = complete().await;
        match &result {
            Ok(completed) => {
                tracing::Span::current().record("outcome", completed.diagnostic());
                return result;
            }
            Err(error) if stale(error) => {
                tracing::Span::current().record("outcome", error.diagnostic());
                return result;
            }
            Err(error) => {
                tracing::warn!(
                    attempt = attempt.diagnostic(),
                    reason = error.diagnostic(),
                    sql_state = SqlState(error).diagnostic(),
                    "Worker attempt failed"
                );
                let again = attempt < ATTEMPTS
                    && match policy {
                        Retry::EachFailure => true,
                        Retry::SerializationFailure => {
                            SqlState(error).diagnostic().as_deref() == Some(SERIALIZATION_FAILURE)
                        }
                    };
                if !again {
                    tracing::Span::current().record("outcome", error.diagnostic());
                    return result;
                }
            }
        }
        attempt += 1;
    }
}

#[cfg(test)]
#[path = "run_once_tests.rs"]
mod tests;
