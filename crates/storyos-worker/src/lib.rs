//! StoryOS Worker composition root.

use std::time::Duration;

use storyos_application::{
    ClaimedExportWork, CompleteArchiveExportError, CompleteReadableExportError, ExportWorkStore,
    claim_next_export_work, complete_archive_export, complete_readable_export,
};

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

pub async fn run(store: impl ExportWorkStore) {
    loop {
        if !step(&store).await {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

pub async fn run_once(
    store: &impl ExportWorkStore,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match claim_next_export_work(store).await {
        Ok(Some(ClaimedExportWork::Readable(claim))) => {
            complete_readable_export(store, &claim).await?;
            Ok(())
        }
        Ok(Some(ClaimedExportWork::Archive(claim))) => {
            complete_archive_export(store, &claim).await?;
            Ok(())
        }
        Ok(None) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub async fn claim_only(
    store: &impl ExportWorkStore,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    claim_next_export_work(store).await?;
    Ok(())
}

async fn step(store: &impl ExportWorkStore) -> bool {
    match claim_next_export_work(store).await {
        Ok(Some(ClaimedExportWork::Readable(claim))) => {
            for _ in 0..4 {
                match complete_readable_export(store, &claim).await {
                    Ok(_) | Err(CompleteReadableExportError::StaleFence) => return true,
                    Err(_) => {}
                }
            }
            false
        }
        Ok(Some(ClaimedExportWork::Archive(claim))) => {
            for _ in 0..4 {
                match complete_archive_export(store, &claim).await {
                    Ok(_) | Err(CompleteArchiveExportError::StaleFence) => return true,
                    Err(_) => {}
                }
            }
            false
        }
        Ok(None) | Err(_) => false,
    }
}
