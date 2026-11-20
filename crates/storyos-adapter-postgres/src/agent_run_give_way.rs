//! Background work gives way to the Runs of the author (ADR 0094 section 5). Between two
//! RunSteps, a background Run gives its claim back when a Run of the author that this Worker can
//! claim is queued. Core decides it (`storyos_core::yields_to_author`).

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{AgentRunKind, ModelAdapter};

use super::PostgresProjectReader;
use crate::agent_run_work::{complete_unavailable, update_run};

impl PostgresProjectReader {
    /// Whether a queued Run of the author waits for a Worker with `adapters`: its Model
    /// Registration binds the model kind of one of them. The Runs that another claim locked do
    /// not count.
    pub(crate) async fn author_run_queued(
        &self,
        adapters: &[ModelAdapter],
    ) -> Result<bool, CompleteAgentRunError> {
        let unavailable = |error| CompleteAgentRunError::Unavailable(Box::new(error));
        let model_kinds = adapters
            .iter()
            .map(|adapter| adapter.kind())
            .collect::<Vec<_>>();
        let client = self.connect().await.map_err(unavailable)?;
        client
            .batch_execute("BEGIN")
            .await
            .map_err(complete_unavailable)?;
        client
            .execute(crate::export_work::WORKER_SCOPE, &[])
            .await
            .map_err(complete_unavailable)?;
        let queued = client
            .query_opt(
                "SELECT 1
                   FROM storyos.agent_runs AS author
                   JOIN storyos.model_registration_revisions AS registration
                     ON registration.model_registration_revision = author.model_registration_revision
                  WHERE author.run_kind = $1 AND author.status = 'queued'
                    AND registration.model_kind = ANY($2::text[])
                  LIMIT 1
                    FOR UPDATE OF author SKIP LOCKED",
                &[&AgentRunKind::Author.name(), &model_kinds],
            )
            .await
            .map_err(complete_unavailable)?
            .is_some();
        client
            .batch_execute("COMMIT")
            .await
            .map_err(complete_unavailable)?;
        Ok(queued)
    }

    /// Gives the claim of a background Run back to the queue between two RunSteps. The next
    /// claim continues the same RunStep.
    pub(crate) async fn give_back(
        &self,
        claim: &ClaimedAgentRun,
    ) -> Result<(), CompleteAgentRunError> {
        let transaction = self
            .begin_serializable_project_command_transaction(&claim.project_scope)
            .await
            .map_err(complete_unavailable)?;
        let given = update_run(
            &transaction.client,
            claim,
            "queued",
            /*settlement*/ None,
            /*clear_lease*/ true,
        )
        .await;
        match given {
            Ok(()) => transaction.commit().await.map_err(complete_unavailable),
            Err(error) => {
                let _rollback = transaction.rollback().await;
                Err(error)
            }
        }
    }
}
