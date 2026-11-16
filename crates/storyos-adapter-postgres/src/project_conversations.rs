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
                "SELECT conversation.conversation_id::text AS conversation_id,
                        first_run.author_message AS first_author_message,
                        to_char(first_run.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"') AS first_run_created_at,
                        to_char(newest_run.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"') AS newest_run_created_at,
                        newest_run.status
                   FROM storyos.project_conversations AS conversation
                  CROSS JOIN LATERAL (
                        SELECT run.author_message, receipt.created_at
                          FROM storyos.agent_runs AS run
                          JOIN storyos.domain_receipts AS receipt
                            ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                               (run.owner_user_id, run.project_id, run.receipt_id)
                         WHERE (run.owner_user_id, run.project_id, run.conversation_id) =
                               (conversation.owner_user_id, conversation.project_id,
                                conversation.conversation_id)
                         ORDER BY run.run_id
                         LIMIT 1) AS first_run
                  CROSS JOIN LATERAL (
                        SELECT run.status::text AS status, receipt.created_at
                          FROM storyos.agent_runs AS run
                          JOIN storyos.domain_receipts AS receipt
                            ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                               (run.owner_user_id, run.project_id, run.receipt_id)
                         WHERE (run.owner_user_id, run.project_id, run.conversation_id) =
                               (conversation.owner_user_id, conversation.project_id,
                                conversation.conversation_id)
                         ORDER BY run.run_id DESC
                         LIMIT 1) AS newest_run
                  WHERE conversation.owner_user_id = $1::text::uuid
                    AND conversation.project_id = $2::text::uuid
                  ORDER BY newest_run.created_at DESC, conversation.conversation_id DESC
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
        let rows = transaction
            .query(
                "SELECT run.run_id::text AS run_id, run.author_message,
                        run.chapter_id::text AS chapter_id, run.status::text AS status,
                        to_char(receipt.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"') AS created_at
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
        let newest_first = rows
            .iter()
            .map(|row| {
                Ok(ProjectConversationRunItem {
                    run_id: row.get("run_id"),
                    author_message: row.get("author_message"),
                    chapter_id: row.get("chapter_id"),
                    status: status(row.get("status"))?,
                    created_at: row.get("created_at"),
                })
            })
            .collect::<Result<Vec<_>, ProjectReadError>>()?;
        let Some(newest) = newest_first.first() else {
            return Err(ProjectReadError::unavailable(std::io::Error::other(
                "the Project Conversation has no Run",
            )));
        };
        // The end time of an ended Run is the recorded time of its last Run event, or the time of
        // its Receipt when it has no Run event. The read time is the clock of the same database.
        let times = transaction
            .query_one(
                "SELECT floor(extract(epoch FROM GREATEST(receipt.created_at,
                          (SELECT max(event.recorded_at) FROM storyos.agent_run_events AS event
                            WHERE (event.owner_user_id, event.project_id, event.run_id) =
                                  (run.owner_user_id, run.project_id, run.run_id))))
                          * 1000000)::bigint,
                        floor(extract(epoch FROM clock_timestamp()) * 1000000)::bigint
                   FROM storyos.agent_runs AS run
                   JOIN storyos.domain_receipts AS receipt
                     ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                        (run.owner_user_id, run.project_id, run.receipt_id)
                  WHERE run.owner_user_id = $1::text::uuid AND run.project_id = $2::text::uuid
                    AND run.run_id = $3::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &newest.run_id,
                ],
            )
            .await
            .map_err(read_error)?;
        transaction.commit().await.map_err(read_error)?;
        Ok(Some(StoredProjectConversationRuns {
            newest_run_last_recorded_at: clock(times.get(/*idx*/ 0))?,
            read_time: clock(times.get(/*idx*/ 1))?,
            newest_first,
        }))
    }
}

#[cfg(test)]
#[path = "project_conversations_tests.rs"]
mod tests;
