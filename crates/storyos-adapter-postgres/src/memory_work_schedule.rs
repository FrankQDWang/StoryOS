//! The schedule of background Memory work in the store (ADR 0094 section 5, writer 2): the due
//! write in the transaction of the completion of a Run of the author, and the Worker claim of a
//! due row. The claim, the decision, the start of the capture, and the empty due time are one
//! transaction. Thus a Worker that stops before the commit leaves the row due, and a second
//! delivery starts no second capture for the same messages. No record of this work has message
//! text outside the input of the capture (ADR 0047).

use std::time::SystemTime;

use storyos_application::{
    ClaimedAgentRun, ClaimedMemoryWork, CompleteAgentRunError, MemoryCaptureSource,
    MemoryWorkOutcome, MemoryWorkScheduleStore, ProjectId, ProjectReadError, ProjectScope, UserId,
};
use storyos_core::{
    ConversationCaptureFacts, ConversationCaptureSkip, ConversationRole, ConversationRunMessages,
};
use tokio_postgres::Client;

use crate::agent_run_work::complete_database_error;
use crate::connection_pool::PooledClient;
use crate::{PostgresProjectReader, read_error};

/// The kind of the schedule row of a Project Conversation.
const CONVERSATION_CAPTURE: &str = "conversation_capture";

/// Sets the due time of the schedule row of the Conversation of the completed Run of the author
/// `claim`, in the transaction of its settlement. A background Run has no Conversation, so it
/// writes no row.
pub(crate) async fn mark_conversation_due(
    client: &Client,
    claim: &ClaimedAgentRun,
) -> Result<(), CompleteAgentRunError> {
    let settled_at: SystemTime = client
        .query_one("SELECT clock_timestamp()", &[])
        .await
        .map_err(complete_database_error)?
        .get(/*idx*/ 0);
    client
        .execute(
            "INSERT INTO storyos.memory_work_schedules
               (owner_user_id, project_id, work_kind, conversation_id, due_at)
             SELECT run.owner_user_id, run.project_id, $4, run.conversation_id, $5
               FROM storyos.agent_runs AS run
              WHERE (run.owner_user_id, run.project_id, run.run_id) =
                    ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                AND run.run_kind = 'author'
             ON CONFLICT (owner_user_id, project_id, work_kind, conversation_id)
             DO UPDATE SET due_at = EXCLUDED.due_at",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
                &CONVERSATION_CAPTURE,
                &storyos_core::conversation_capture_due(settled_at),
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(())
}

impl MemoryWorkScheduleStore for PostgresProjectReader {
    async fn memory_work_due(&self) -> Result<bool, ProjectReadError> {
        let client = self.connect().await?;
        client.batch_execute("BEGIN").await.map_err(read_error)?;
        set_worker_scope(&client).await?;
        let due = client
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM storyos.memory_work_schedules
                                 WHERE due_at <= clock_timestamp())",
                &[],
            )
            .await
            .map_err(read_error)?
            .get(/*idx*/ 0);
        client.batch_execute("COMMIT").await.map_err(read_error)?;
        Ok(due)
    }

    async fn claim_due_memory_work(&self) -> Result<Option<ClaimedMemoryWork>, ProjectReadError> {
        self.require_release1_storage_activation_proof()
            .await
            .map_err(ProjectReadError::unavailable)?;
        // The start reads the Memory head and its reference items in one snapshot, as
        // `start_memory_capture` does. A contended claim fails with a serialization failure.
        let client = self.connect().await?;
        client
            .batch_execute("BEGIN ISOLATION LEVEL SERIALIZABLE")
            .await
            .map_err(read_error)?;
        // A stop before the commit drops the connection, so the row stays due.
        let Some(claimed) = claim_and_decide(&client).await? else {
            client.batch_execute("COMMIT").await.map_err(read_error)?;
            return Ok(None);
        };
        client.batch_execute("COMMIT").await.map_err(read_error)?;
        Ok(Some(claimed))
    }
}

/// Sets the Worker scope in the open transaction of `client`.
async fn set_worker_scope(client: &PooledClient) -> Result<(), ProjectReadError> {
    client
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

/// Claims one due row in the Worker scope and makes its due time empty, then decides and starts
/// in the Project Scope of the row.
async fn claim_and_decide(
    client: &PooledClient,
) -> Result<Option<ClaimedMemoryWork>, ProjectReadError> {
    set_worker_scope(client).await?;
    let Some(row) = client
        .query_opt(
            "WITH next_work AS (
               SELECT owner_user_id, project_id, work_kind, conversation_id
                 FROM storyos.memory_work_schedules
                WHERE due_at <= clock_timestamp()
                ORDER BY due_at, owner_user_id, project_id, conversation_id
                FOR UPDATE SKIP LOCKED
                LIMIT 1
             )
             UPDATE storyos.memory_work_schedules AS work SET due_at = NULL
               FROM next_work
              WHERE (work.owner_user_id, work.project_id, work.work_kind, work.conversation_id) =
                    (next_work.owner_user_id, next_work.project_id, next_work.work_kind,
                     next_work.conversation_id)
          RETURNING work.owner_user_id::text, work.project_id::text, work.conversation_id::text",
            &[],
        )
        .await
        .map_err(read_error)?
    else {
        return Ok(None);
    };
    let scope = ProjectScope::new(
        UserId::new(row.get::<_, String>(/*idx*/ 0)),
        ProjectId::new(row.get::<_, String>(/*idx*/ 1)),
    );
    let conversation_id: String = row.get(/*idx*/ 2);
    client
        .execute(
            "SELECT set_config('storyos.scope_mode', '', true),
                    set_config('storyos.user_id', $1, true),
                    set_config('storyos.owner_user_id', $1, true),
                    set_config('storyos.project_id', $2, true)",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(read_error)?;
    let outcome = decide_and_start(client, &scope, &conversation_id).await?;
    Ok(Some(ClaimedMemoryWork {
        project_scope: scope,
        conversation_id,
        outcome,
    }))
}

async fn decide_and_start(
    client: &PooledClient,
    scope: &ProjectScope,
    conversation_id: &str,
) -> Result<MemoryWorkOutcome, ProjectReadError> {
    let keys: [&(dyn tokio_postgres::types::ToSql + Sync); 3] = [
        &scope.owner_user_id.as_ref(),
        &scope.project_id.as_ref(),
        &conversation_id,
    ];
    let row = client
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM storyos.agent_runs AS run
                             WHERE (run.owner_user_id, run.project_id, run.conversation_id) =
                                   ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                               AND run.run_kind = 'author'
                               AND run.status IN ('queued', 'claimed', 'waiting', 'paused')),
                    COALESCE((SELECT settings.contribution_enabled
                                FROM storyos.conversation_memory_settings AS settings
                               WHERE (settings.owner_user_id, settings.project_id,
                                      settings.conversation_id) =
                                     ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                                 AND COALESCE(settings.is_current, true)), false),
                    EXISTS (SELECT 1 FROM storyos.memory_captures AS capture
                             WHERE (capture.owner_user_id, capture.project_id,
                                    capture.conversation_id) =
                                   ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                               AND capture.state = 'running')",
            &keys,
        )
        .await
        .map_err(read_error)?;
    let runs = unread_runs(client, &keys).await?;
    let facts = ConversationCaptureFacts {
        author_run_unsettled: row.get(/*idx*/ 0),
        contribution_permitted: row.get(/*idx*/ 1),
        unread_runs: !runs.is_empty(),
        capture_in_progress: row.get(/*idx*/ 2),
    };
    if let Err(skip) = storyos_core::decide_conversation_capture(&facts) {
        return Ok(MemoryWorkOutcome::Skipped(skip));
    }
    let Some(input) = storyos_core::conversation_capture_input(&runs) else {
        return Ok(MemoryWorkOutcome::Skipped(
            ConversationCaptureSkip::NothingUnread,
        ));
    };
    let source = MemoryCaptureSource::Conversation {
        conversation_id: conversation_id.to_owned(),
        first_run_id: input.first_run_id,
        last_run_id: input.last_run_id,
    };
    // A refused start writes no row, and the empty due time stays.
    client
        .batch_execute("SAVEPOINT capture_start")
        .await
        .map_err(read_error)?;
    match crate::memory_capture::start(client, scope, &source, &input.text).await? {
        Ok(started) => Ok(MemoryWorkOutcome::Started(started)),
        Err(_) => {
            client
                .batch_execute("ROLLBACK TO SAVEPOINT capture_start")
                .await
                .map_err(read_error)?;
            Ok(MemoryWorkOutcome::Skipped(
                ConversationCaptureSkip::StartRefused,
            ))
        }
    }
}

/// The settled messages of each completed Run of the author in the Conversation after the last
/// Run that a capture read, the oldest first. That Run is the last Run of the newest capture of the
/// Conversation with the outcome `observations_recorded` or `no_observation`. The messages are
/// the author message, each steering input that a RunStep read, and the final answer text. A Run
/// that is refused, cancelled, or in progress gives no message.
async fn unread_runs(
    client: &Client,
    keys: &[&(dyn tokio_postgres::types::ToSql + Sync); 3],
) -> Result<Vec<ConversationRunMessages>, ProjectReadError> {
    let rows = client
        .query(
            "SELECT run.run_id::text, message.author, message.text
               FROM storyos.agent_runs AS run
              CROSS JOIN LATERAL (
                SELECT true AS author, run.author_message AS text, -1::numeric AS position
                UNION ALL
                SELECT true, input.payload->>'author_message',
                       (input.payload->>'input_position')::numeric
                  FROM storyos.project_activity_event_payloads AS input
                 WHERE (input.owner_user_id, input.project_id) = (run.owner_user_id, run.project_id)
                   AND input.event_kind = 'agent_run_steering_retained'
                   AND input.payload->>'run_id' = run.run_id::text
                   AND (input.payload->>'input_position')::numeric <= run.active_decision_position
                UNION ALL
                (SELECT false, attempt.payload #>> '{decision,text}', 1e30
                   FROM storyos.model_attempts AS attempt
                  WHERE (attempt.owner_user_id, attempt.project_id, attempt.run_id) =
                        (run.owner_user_id, run.project_id, run.run_id)
                    AND attempt.attempt_role = 'decision'
                    AND attempt.decision_position = run.active_decision_position
                    AND attempt.payload #>> '{decision,kind}' = 'advisory'
                    AND attempt.payload #> '{decision,selected}' = 'true'::jsonb
                    AND COALESCE(attempt.payload #>> '{decision,text}', '') <> ''
                  LIMIT 1)
              ) AS message
              WHERE (run.owner_user_id, run.project_id, run.conversation_id) =
                    ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                AND run.run_kind = 'author' AND run.status = 'completed'
                AND run.run_id > COALESCE(
                      (SELECT capture.last_run_id FROM storyos.memory_captures AS capture
                        WHERE (capture.owner_user_id, capture.project_id, capture.conversation_id) =
                              (run.owner_user_id, run.project_id, run.conversation_id)
                          AND capture.outcome IN ('observations_recorded', 'no_observation')
                        ORDER BY capture.capture_id DESC
                        LIMIT 1),
                      '00000000-0000-0000-0000-000000000000'::uuid)
              ORDER BY run.run_id, message.position",
            keys,
        )
        .await
        .map_err(read_error)?;
    let mut runs: Vec<ConversationRunMessages> = Vec::new();
    for row in rows {
        let run_id: String = row.get(/*idx*/ 0);
        let role = match row.get(/*idx*/ 1) {
            true => ConversationRole::Author,
            false => ConversationRole::Assistant,
        };
        let message = (role, row.get(/*idx*/ 2));
        match runs.last_mut() {
            Some(run) if run.run_id == run_id => run.messages.push(message),
            _ => runs.push(ConversationRunMessages {
                run_id,
                messages: vec![message],
            }),
        }
    }
    Ok(runs)
}

#[cfg(test)]
#[path = "memory_work_schedule_tests.rs"]
mod tests;
