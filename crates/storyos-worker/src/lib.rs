//! The StoryOS Worker loop. Composition roots supply the store, the adapter, and the observer.

use std::future::Future;
use std::time::Duration;

use storyos_application::{
    AgentRunWorkStore, ClaimedAgentRun, ClaimedArchiveExport, ClaimedExportWork,
    ClaimedReadableExport, CompleteAgentRun, CompleteAgentRunError, CompleteArchiveExport,
    CompleteArchiveExportError, CompleteReadableExport, CompleteReadableExportError,
    ContractFaultObserver, DiagnosticField, DiagnosticId, ExportWorkStore, ModelDispatchStore,
    ModelProviderAdapter, ProjectReadError, SqlState, claim_next_agent_run, claim_next_export_work,
    complete_agent_run, complete_archive_export, complete_readable_export,
};

const ATTEMPTS: u64 = 4;

/// Combined persistence port for the packaged Worker loop.
pub trait WorkerStore: ExportWorkStore + AgentRunWorkStore + ModelDispatchStore {}

impl<T> WorkerStore for T where T: ExportWorkStore + AgentRunWorkStore + ModelDispatchStore {}

/// The destination side of the Worker loop: one Model Provider Adapter and its fault observer.
pub struct ModelDestination<A, O> {
    pub adapter: A,
    pub observer: O,
}

pub fn in_process_loop_enabled() -> bool {
    std::env::var("STORYOS_WORKER").ok().as_deref() != Some("0")
}

pub fn readable_export_lease_ttl_from_env() -> Duration {
    std::env::var("STORYOS_EXPORT_LEASE_TTL_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(30))
}

pub async fn run<A: ModelProviderAdapter, O: ContractFaultObserver>(
    store: impl WorkerStore,
    destination: ModelDestination<A, O>,
) {
    loop {
        if !step(&store, &destination).await {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

pub async fn run_once<A: ModelProviderAdapter, O: ContractFaultObserver>(
    store: &impl WorkerStore,
    destination: &ModelDestination<A, O>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match claim_next_export_work(store).await? {
        Some(ClaimedExportWork::Readable(claim)) => {
            readable_export_claim(store, &claim, /*attempts*/ 1).await?;
            Ok(())
        }
        Some(ClaimedExportWork::Archive(claim)) => {
            archive_export_claim(store, &claim, /*attempts*/ 1).await?;
            Ok(())
        }
        None => match claim_next_agent_run(store).await? {
            Some(claim) => {
                match agent_run_claim(store, destination, &claim, /*attempts*/ 1).await {
                    Ok(_) | Err(CompleteAgentRunError::StaleFence) => Ok(()),
                    Err(error) => Err(error.into()),
                }
            }
            None => Ok(()),
        },
    }
}

pub async fn claim_only(
    store: &impl WorkerStore,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if claim_next_export_work(store).await?.is_some() {
        return Ok(());
    }
    claim_next_agent_run(store).await?;
    Ok(())
}

async fn step<A: ModelProviderAdapter, O: ContractFaultObserver>(
    store: &impl WorkerStore,
    destination: &ModelDestination<A, O>,
) -> bool {
    match claim_next_export_work(store).await {
        Ok(Some(ClaimedExportWork::Readable(claim))) => matches!(
            readable_export_claim(store, &claim, ATTEMPTS).await,
            Ok(_) | Err(CompleteReadableExportError::StaleFence)
        ),
        Ok(Some(ClaimedExportWork::Archive(claim))) => matches!(
            archive_export_claim(store, &claim, ATTEMPTS).await,
            Ok(_) | Err(CompleteArchiveExportError::StaleFence)
        ),
        Ok(None) => match claim_next_agent_run(store).await {
            Ok(Some(claim)) => matches!(
                agent_run_claim(store, destination, &claim, ATTEMPTS).await,
                Ok(_) | Err(CompleteAgentRunError::StaleFence)
            ),
            Ok(None) => false,
            Err(error) => claim_failed(&error),
        },
        Err(error) => claim_failed(&error),
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
    attempts: u64,
) -> Result<CompleteReadableExport, CompleteReadableExportError> {
    retry(
        attempts,
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
    attempts: u64,
) -> Result<CompleteArchiveExport, CompleteArchiveExportError> {
    retry(
        attempts,
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
async fn agent_run_claim<A: ModelProviderAdapter, O: ContractFaultObserver>(
    store: &impl WorkerStore,
    destination: &ModelDestination<A, O>,
    claim: &ClaimedAgentRun,
    attempts: u64,
) -> Result<CompleteAgentRun, CompleteAgentRunError> {
    retry(
        attempts,
        || complete_agent_run(store, &destination.adapter, &destination.observer, claim),
        |error| matches!(error, CompleteAgentRunError::StaleFence),
    )
    .await
}

/// Runs the completion until it succeeds, the fence is stale, or the attempts end.
/// Records the attempt and the outcome on the current claim span.
async fn retry<T, E, F>(
    attempts: u64,
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
                if attempt == attempts {
                    tracing::Span::current().record("outcome", error.diagnostic());
                    return result;
                }
            }
        }
        attempt += 1;
    }
}
