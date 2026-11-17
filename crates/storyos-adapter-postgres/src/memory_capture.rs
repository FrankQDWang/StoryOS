//! A capture in the store (ADR 0094 sections 5 and 8): the start in one transaction, the Context
//! Assembly of each RunStep, and the records of the settled output call. A capture is a Run of
//! the kind `memory_capture` with no Conversation, no Chapter, no author message, and no Receipt. No diagnostic records the input or an observation (ADR 0047).

use storyos_application::{
    ClaimedAgentRun, CompleteAgentRunError, MemoryCaptureSource, MemoryCaptureStartRefusal,
    MemoryCaptureStore, ProjectReadError, ProjectScope, StartedMemoryCapture,
};
use storyos_core::{
    AgentRunKind, AssistanceAvailability, CaptureAdmissionFacts, CaptureRefusal,
    ContextSourceClass, CurrentPassageAssemblyRecord, MemoryCaptureAssembly, MemoryObservation,
    ProjectionMode, ToolResultCandidate,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::agent_run_tool_authority::damaged;
use crate::agent_run_work::complete_database_error;
use crate::update_project_assistance::read_assistance_record;
use crate::{PostgresProjectReader, read_error};

impl MemoryCaptureStore for PostgresProjectReader {
    async fn start_memory_capture(
        &self,
        scope: &ProjectScope,
        source: &MemoryCaptureSource,
        input: &str,
    ) -> Result<Result<StartedMemoryCapture, MemoryCaptureStartRefusal>, ProjectReadError> {
        let transaction = self
            .begin_serializable_project_command_transaction(scope)
            .await
            .map_err(ProjectReadError::unavailable)?;
        let started = start(
            &transaction.client,
            scope,
            source,
            input,
            ProjectionMode::ExactRequired,
        )
        .await;
        match &started {
            Ok(Ok(_)) => transaction
                .commit()
                .await
                .map_err(ProjectReadError::unavailable)?,
            Ok(Err(_)) | Err(_) => {
                let _rollback = transaction.rollback().await;
            }
        }
        started
    }
}

/// Pins the current Model Use Binding, its grant, and its Model Registration, and writes the
/// queued Run, the capture, and the first RunStep. A refused start writes no row. The capture of a
/// Conversation records its current Conversation Memory Settings revision. The input item has
/// `projection_mode`.
pub(crate) async fn start(
    client: &Client,
    scope: &ProjectScope,
    source: &MemoryCaptureSource,
    input: &str,
    projection_mode: ProjectionMode,
) -> Result<Result<StartedMemoryCapture, MemoryCaptureStartRefusal>, ProjectReadError> {
    let assistance = read_assistance_record(client, scope).await?;
    let grant_scope: Option<String> = match &assistance {
        Some(record) => client
            .query_opt(
                "SELECT grant_scope FROM storyos.project_destination_grants
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND grant_id = $3::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &record.grant_id,
                ],
            )
            .await
            .map_err(read_error)?
            .and_then(|row| row.get(/*idx*/ 0)),
        None => None,
    };
    let facts = CaptureAdmissionFacts {
        binding: assistance.is_some(),
        assistance_available: assistance
            .as_ref()
            .is_some_and(|record| record.availability == AssistanceAvailability::Available),
        grant_scope: grant_scope.as_deref(),
        input,
    };
    let assistance = match (storyos_core::admit_memory_capture(&facts), assistance) {
        (Ok(()), Some(assistance)) => assistance,
        (Err(refusal), _) => return Ok(Err(MemoryCaptureStartRefusal::Admission(refusal))),
        (Ok(()), None) => {
            let refusal = CaptureRefusal::NoModelUseBinding;
            return Ok(Err(MemoryCaptureStartRefusal::Admission(refusal)));
        }
    };
    let run_id = Uuid::now_v7().to_string();
    let capture_id = Uuid::now_v7().to_string();
    // Background work writes no grant and no binding: the Run pins the current ones. The first
    // Run of the author made the Agent of the Project.
    client
        .execute(
            "INSERT INTO storyos.agent_runs
               (owner_user_id, project_id, run_id, project_agent_id, grant_id,
                project_model_use_binding_revision, status, model_registration_revision, run_kind)
             SELECT agent.owner_user_id, agent.project_id, $3::text::uuid, agent.project_agent_id,
                    $4::text::uuid, $5::text::uuid, 'queued', $6::text::uuid, $7
               FROM storyos.project_agents AS agent
              WHERE agent.owner_user_id = $1::text::uuid AND agent.project_id = $2::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
                &assistance.grant_id,
                &assistance.project_model_use_binding_revision,
                &assistance.model_registration_revision,
                &AgentRunKind::MemoryCapture.name(),
            ],
        )
        .await
        .map_err(read_error)?;
    let (conversation, first, last, chapter, revision) = match source {
        MemoryCaptureSource::Conversation {
            conversation_id: c,
            first_run_id: f,
            last_run_id: l,
        } => (Some(c), Some(f), Some(l), None, None),
        MemoryCaptureSource::Chapter {
            chapter_id: c,
            revision_id: r,
        } => (None, None, None, Some(c), Some(r)),
    };
    // The source must be in the Project Scope: the two Runs are Runs of the Conversation, in
    // order, or the Revision is a Revision of the Chapter.
    let inserted = client
        .execute(
            "INSERT INTO storyos.memory_captures
               (owner_user_id, project_id, capture_id, run_id, source_kind, conversation_id,
                first_run_id, last_run_id, chapter_id, chapter_revision_id,
                memory_settings_revision)
             SELECT run.owner_user_id, run.project_id, $3::text::uuid, run.run_id,
                    CASE WHEN $5::text IS NULL THEN 'chapter' ELSE 'conversation' END,
                    $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid, $9::text::uuid,
                    (SELECT settings.memory_settings_revision
                       FROM storyos.conversation_memory_settings AS settings
                      WHERE (settings.owner_user_id, settings.project_id, settings.conversation_id) =
                            (run.owner_user_id, run.project_id, $5::text::uuid)
                        AND COALESCE(settings.is_current, true))
               FROM storyos.agent_runs AS run
              WHERE (run.owner_user_id, run.project_id, run.run_id) =
                    ($1::text::uuid, $2::text::uuid, $4::text::uuid)
                AND (EXISTS (SELECT 1 FROM storyos.agent_runs AS first
                               JOIN storyos.agent_runs AS last
                                 ON (last.owner_user_id, last.project_id, last.conversation_id) =
                                    (first.owner_user_id, first.project_id, first.conversation_id)
                              WHERE (first.owner_user_id, first.project_id) =
                                    (run.owner_user_id, run.project_id)
                                AND first.conversation_id = $5::text::uuid
                                AND first.run_id = $6::text::uuid AND last.run_id = $7::text::uuid
                                AND first.run_id <= last.run_id)
                  OR EXISTS (SELECT 1 FROM storyos.authoritative_revisions AS revision
                               JOIN storyos.manuscript_objects AS chapter
                                 USING (owner_user_id, project_id, manuscript_object_id)
                              WHERE (revision.owner_user_id, revision.project_id) =
                                    (run.owner_user_id, run.project_id)
                                AND revision.manuscript_object_id = $8::text::uuid
                                AND revision.revision_id = $9::text::uuid
                                AND chapter.object_kind = 'chapter'))",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &capture_id,
                &run_id,
                &conversation,
                &first,
                &last,
                &chapter,
                &revision,
            ],
        )
        .await
        .map_err(read_error)?;
    if inserted != 1 {
        return Ok(Err(MemoryCaptureStartRefusal::SourceNotFound));
    }
    let mut record = storyos_core::assemble_memory_capture_context(&MemoryCaptureAssembly {
        operation_requirement_id: Uuid::now_v7().to_string(),
        input_snapshot_id: Uuid::now_v7().to_string(),
        run_id: run_id.clone(),
        owner_user_id: scope.owner_user_id.as_ref().to_owned(),
        project_id: scope.project_id.as_ref().to_owned(),
        input: input.to_owned(),
        projection_mode,
        destination_identity: assistance.processing_destination_identity,
    });
    add_references(client, scope, &mut record).await?;
    crate::create_agent_run::context::insert_assembly(
        client, scope, &run_id, /*receipt_id*/ None, "0", &record,
    )
    .await
    .map_err(ProjectReadError::unavailable)?;
    Ok(Ok(StartedMemoryCapture { capture_id, run_id }))
}

/// Adds the reference items of the first RunStep: the Memory Index at the current head, the
/// Chapter list, and the delete marks (ADR 0094 sections 4, 5, and 8).
async fn add_references(
    client: &Client,
    scope: &ProjectScope,
    record: &mut CurrentPassageAssemblyRecord,
) -> Result<(), ProjectReadError> {
    let unavailable = |error| ProjectReadError::unavailable(std::io::Error::other(error));
    let memory_head = crate::memory_entries::memory_head(client, scope)
        .await
        .map_err(unavailable)?;
    let memory_index = crate::memory_entries::read_memory_index(client, scope, memory_head)
        .await
        .map_err(unavailable)?;
    let chapters = crate::create_agent_run::context::chapter_list_entries(client, scope).await?;
    let delete_marks = crate::memory_entries::read_delete_marks(client, scope)
        .await
        .map_err(unavailable)?;
    storyos_core::assemble_capture_references(
        record,
        &storyos_core::CaptureReferences {
            memory_head,
            memory_index: &memory_index,
            chapters: &chapters,
            delete_marks: &delete_marks,
        },
    );
    Ok(())
}

/// The Context Assembly of the next RunStep of a capture. It has the input item and the reference
/// items of the RunStep before. It also has one `tool_result` item for each resolved ToolCall of
/// the Run (ADR 0085).
pub(crate) async fn persist_next_step(
    client: &Client,
    claim: &ClaimedAgentRun,
    retained: &CurrentPassageAssemblyRecord,
    next_position: &str,
    candidates: &[ToolResultCandidate<'_>],
) -> Result<(), CompleteAgentRunError> {
    let input = retained
        .selected
        .iter()
        .find(|item| item.source_class == ContextSourceClass::MemoryCaptureInput)
        .ok_or_else(|| damaged("The input of the capture is missing"))?;
    let scope = &claim.project_scope;
    let mut record = storyos_core::assemble_memory_capture_context(&MemoryCaptureAssembly {
        operation_requirement_id: Uuid::now_v7().to_string(),
        input_snapshot_id: Uuid::now_v7().to_string(),
        run_id: claim.run_id.clone(),
        owner_user_id: scope.owner_user_id.as_ref().to_owned(),
        project_id: scope.project_id.as_ref().to_owned(),
        input: input.content.clone(),
        projection_mode: input.projection_mode,
        destination_identity: retained.operation_requirement.destination_identity.clone(),
    });
    storyos_core::keep_capture_references(&mut record, retained);
    storyos_core::assemble_tool_result_continuation(&mut record, candidates);
    crate::create_agent_run::context::insert_assembly(
        client,
        scope,
        &claim.run_id,
        /*receipt_id*/ None,
        next_position,
        &record,
    )
    .await
    .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    Ok(())
}

/// The kind of the input of the capture of the Run. A capture Run with no capture row is damaged.
pub(crate) async fn input_kind(
    client: &Client,
    claim: &ClaimedAgentRun,
) -> Result<storyos_core::CaptureInputKind, CompleteAgentRunError> {
    let scope = &claim.project_scope;
    let source_kind: Option<String> = client
        .query_opt(
            "SELECT source_kind FROM storyos.memory_captures
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(complete_database_error)?
        .map(|row| row.get(/*idx*/ 0));
    source_kind
        .as_deref()
        .and_then(storyos_core::CaptureInputKind::parse)
        .ok_or_else(|| damaged("The capture of the Run"))
}

/// Settles the capture of the Run with the `outcome` that Core decided and stores each of
/// `observations` as a waiting Memory Observation in list order, in the Tool Gateway transaction
/// of the call. The settlement of a Conversation capture can make its schedule row due, and a
/// settlement that stores an observation makes the consolidation row of the Project due.
pub(crate) async fn record_observations(
    client: &Client,
    claim: &ClaimedAgentRun,
    outcome: storyos_core::CaptureOutcome,
    observations: &[MemoryObservation],
) -> Result<(), CompleteAgentRunError> {
    let scope = &claim.project_scope;
    let label = outcome.label();
    let capture_id: String = client
        .query_opt(
            "UPDATE storyos.memory_captures SET state = 'settled', outcome = $4
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND state = 'running'
          RETURNING capture_id::text",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &claim.run_id,
                &label,
            ],
        )
        .await
        .map_err(complete_database_error)?
        .ok_or_else(|| damaged("The running capture of the Run is missing"))?
        .get(/*idx*/ 0);
    crate::memory_work_schedule::mark_due_after_capture(client, claim, &capture_id, outcome)
        .await?;
    // Each item as its kind, its type, title, and text, or its entry number and reason.
    let rows: Vec<serde_json::Value> = observations
        .iter()
        .map(|item| match item {
            MemoryObservation::Fact {
                entry_type,
                title,
                text,
            } => serde_json::json!(["observation", entry_type.as_str(), title, text, null, null]),
            MemoryObservation::Conflict { entry, reason } => {
                serde_json::json!(["conflict", null, null, null, entry.to_string(), reason])
            }
        })
        .collect();
    client
        .execute(
            "INSERT INTO storyos.memory_observations
               (owner_user_id, project_id, capture_id, position, kind, entry_type, title, text,
                entry_number, reason)
             SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, item.position::numeric,
                    item.row ->> 0, item.row ->> 1, item.row ->> 2, item.row ->> 3,
                    (item.row ->> 4)::numeric, item.row ->> 5
               FROM jsonb_array_elements($4::text::jsonb) WITH ORDINALITY AS item(row, position)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &capture_id,
                &serde_json::Value::from(rows).to_string(),
            ],
        )
        .await
        .map_err(complete_database_error)?;
    if !observations.is_empty() {
        crate::memory_work_schedule::mark_consolidation_due(client, scope).await?;
    }
    Ok(())
}

/// Settles the running capture of the Run, when the Run has one, with the outcome that Core
/// gives from `end`. It stores no observation. A Run with no running capture is not changed.
pub(crate) async fn settle_end(
    client: &Client,
    claim: &ClaimedAgentRun,
    end: storyos_core::CaptureRunEnd<'_>,
) -> Result<(), CompleteAgentRunError> {
    let Some(end) = storyos_core::capture_end(end) else {
        return Ok(());
    };
    let scope = &claim.project_scope;
    client
        .execute(
            "UPDATE storyos.memory_captures
                SET state = 'settled', outcome = $4, failure_reason = $5
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND run_id = $3::text::uuid AND state = 'running'",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &claim.run_id,
                &end.outcome.label(),
                &end.failure_reason,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    Ok(())
}

#[cfg(test)]
#[path = "memory_capture_tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "memory_capture_loop_tests.rs"]
pub(crate) mod loop_tests;

#[cfg(test)]
#[path = "memory_capture_order_tests.rs"]
mod order_tests;

#[cfg(test)]
#[path = "memory_capture_failure_tests.rs"]
mod failure_tests;

#[cfg(test)]
#[path = "memory_capture_recovery_tests.rs"]
mod recovery_tests;

#[cfg(test)]
#[path = "memory_capture_tools_tests.rs"]
mod tools_tests;
