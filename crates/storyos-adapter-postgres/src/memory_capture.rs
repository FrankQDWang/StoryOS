//! A capture in the store (ADR 0094 sections 5 and 8): the start in one transaction, the Context
//! Assembly of each RunStep, and the records of the settled output call. A capture is a Run of
//! the kind `memory_capture` with no Conversation, no Chapter, no author message, and no Receipt.
//! No diagnostic records the input or an observation (ADR 0047).

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
    if !source_exists(client, scope, source).await? {
        return Ok(Err(MemoryCaptureStartRefusal::SourceNotFound));
    }
    let run_id = Uuid::now_v7().to_string();
    let capture_id = Uuid::now_v7().to_string();
    // Background work writes no grant and no binding: the Run pins the current ones. The first
    // Run of the author made the Agent of the Project.
    let inserted = client
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
    if inserted != 1 {
        return Err(ProjectReadError::unavailable(std::io::Error::other(
            "A capture needs the Agent of the Project",
        )));
    }
    let (kind, conversation, first, last, chapter, revision) = match source {
        MemoryCaptureSource::Conversation {
            conversation_id,
            first_run_id,
            last_run_id,
        } => (
            "conversation",
            Some(conversation_id),
            Some(first_run_id),
            Some(last_run_id),
            None,
            None,
        ),
        MemoryCaptureSource::Chapter {
            chapter_id,
            revision_id,
        } => (
            "chapter",
            None,
            None,
            None,
            Some(chapter_id),
            Some(revision_id),
        ),
    };
    client
        .execute(
            "INSERT INTO storyos.memory_captures
               (owner_user_id, project_id, capture_id, run_id, source_kind, conversation_id,
                first_run_id, last_run_id, chapter_id, chapter_revision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5,
                     $6::text::uuid, $7::text::uuid, $8::text::uuid, $9::text::uuid,
                     $10::text::uuid)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &capture_id,
                &run_id,
                &kind,
                &conversation,
                &first,
                &last,
                &chapter,
                &revision,
            ],
        )
        .await
        .map_err(read_error)?;
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

/// Whether the source is in the Project Scope: the two Runs are Runs of the Conversation, in
/// order, or the Revision is a Revision of the Chapter.
async fn source_exists(
    client: &Client,
    scope: &ProjectScope,
    source: &MemoryCaptureSource,
) -> Result<bool, ProjectReadError> {
    let (owner, project) = (scope.owner_user_id.as_ref(), scope.project_id.as_ref());
    let row = match source {
        MemoryCaptureSource::Conversation {
            conversation_id,
            first_run_id,
            last_run_id,
        } => {
            client
                .query_one(
                    "SELECT EXISTS (SELECT 1 FROM storyos.agent_runs AS first
                              JOIN storyos.agent_runs AS last
                                ON (last.owner_user_id, last.project_id, last.conversation_id) =
                                   (first.owner_user_id, first.project_id, first.conversation_id)
                             WHERE first.owner_user_id = $1::text::uuid
                               AND first.project_id = $2::text::uuid
                               AND first.conversation_id = $3::text::uuid
                               AND first.run_id = $4::text::uuid
                               AND last.run_id = $5::text::uuid
                               AND first.run_id <= last.run_id)",
                    &[&owner, &project, conversation_id, first_run_id, last_run_id],
                )
                .await
        }
        MemoryCaptureSource::Chapter {
            chapter_id,
            revision_id,
        } => {
            client
                .query_one(
                    "SELECT EXISTS (SELECT 1 FROM storyos.authoritative_revisions AS revision
                              JOIN storyos.manuscript_objects AS chapter
                                ON (chapter.owner_user_id, chapter.project_id,
                                    chapter.manuscript_object_id) =
                                   (revision.owner_user_id, revision.project_id,
                                    revision.manuscript_object_id)
                             WHERE revision.owner_user_id = $1::text::uuid
                               AND revision.project_id = $2::text::uuid
                               AND revision.manuscript_object_id = $3::text::uuid
                               AND revision.revision_id = $4::text::uuid
                               AND chapter.object_kind = 'chapter')",
                    &[&owner, &project, chapter_id, revision_id],
                )
                .await
        }
    };
    Ok(row.map_err(read_error)?.get(/*idx*/ 0))
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

/// Settles the capture of the Run with `observations` and stores each one as a waiting Memory
/// Observation in list order, in the Tool Gateway transaction of the call.
pub(crate) async fn record_observations(
    client: &Client,
    claim: &ClaimedAgentRun,
    observations: &[MemoryObservation],
) -> Result<(), CompleteAgentRunError> {
    let scope = &claim.project_scope;
    let outcome = match observations.is_empty() {
        true => "no_observation",
        false => "observations_recorded",
    };
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
