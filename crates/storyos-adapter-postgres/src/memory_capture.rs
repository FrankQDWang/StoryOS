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
    ToolResultCandidate,
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
        let started = start(&transaction.client, scope, source, input).await;
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
/// queued Run, the capture, and the first RunStep. A refused start writes no row.
async fn start(
    client: &Client,
    scope: &ProjectScope,
    source: &MemoryCaptureSource,
    input: &str,
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
                first_run_id, last_run_id, chapter_id, chapter_revision_id)
             SELECT run.owner_user_id, run.project_id, $3::text::uuid, run.run_id,
                    CASE WHEN $5::text IS NULL THEN 'chapter' ELSE 'conversation' END,
                    $5::text::uuid, $6::text::uuid, $7::text::uuid, $8::text::uuid, $9::text::uuid
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
    let record = storyos_core::assemble_memory_capture_context(&MemoryCaptureAssembly {
        operation_requirement_id: Uuid::now_v7().to_string(),
        input_snapshot_id: Uuid::now_v7().to_string(),
        run_id: run_id.clone(),
        owner_user_id: scope.owner_user_id.as_ref().to_owned(),
        project_id: scope.project_id.as_ref().to_owned(),
        input: input.to_owned(),
        destination_identity: assistance.processing_destination_identity,
    });
    crate::create_agent_run::context::insert_assembly(
        client, scope, &run_id, /*receipt_id*/ None, "0", &record,
    )
    .await
    .map_err(ProjectReadError::unavailable)?;
    Ok(Ok(StartedMemoryCapture { capture_id, run_id }))
}

/// The Context Assembly of the next RunStep of a capture: the input item of the RunStep before,
/// and one `tool_result` item for each resolved ToolCall of the Run (ADR 0085).
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
        destination_identity: retained.operation_requirement.destination_identity.clone(),
    });
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

/// Settles the capture of the Run with the `outcome` that Core decided and stores each of
/// `observations` as a waiting Memory Observation in list order, in the Tool Gateway transaction
/// of the call.
pub(crate) async fn record_observations(
    client: &Client,
    claim: &ClaimedAgentRun,
    outcome: storyos_core::CaptureOutcome,
    observations: &[MemoryObservation],
) -> Result<(), CompleteAgentRunError> {
    let scope = &claim.project_scope;
    let outcome = outcome.label();
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
                &outcome,
            ],
        )
        .await
        .map_err(complete_database_error)?
        .ok_or_else(|| damaged("The running capture of the Run is missing"))?
        .get(/*idx*/ 0);
    let types: Vec<&str> = observations
        .iter()
        .map(|item| item.entry_type.as_str())
        .collect();
    let titles: Vec<&str> = observations
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    let texts: Vec<&str> = observations.iter().map(|item| item.text.as_str()).collect();
    client
        .execute(
            "INSERT INTO storyos.memory_observations
               (owner_user_id, project_id, capture_id, position, entry_type, title, text)
             SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, item.position::numeric,
                    item.entry_type, item.title, item.text
               FROM unnest($4::text[], $5::text[], $6::text[]) WITH ORDINALITY
                    AS item(entry_type, title, text, position)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &capture_id,
                &types,
                &titles,
                &texts,
            ],
        )
        .await
        .map_err(complete_database_error)?;
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
mod loop_tests;

#[cfg(test)]
#[path = "memory_capture_order_tests.rs"]
mod order_tests;

#[cfg(test)]
#[path = "memory_capture_failure_tests.rs"]
mod failure_tests;
