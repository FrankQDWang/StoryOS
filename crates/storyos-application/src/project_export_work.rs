use std::future::Future;

use crate::{ProjectReadError, ProjectScope};

/// One fenced Worker claim of an admitted Project Export Archive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimedArchiveExport {
    pub project_scope: ProjectScope,
    pub export_id: String,
    pub fence_token: i64,
    pub source_snapshot_id: String,
    pub author_command_admission_id: String,
    pub command_id: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompleteArchiveExport {
    SettledReady,
    SettledFailed,
    AlreadySettled,
}

#[derive(Debug)]
pub enum CompleteArchiveExportError {
    StaleFence,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for CompleteArchiveExportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaleFence => formatter.write_str("The Project Export Archive fence is stale"),
            Self::Unavailable(_) => {
                formatter.write_str("The Project Export Archive work store is unavailable")
            }
        }
    }
}

impl std::error::Error for CompleteArchiveExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::StaleFence => None,
        }
    }
}

impl CompleteArchiveExportError {
    pub fn unavailable(source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Unavailable(Box::new(source))
    }
}

/// Claims and settles admitted Project Export Archive work.
///
/// Implementations must use the same persistence adapter as other Project
/// commands. They must not treat Worker SQL as a private domain store.
pub trait ArchiveExportWorkStore: Sync {
    fn claim_next_archive_export(
        &self,
    ) -> impl Future<Output = Result<Option<ClaimedArchiveExport>, ProjectReadError>> + Send;

    fn complete_archive_export(
        &self,
        claim: &ClaimedArchiveExport,
    ) -> impl Future<Output = Result<CompleteArchiveExport, CompleteArchiveExportError>> + Send;
}

pub async fn claim_next_archive_export(
    store: &impl ArchiveExportWorkStore,
) -> Result<Option<ClaimedArchiveExport>, ProjectReadError> {
    store.claim_next_archive_export().await
}

pub async fn complete_archive_export(
    store: &impl ArchiveExportWorkStore,
    claim: &ClaimedArchiveExport,
) -> Result<CompleteArchiveExport, CompleteArchiveExportError> {
    store.complete_archive_export(claim).await
}
