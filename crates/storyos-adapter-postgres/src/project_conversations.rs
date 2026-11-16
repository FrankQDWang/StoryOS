//! The reads of the Project Conversations of a Project (ADR 0094 section 10). Each read writes no
//! row.

use std::time::{Duration, SystemTime};

use storyos_application::{
    ProjectConversationReader, ProjectConversationRunItem, ProjectReadError, ProjectScope,
    StoredProjectConversation, StoredProjectConversationRuns,
};
use storyos_core::AgentRunStatus;

use super::{PostgresProjectReader, read_error, set_scope};

fn status(token: &str) -> Result<AgentRunStatus, ProjectReadError> {
    AgentRunStatus::parse(token).ok_or_else(|| {
        ProjectReadError::unavailable(std::io::Error::other("the AgentRun status is invalid"))
    })
}

fn clock(micros: i64) -> Result<SystemTime, ProjectReadError> {
    let micros = u64::try_from(micros).map_err(ProjectReadError::unavailable)?;
    Ok(SystemTime::UNIX_EPOCH + Duration::from_micros(micros))
}

impl ProjectConversationReader for PostgresProjectReader {
    async fn read_project_conversations(
        &self,
        scope: &ProjectScope,
        limit: usize,
    ) -> Result<Option<Vec<StoredProjectConversation>>, ProjectReadError> {
        let limit = i64::try_from(limit).map_err(ProjectReadError::unavailable)?;
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        let project = transaction
            .query_opt(
                "SELECT 1 FROM storyos.projects
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .map_err(read_error)?;
        if project.is_none() {
            transaction.commit().await.map_err(read_error)?;
            return Ok(None);
        }
        // The first Run and the newest Run are the first and the last Run in Run identity order.
        let rows = transaction
            .query(
                "SELECT conversation_id, first_author_message, status,
                        to_char(first_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
                          AS first_run_created_at,
                        to_char(newest_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
                          AS newest_run_created_at
                   FROM (SELECT run.conversation_id::text AS conversation_id,
                                (array_agg(run.author_message ORDER BY run.run_id))[1]
                                  AS first_author_message,
                                (array_agg(run.status::text ORDER BY run.run_id DESC))[1] AS status,
                                (array_agg(receipt.created_at ORDER BY run.run_id))[1] AS first_at,
                                (array_agg(receipt.created_at ORDER BY run.run_id DESC))[1] AS newest_at
                           FROM storyos.agent_runs AS run
                           JOIN storyos.domain_receipts AS receipt
                             ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                                (run.owner_user_id, run.project_id, run.receipt_id)
                          WHERE run.owner_user_id = $1::text::uuid
                            AND run.project_id = $2::text::uuid
                          GROUP BY run.conversation_id) AS conversation
                  ORDER BY newest_at DESC, conversation_id DESC
                  LIMIT $3",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &limit,
                ],
            )
            .await
            .map_err(read_error)?;
        transaction.commit().await.map_err(read_error)?;
        rows.iter()
            .map(|row| {
                Ok(StoredProjectConversation {
                    conversation_id: row.get("conversation_id"),
                    first_author_message: row.get("first_author_message"),
                    first_run_created_at: row.get("first_run_created_at"),
                    newest_run_created_at: row.get("newest_run_created_at"),
                    newest_run_status: status(row.get("status"))?,
                })
            })
            .collect::<Result<_, _>>()
            .map(Some)
    }

    async fn read_project_conversation(
        &self,
        scope: &ProjectScope,
        conversation_id: &str,
        limit: usize,
    ) -> Result<Option<StoredProjectConversationRuns>, ProjectReadError> {
        let limit = i64::try_from(limit).map_err(ProjectReadError::unavailable)?;
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        let conversation = transaction
            .query_opt(
                "SELECT 1 FROM storyos.project_conversations
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND conversation_id = $3::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &conversation_id,
                ],
            )
            .await
            .map_err(read_error)?;
        if conversation.is_none() {
            transaction.commit().await.map_err(read_error)?;
            return Ok(None);
        }
        // The end time of an ended Run is the recorded time of its last Run event, or the time of
        // its Receipt when it has no Run event. The read time is the clock of the same database.
        let rows = transaction
            .query(
                "SELECT run.run_id::text AS run_id, run.author_message,
                        run.chapter_id::text AS chapter_id, run.status::text AS status,
                        to_char(receipt.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"') AS created_at,
                        floor(extract(epoch FROM GREATEST(receipt.created_at,
                          (SELECT max(event.recorded_at) FROM storyos.agent_run_events AS event
                            WHERE (event.owner_user_id, event.project_id, event.run_id) =
                                  (run.owner_user_id, run.project_id, run.run_id))))
                          * 1000000)::bigint AS last_recorded_at,
                        floor(extract(epoch FROM statement_timestamp()) * 1000000)::bigint AS read_time
                   FROM storyos.agent_runs AS run
                   JOIN storyos.domain_receipts AS receipt
                     ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                        (run.owner_user_id, run.project_id, run.receipt_id)
                  WHERE run.owner_user_id = $1::text::uuid AND run.project_id = $2::text::uuid
                    AND run.conversation_id = $3::text::uuid
                  ORDER BY run.run_id DESC
                  LIMIT $4",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &conversation_id,
                    &limit,
                ],
            )
            .await
            .map_err(read_error)?;
        transaction.commit().await.map_err(read_error)?;
        let Some(newest) = rows.first() else {
            return Err(ProjectReadError::unavailable(std::io::Error::other(
                "the Project Conversation has no Run",
            )));
        };
        let runs = rows.iter().map(|row| {
            Ok(ProjectConversationRunItem {
                run_id: row.get("run_id"),
                author_message: row.get("author_message"),
                chapter_id: row.get("chapter_id"),
                status: status(row.get("status"))?,
                created_at: row.get("created_at"),
            })
        });
        Ok(Some(StoredProjectConversationRuns {
            newest_first: runs.collect::<Result<_, ProjectReadError>>()?,
            newest_run_last_recorded_at: clock(newest.get("last_recorded_at"))?,
            read_time: clock(newest.get("read_time"))?,
        }))
    }
}

#[cfg(test)]
#[path = "project_conversations_tests.rs"]
mod tests;
