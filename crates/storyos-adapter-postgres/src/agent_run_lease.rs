//! The renewal of the Run Lease of a claimed AgentRun while its destination request is in progress.

use std::time::Duration;

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};

use crate::PostgresProjectReader;
use crate::agent_run_work::{complete_challenge_error, complete_database_error};

/// The renewals in each lease period. A renewal that is late by up to two delays keeps the lease.
const RENEWALS_PER_LEASE: u32 = 3;

impl PostgresProjectReader {
    /// Extends the lease of the fenced claim by one lease period from now. The transaction is read
    /// committed, so that it is never a part of a serialization cycle (ADR 0058).
    pub(crate) async fn renew_agent_run_lease_row(
        &self,
        claim: &ClaimedAgentRun,
    ) -> Result<Option<Duration>, CompleteAgentRunError> {
        let lease_seconds = i64::try_from(self.readable_export_lease_ttl.as_secs())
            .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
        let transaction = self
            .begin_project_command_transaction(&claim.project_scope)
            .await
            .map_err(complete_challenge_error)?;
        let renewed = transaction
            .client
            .execute(
                "UPDATE storyos.agent_runs
                    SET lease_expires_at = clock_timestamp() + ($5::bigint * interval '1 second')
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND run_id = $3::text::uuid
                    AND fence_token = $4
                    AND status IN ('claimed', 'cancelled')
                    AND lease_expires_at IS NOT NULL",
                &[
                    &claim.project_scope.owner_user_id.as_ref(),
                    &claim.project_scope.project_id.as_ref(),
                    &claim.run_id,
                    &claim.fence_token,
                    &lease_seconds,
                ],
            )
            .await
            .map_err(complete_database_error);
        let renewed = match renewed {
            Ok(renewed) => renewed,
            Err(error) => {
                let _rollback = transaction.rollback().await;
                return Err(error);
            }
        };
        transaction
            .commit()
            .await
            .map_err(complete_challenge_error)?;
        Ok((renewed == 1).then(|| self.readable_export_lease_ttl / RENEWALS_PER_LEASE))
    }
}

#[cfg(test)]
#[path = "agent_run_lease_tests.rs"]
mod tests;
