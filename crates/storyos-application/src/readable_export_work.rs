use std::future::Future;

use crate::{ProjectReadError, ProjectScope};

/// One fenced Worker claim of an admitted human-readable export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimedReadableExport {
    pub project_scope: ProjectScope,
    pub export_id: String,
    pub fence_token: i64,
    pub source_snapshot_id: String,
    pub author_command_admission_id: String,
    pub command_id: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompleteReadableExport {
    SettledReady,
    SettledFailed,
    AlreadySettled,
}

#[derive(Debug)]
pub enum CompleteReadableExportError {
    StaleFence,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for CompleteReadableExportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaleFence => formatter.write_str("The human-readable export fence is stale"),
            Self::Unavailable(_) => {
                formatter.write_str("The human-readable export work store is unavailable")
            }
        }
    }
}

impl std::error::Error for CompleteReadableExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::StaleFence => None,
        }
    }
}

impl CompleteReadableExportError {
    pub fn unavailable(source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Unavailable(Box::new(source))
    }
}

/// Claims and settles admitted human-readable export work.
///
/// Implementations must use the same persistence adapter as other Project
/// commands. They must not treat Worker SQL as a private domain store.
pub trait ReadableExportWorkStore: Sync {
    fn claim_next_readable_export(
        &self,
    ) -> impl Future<Output = Result<Option<ClaimedReadableExport>, ProjectReadError>> + Send;

    fn complete_readable_export(
        &self,
        claim: &ClaimedReadableExport,
    ) -> impl Future<Output = Result<CompleteReadableExport, CompleteReadableExportError>> + Send;
}

pub async fn claim_next_readable_export(
    store: &impl ReadableExportWorkStore,
) -> Result<Option<ClaimedReadableExport>, ProjectReadError> {
    store.claim_next_readable_export().await
}

pub async fn complete_readable_export(
    store: &impl ReadableExportWorkStore,
    claim: &ClaimedReadableExport,
) -> Result<CompleteReadableExport, CompleteReadableExportError> {
    store.complete_readable_export(claim).await
}
