//! The StoryOS Worker loop. Composition roots supply the store, the adapter, and the observer.

use std::future::Future;
use std::time::Duration;

use storyos_application::{
    AgentRunWorkStore, ClaimedAgentRun, ClaimedArchiveExport, ClaimedExportWork,
    ClaimedReadableExport, CompleteAgentRun, CompleteAgentRunError, CompleteArchiveExport,
    CompleteArchiveExportError, CompleteReadableExport, CompleteReadableExportError,
    ContractFaultObserver, DiagnosticField, DiagnosticId, ExportWorkStore, GatewayClock,
    ModelDispatchStore, ModelProviderAdapter, ProjectReadError, SqlState, ToolDestinationAdapter,
    ToolDispatchStore, claim_next_agent_run, claim_next_export_work, complete_agent_run_with_tools,
    complete_archive_export, complete_readable_export,
};

const ATTEMPTS: u64 = 4;
const SERIALIZATION_FAILURE: &str = "40001";

/// Combined persistence port for the packaged Worker loop.
pub trait WorkerStore:
    ExportWorkStore + AgentRunWorkStore + ModelDispatchStore + ToolDispatchStore
{
}

impl<T> WorkerStore for T where
    T: ExportWorkStore + AgentRunWorkStore + ModelDispatchStore + ToolDispatchStore
{
}

/// The destination side of the Worker loop: one Model Provider Adapter, its fault observer, and
/// the Tool destination of the ToolDispatch phase.
pub struct ModelDestination<A, O, T> {
    pub adapter: A,
    pub observer: O,
    pub tools: T,
}

/// The lease of a readable export claim: the `STORYOS_EXPORT_LEASE_TTL_SECS` setting, or 30
/// seconds.
pub fn readable_export_lease_ttl(seconds: Option<u64>) -> Duration {
    Duration::from_secs(seconds.unwrap_or(/*default*/ 30))
}

pub async fn run<A: ModelProviderAdapter, O: ContractFaultObserver, T: ToolDestinationAdapter>(
    store: impl WorkerStore,
    destination: ModelDestination<A, O, T>,
) {
    loop {
        if !step(&store, &destination).await {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

pub async fn run_once<
    A: ModelProviderAdapter,
    O: ContractFaultObserver,
    T: ToolDestinationAdapter,
>(
    store: &impl WorkerStore,
    destination: &ModelDestination<A, O, T>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match claim_after_contention(|| claim_next_export_work(store)).await? {
        Some(ClaimedExportWork::Readable(claim)) => {
            readable_export_claim(store, &claim, Retry::SerializationFailure).await?;
            Ok(())
        }
        Some(ClaimedExportWork::Archive(claim)) => {
            archive_export_claim(store, &claim, Retry::SerializationFailure).await?;
            Ok(())
        }
        None => match claim_after_contention(|| claim_next_agent_run(store, A::ADAPTERS)).await? {
            Some(claim) => {
                match agent_run_claim(store, destination, &claim, Retry::SerializationFailure).await
                {
                    Ok(_) | Err(CompleteAgentRunError::StaleFence) => Ok(()),
                    Err(error) => Err(error.into()),
                }
            }
            None => Ok(()),
        },
    }
}

pub async fn claim_only<A: ModelProviderAdapter>(
    store: &impl WorkerStore,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if claim_next_export_work(store).await?.is_some() {
        return Ok(());
    }
    claim_next_agent_run(store, A::ADAPTERS).await?;
    Ok(())
}

async fn step<A: ModelProviderAdapter, O: ContractFaultObserver, T: ToolDestinationAdapter>(
    store: &impl WorkerStore,
    destination: &ModelDestination<A, O, T>,
) -> bool {
    match claim_next_export_work(store).await {
        Ok(Some(ClaimedExportWork::Readable(claim))) => matches!(
            readable_export_claim(store, &claim, Retry::EachFailure).await,
            Ok(_) | Err(CompleteReadableExportError::StaleFence)
        ),
        Ok(Some(ClaimedExportWork::Archive(claim))) => matches!(
            archive_export_claim(store, &claim, Retry::EachFailure).await,
            Ok(_) | Err(CompleteArchiveExportError::StaleFence)
        ),
        Ok(None) => match claim_next_agent_run(store, A::ADAPTERS).await {
            Ok(Some(claim)) => matches!(
                agent_run_claim(store, destination, &claim, Retry::EachFailure).await,
                Ok(_) | Err(CompleteAgentRunError::StaleFence)
            ),
            Ok(None) => false,
            Err(error) => claim_failed(&error),
        },
        Err(error) => claim_failed(&error),
    }
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
>(
    store: &impl WorkerStore,
    destination: &ModelDestination<A, O, T>,
    claim: &ClaimedAgentRun,
    policy: Retry,
) -> Result<CompleteAgentRun, CompleteAgentRunError> {
    retry(
        policy,
        || {
            complete_agent_run_with_tools(
                store,
                &destination.adapter,
                &destination.tools,
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
