//! The schedule of background Memory work in the store (ADR 0094 sections 5 and 6, writers 2 and
//! 3): the due writes in the transactions of the settlements, and the Worker claim of a due row. The
//! claim, the decision, the start of the capture or the pass, and the empty due time are one
//! transaction. Thus a Worker that stops before the commit leaves the row due, and a second
//! delivery starts no second capture for the same messages and no second pass. No record of this
//! work has message text outside the input of the capture (ADR 0047).

use std::time::SystemTime;

use storyos_application::{
    ClaimedAgentRun, ClaimedMemoryWork, CompleteAgentRunError, MemoryCaptureSource,
    MemoryWorkOutcome, MemoryWorkScheduleStore, MemoryWorkSource, ProjectId, ProjectReadError,
    ProjectScope, UserId,
};
use storyos_core::{
    ConsolidationFacts, ConsolidationSkip, ConversationCaptureFacts, ConversationCaptureSkip,
    ConversationRole, ConversationRunMessages, IntentPassFacts, IntentPassSkip,
};
use tokio_postgres::Client;

use crate::agent_run_work::complete_unavailable;
use crate::connection_pool::PooledClient;
use crate::export_work::set_worker_scope;
use crate::{PostgresProjectReader, read_error};

/// The kind of the schedule row of a Project Conversation.
const CONVERSATION_CAPTURE: &str = "conversation_capture";
/// The kind of the schedule row of the consolidation pass of a Project.
const CONSOLIDATION: &str = "consolidation";
/// The kind of the schedule row of the intent pass of a Project (ADR 0101 section 4).
const INTENT_PASS: &str = "intent_pass";

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
        .map_err(complete_unavailable)?
        .get(/*idx*/ 0);
    client
        .execute(
            "INSERT INTO storyos.memory_work_schedules
               (owner_user_id, project_id, work_kind, source_id, conversation_id, due_at)
             SELECT run.owner_user_id, run.project_id, $4, run.conversation_id,
                    run.conversation_id, $5
               FROM storyos.agent_runs AS run
              WHERE (run.owner_user_id, run.project_id, run.run_id) =
                    ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                AND run.run_kind = 'author'
             ON CONFLICT (owner_user_id, project_id, work_kind, source_id)
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
        .map_err(complete_unavailable)?;
    Ok(())
}

/// Sets the due time of the schedule row of the Conversation of the capture `capture_id` that
/// settles with `outcome`, in the transaction of its settlement. A Chapter capture has no row of a
/// Conversation.
pub(crate) async fn mark_due_after_capture(
    client: &Client,
    claim: &ClaimedAgentRun,
    capture_id: &str,
    outcome: storyos_core::CaptureOutcome,
) -> Result<(), CompleteAgentRunError> {
    let keys: [&(dyn tokio_postgres::types::ToSql + Sync); 3] = [
        &claim.project_scope.owner_user_id.as_ref(),
        &claim.project_scope.project_id.as_ref(),
        &capture_id,
    ];
    let Some(row) = client
        .query_opt(
            "SELECT capture.conversation_id::text,
                    EXISTS (SELECT 1 FROM storyos.agent_runs AS run
                             WHERE (run.owner_user_id, run.project_id, run.conversation_id) =
                                   (capture.owner_user_id, capture.project_id,
                                    capture.conversation_id)
                               AND run.run_kind = 'author' AND run.status = 'completed'
                               AND run.run_id > capture.last_run_id),
                    clock_timestamp()
               FROM storyos.memory_captures AS capture
              WHERE (capture.owner_user_id, capture.project_id, capture.capture_id) =
                    ($1::text::uuid, $2::text::uuid, $3::text::uuid)
                AND capture.source_kind = 'conversation'",
            &keys,
        )
        .await
        .map_err(complete_unavailable)?
    else {
        return Ok(());
    };
    let conversation_id: String = row.get(/*idx*/ 0);
    let Some(due_at) =
        storyos_core::capture_settlement_due(outcome, row.get(/*idx*/ 1), row.get(/*idx*/ 2))
    else {
        return Ok(());
    };
    client
        .execute(
            "INSERT INTO storyos.memory_work_schedules
               (owner_user_id, project_id, work_kind, source_id, conversation_id, due_at)
             VALUES ($1::text::uuid, $2::text::uuid, $3, $4::text::uuid, $4::text::uuid, $5)
             ON CONFLICT (owner_user_id, project_id, work_kind, source_id)
             DO UPDATE SET due_at = EXCLUDED.due_at",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &CONVERSATION_CAPTURE,
                &conversation_id,
                &due_at,
            ],
        )
        .await
        .map_err(complete_unavailable)?;
    Ok(())
}

/// Sets the due time of the consolidation row of the Project of `scope` by the rule of Core, in the
/// transaction of the settlement of a capture that stored observations or of a pass with each
/// outcome. When no observation waits, the row does not change.
pub(crate) async fn mark_consolidation_due(
    client: &Client,
    scope: &ProjectScope,
) -> Result<(), CompleteAgentRunError> {
    let keys = [scope.owner_user_id.as_ref(), scope.project_id.as_ref()];
    let row = client
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM storyos.memory_observations
                             WHERE (owner_user_id, project_id) = ($1::text::uuid, $2::text::uuid)
                               AND state = 'waiting'),
                    clock_timestamp(),
                    (SELECT max(started_at) FROM storyos.memory_consolidation_passes
                      WHERE (owner_user_id, project_id) = ($1::text::uuid, $2::text::uuid))",
            &[&keys[0], &keys[1]],
        )
        .await
        .map_err(complete_unavailable)?;
    let Some(due_at) =
        storyos_core::consolidation_due(row.get(/*idx*/ 0), row.get(/*idx*/ 1), row.get(/*idx*/ 2))
    else {
        return Ok(());
    };
    client
        .execute(
            "INSERT INTO storyos.memory_work_schedules
               (owner_user_id, project_id, work_kind, source_id, due_at)
             VALUES ($1::text::uuid, $2::text::uuid, $3, $2::text::uuid, $4)
             ON CONFLICT (owner_user_id, project_id, work_kind, source_id)
             DO UPDATE SET due_at = EXCLUDED.due_at",
            &[&keys[0], &keys[1], &CONSOLIDATION, &due_at],
        )
        .await
        .map_err(complete_unavailable)?;
    Ok(())
}

/// Makes the intent pass row of the Project of `scope` due at `due_at`.
pub(crate) async fn mark_intent_pass_due(
    client: &Client,
    scope: &ProjectScope,
    due_at: SystemTime,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO storyos.memory_work_schedules
               (owner_user_id, project_id, work_kind, source_id, due_at)
             VALUES ($1::text::uuid, $2::text::uuid, $3, $2::text::uuid, $4)
             ON CONFLICT (owner_user_id, project_id, work_kind, source_id)
             DO UPDATE SET due_at = EXCLUDED.due_at",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &INTENT_PASS,
                &due_at,
            ],
        )
        .await?;
    Ok(())
}

impl MemoryWorkScheduleStore for PostgresProjectReader {
    async fn memory_work_due(&self) -> Result<bool, ProjectReadError> {
        let client = self.connect().await?;
        client.batch_execute("BEGIN").await.map_err(read_error)?;
        set_worker_scope(&*client).await?;
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

/// Claims one due row in the Worker scope and makes its due time empty, then decides and starts
/// in the Project Scope of the row.
async fn claim_and_decide(
    client: &PooledClient,
) -> Result<Option<ClaimedMemoryWork>, ProjectReadError> {
    set_worker_scope(&**client).await?;
    let Some(row) = client
        .query_opt(
            "WITH next_work AS (
               SELECT owner_user_id, project_id, work_kind, source_id, due_at
                 FROM storyos.memory_work_schedules
                WHERE due_at <= clock_timestamp()
                ORDER BY due_at, owner_user_id, project_id, work_kind, source_id
                FOR UPDATE SKIP LOCKED
                LIMIT 1
             )
             UPDATE storyos.memory_work_schedules AS work SET due_at = NULL
               FROM next_work
              WHERE (work.owner_user_id, work.project_id, work.work_kind, work.source_id) =
                    (next_work.owner_user_id, next_work.project_id, next_work.work_kind,
                     next_work.source_id)
          RETURNING work.owner_user_id::text, work.project_id::text, work.conversation_id::text,
                    work.chapter_id::text, next_work.due_at, work.work_kind",
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
    let source = match (
        row.get::<_, Option<String>>(/*idx*/ 2),
        row.get::<_, Option<String>>(/*idx*/ 3),
    ) {
        (Some(conversation_id), _) => MemoryWorkSource::Conversation(conversation_id),
        (None, Some(chapter_id)) => MemoryWorkSource::Chapter(chapter_id),
        (None, None) if row.get::<_, &str>(/*idx*/ 5) == INTENT_PASS => {
            MemoryWorkSource::ProjectIntents
        }
        (None, None) => MemoryWorkSource::Project,
    };
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
    let (source, outcome) = match source {
        MemoryWorkSource::Conversation(conversation_id) => {
            let outcome = decide_and_start(client, &scope, &conversation_id).await?;
            (MemoryWorkSource::Conversation(conversation_id), outcome)
        }
        MemoryWorkSource::Project => (
            MemoryWorkSource::Project,
            decide_and_start_pass(client, &scope).await?,
        ),
        MemoryWorkSource::ProjectIntents => (
            MemoryWorkSource::ProjectIntents,
            decide_and_start_intent_pass(client, &scope).await?,
        ),
        // The decision can be for another due Chapter of the Project that is first in order.
        MemoryWorkSource::Chapter(claimed) => {
            let (chapter_id, outcome) = crate::chapter_capture_schedule::decide_and_start(
                client,
                &scope,
                &claimed,
                row.get(/*idx*/ 4),
            )
            .await?;
            (MemoryWorkSource::Chapter(chapter_id), outcome)
        }
    };
    Ok(Some(ClaimedMemoryWork {
        project_scope: scope,
        source,
        outcome,
    }))
}

/// Decides for the consolidation row of the Project and starts one pass or none. A refused start
/// writes no row, and the empty due time stays.
async fn decide_and_start_pass(
    client: &PooledClient,
    scope: &ProjectScope,
) -> Result<MemoryWorkOutcome, ProjectReadError> {
    let row = client
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM storyos.memory_observations
                             WHERE (owner_user_id, project_id) = ($1::text::uuid, $2::text::uuid)
                               AND state = 'waiting' AND kind = 'observation'),
                    EXISTS (SELECT 1 FROM storyos.memory_observations
                             WHERE (owner_user_id, project_id) = ($1::text::uuid, $2::text::uuid)
                               AND state = 'waiting' AND kind = 'conflict'),
                    EXISTS (SELECT 1 FROM (SELECT DISTINCT ON (memory_entry_id) state
                                             FROM storyos.memory_entry_versions
                                            WHERE (owner_user_id, project_id) =
                                                  ($1::text::uuid, $2::text::uuid)
                                            ORDER BY memory_entry_id, memory_revision DESC) AS latest
                             WHERE latest.state = 'current'),
                    EXISTS (SELECT 1 FROM storyos.memory_consolidation_passes
                             WHERE (owner_user_id, project_id) = ($1::text::uuid, $2::text::uuid)
                               AND outcome IS NULL),
                    EXISTS (SELECT 1 FROM storyos.intent_passes
                             WHERE (owner_user_id, project_id) = ($1::text::uuid, $2::text::uuid)
                               AND outcome IS NULL)",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(read_error)?;
    let facts = ConsolidationFacts {
        observation_waits: row.get(/*idx*/ 0),
        conflict_waits: row.get(/*idx*/ 1),
        entry_current: row.get(/*idx*/ 2),
        pass_in_progress: row.get(/*idx*/ 3),
        intent_pass_in_progress: row.get(/*idx*/ 4),
    };
    if let Err(skip) = storyos_core::decide_consolidation(&facts) {
        return Ok(MemoryWorkOutcome::PassSkipped(skip));
    }
    client
        .batch_execute("SAVEPOINT pass_start")
        .await
        .map_err(read_error)?;
    match crate::memory_consolidation::start(client, scope).await? {
        Ok(started) => Ok(MemoryWorkOutcome::PassStarted(started)),
        Err(_) => {
            client
                .batch_execute("ROLLBACK TO SAVEPOINT pass_start")
                .await
                .map_err(read_error)?;
            Ok(MemoryWorkOutcome::PassSkipped(
                ConsolidationSkip::StartRefused,
            ))
        }
    }
}

/// Decides for the intent pass row of the Project and starts one pass for the first entry that
/// needs one, or none. A refused start writes no row, and the empty due time stays. When the rate
/// does not permit a start, the row is due at the time that the rate permits.
async fn decide_and_start_intent_pass(
    client: &PooledClient,
    scope: &ProjectScope,
) -> Result<MemoryWorkOutcome, ProjectReadError> {
    let facts = crate::intent_pass::entry_facts(client, scope)
        .await
        .map_err(read_error)?;
    let entry = facts.needed();
    let row = client
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM storyos.intent_passes
                             WHERE (owner_user_id, project_id) = ($1::text::uuid, $2::text::uuid)
                               AND outcome IS NULL),
                    EXISTS (SELECT 1 FROM storyos.memory_observations
                             WHERE (owner_user_id, project_id) = ($1::text::uuid, $2::text::uuid)
                               AND state = 'waiting' AND kind = 'observation'),
                    EXISTS (SELECT 1 FROM storyos.memory_consolidation_passes
                             WHERE (owner_user_id, project_id) = ($1::text::uuid, $2::text::uuid)
                               AND outcome IS NULL),
                    clock_timestamp(),
                    (SELECT max(started_at) FROM storyos.intent_passes
                      WHERE (owner_user_id, project_id) = ($1::text::uuid, $2::text::uuid))",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(read_error)?;
    let facts = IntentPassFacts {
        entry_needs_pass: entry.is_some(),
        pass_in_progress: row.get(/*idx*/ 0),
        observation_waits: row.get(/*idx*/ 1),
        consolidation_in_progress: row.get(/*idx*/ 2),
        now: row.get(/*idx*/ 3),
        last_pass_start: row.get(/*idx*/ 4),
    };
    let skip = storyos_core::decide_intent_pass(&facts).err();
    let (None, Some(entry)) = (skip, entry) else {
        let skip = skip.unwrap_or(IntentPassSkip::NoEntryNeedsPass);
        if let IntentPassSkip::RateLimited { due_at } = skip {
            mark_intent_pass_due(client, scope, due_at)
                .await
                .map_err(read_error)?;
        }
        return Ok(MemoryWorkOutcome::IntentPassSkipped(skip));
    };
    client
        .batch_execute("SAVEPOINT intent_pass_start")
        .await
        .map_err(read_error)?;
    match crate::intent_pass::start(client, scope, entry).await? {
        Ok(started) => Ok(MemoryWorkOutcome::IntentPassStarted(started)),
        Err(_) => {
            client
                .batch_execute("ROLLBACK TO SAVEPOINT intent_pass_start")
                .await
                .map_err(read_error)?;
            Ok(MemoryWorkOutcome::IntentPassSkipped(
                IntentPassSkip::StartRefused,
            ))
        }
    }
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
    let Some(input) =
        storyos_core::conversation_capture_input(&runs, storyos_core::CONTEXT_ITEM_TOKEN_LIMIT)
    else {
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
    let started =
        crate::memory_capture::start(client, scope, &source, &input.text, input.projection_mode)
            .await?;
    match started {
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
pub(crate) mod tests;

#[cfg(test)]
#[path = "memory_work_outcome_tests.rs"]
pub(crate) mod outcome_tests;
