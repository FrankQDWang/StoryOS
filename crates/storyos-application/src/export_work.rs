use std::future::Future;

use crate::ProjectReadError;
use crate::project_export_work::{ArchiveExportWorkStore, ClaimedArchiveExport};
use crate::readable_export_work::{ClaimedReadableExport, ReadableExportWorkStore};

/// One fenced Worker claim of admitted export work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClaimedExportWork {
    Readable(ClaimedReadableExport),
    Archive(ClaimedArchiveExport),
}

/// Claims the next admitted export work in readable-first order.
///
/// Implementations must use the same persistence adapter as other Project
/// commands. They must not treat Worker SQL as a private domain store.
/// Settlement stays on the existing readable and Archive owners.
pub trait ExportWorkStore: ReadableExportWorkStore + ArchiveExportWorkStore {
    fn claim_next_export_work(
        &self,
    ) -> impl Future<Output = Result<Option<ClaimedExportWork>, ProjectReadError>> + Send;
}

pub async fn claim_next_export_work(
    store: &impl ExportWorkStore,
) -> Result<Option<ClaimedExportWork>, ProjectReadError> {
    store.claim_next_export_work().await
}
