use storyos_application::{ClaimedExportWork, ExportWorkStore, ProjectReadError};

use super::*;

impl ExportWorkStore for PostgresProjectReader {
    async fn claim_next_export_work(&self) -> Result<Option<ClaimedExportWork>, ProjectReadError> {
        crate::require_release1_storage_activation_proof(&self.database_url)
            .await
            .map_err(ProjectReadError::unavailable)?;
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_worker_scope(&transaction).await?;
        let lease_seconds = i64::try_from(self.readable_export_lease_ttl.as_secs())
            .map_err(ProjectReadError::unavailable)?;
        if let Some(claim) =
            crate::readable_export_work::claim_readable_export_row(&transaction, lease_seconds)
                .await?
        {
            transaction.commit().await.map_err(read_error)?;
            return Ok(Some(ClaimedExportWork::Readable(claim)));
        }
        let claimed =
            crate::project_export_work::claim_archive_export_row(&transaction, lease_seconds)
                .await?;
        transaction.commit().await.map_err(read_error)?;
        Ok(claimed.map(ClaimedExportWork::Archive))
    }
}

pub(crate) async fn set_worker_scope(
    transaction: &tokio_postgres::Transaction<'_>,
) -> Result<(), ProjectReadError> {
    transaction
        .execute(
            "SELECT set_config('storyos.scope_mode', 'worker', true),
                    set_config('storyos.owner_user_id', '', true),
                    set_config('storyos.project_id', '', true),
                    set_config('storyos.user_id', '', true)",
            &[],
        )
        .await
        .map(|_| ())
        .map_err(read_error)
}

#[cfg(test)]
#[path = "export_work_tests.rs"]
mod tests;
