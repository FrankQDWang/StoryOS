//! The records of a Subrun (ADR 0103 section 3A): the Subrun Request, the Subrun row, and the
//! Context Assembly of its RunSteps. A Subrun binds the grant, the use binding, the Model
//! Registration revision, and the Project Instruction revision of its parent Run, and it uses the
//! Memory pin of its parent Run.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{AgentRunKind, CurrentPassageAssemblyRecord, ToolResultCandidate};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::agent_run_tool_authority::damaged;
use crate::agent_run_work::complete_unavailable;

/// Writes the queued Subrun of the call `parent_tool_call_id` of the Run of `parent`, its Subrun
/// Request, and its first RunStep, and gives the identity of the Subrun.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the Host Tool start_subrun of #436 part B calls it"
    )
)]
pub(crate) async fn start(
    client: &Client,
    parent: &ClaimedAgentRun,
    parent_tool_call_id: &str,
    title: &str,
    brief: &str,
) -> Result<String, CompleteAgentRunError> {
    let scope = &parent.project_scope;
    let run_id = Uuid::now_v7().to_string();
    client
        .execute(
            "INSERT INTO storyos.agent_runs
               (owner_user_id, project_id, run_id, project_agent_id, grant_id,
                project_model_use_binding_revision, status, model_registration_revision,
                project_instruction_revision_id, run_kind)
             SELECT parent.owner_user_id, parent.project_id, $4::text::uuid,
                    parent.project_agent_id, parent.grant_id,
                    parent.project_model_use_binding_revision, 'queued',
                    parent.model_registration_revision, parent.project_instruction_revision_id,
                    $5
               FROM storyos.agent_runs AS parent
              WHERE parent.owner_user_id = $1::text::uuid AND parent.project_id = $2::text::uuid
                AND parent.run_id = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &parent.run_id,
                &run_id,
                &AgentRunKind::Subrun.name(),
            ],
        )
        .await
        .map_err(complete_unavailable)?;
    client
        .execute(
            "INSERT INTO storyos.subrun_requests
               (owner_user_id, project_id, subrun_run_id, parent_run_id, parent_tool_call_id,
                title, brief)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6, $7)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
                &parent.run_id,
                &parent_tool_call_id,
                &title,
                &brief,
            ],
        )
        .await
        .map_err(complete_unavailable)?;
    let first = parent_first_record(client, parent).await?;
    let memory_pin = first.operation_requirement.memory_revision;
    let memory_index = match memory_pin {
        Some(revision) => {
            crate::memory_entries::read_memory_index(client, scope, revision, AgentRunKind::Subrun)
                .await
                .map_err(CompleteAgentRunError::Unavailable)?
        }
        None => Vec::new(),
    };
    let chapters = crate::create_agent_run::context::chapter_list_entries(client, scope)
        .await
        .map_err(complete_unavailable)?;
    let instruction = crate::create_agent_run::context::bound_instruction(client, scope, &run_id)
        .await
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    let operation_requirement_id = Uuid::now_v7().to_string();
    let record = storyos_core::assemble_subrun_context(&storyos_core::SubrunAssembly {
        operation_requirement_id: &operation_requirement_id,
        run_id: &run_id,
        owner_user_id: scope.owner_user_id.as_ref(),
        project_id: scope.project_id.as_ref(),
        destination_identity: &first.operation_requirement.destination_identity,
        brief,
        instruction: instruction.as_ref(),
        chapters: &chapters,
        memory_pin,
        memory_index: &memory_index,
    });
    crate::create_agent_run::context::insert_assembly(
        client, scope, &run_id, /*receipt_id*/ None, "0", &record,
    )
    .await
    .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    Ok(run_id)
}

/// The Context Assembly of the first RunStep of the Run of `parent`. It holds the Memory pin and
/// the Processing Destination Identity of the Run.
async fn parent_first_record(
    client: &Client,
    parent: &ClaimedAgentRun,
) -> Result<CurrentPassageAssemblyRecord, CompleteAgentRunError> {
    let payload: String = client
        .query_one(
            "SELECT requirement.payload::text
               FROM storyos.context_assembly_manifests AS assembly
               JOIN storyos.operation_requirements AS requirement
                 ON (requirement.owner_user_id, requirement.project_id,
                     requirement.operation_requirement_id) =
                    (assembly.owner_user_id, assembly.project_id,
                     assembly.operation_requirement_id)
              WHERE assembly.owner_user_id = $1::text::uuid
                AND assembly.project_id = $2::text::uuid AND assembly.run_id = $3::text::uuid
                AND assembly.manifest_role = 'decision' AND assembly.decision_position = 0",
            &[
                &parent.project_scope.owner_user_id.as_ref(),
                &parent.project_scope.project_id.as_ref(),
                &parent.run_id,
            ],
        )
        .await
        .map_err(complete_unavailable)?
        .get(/*idx*/ 0);
    serde_json::from_str(&payload)
        .ok()
        .as_ref()
        .and_then(storyos_core::decode_assembly_record)
        .ok_or_else(|| damaged("The Context Assembly of the parent Run is damaged"))
}

/// Writes the Context Assembly of the next RunStep of a Subrun: the items of the RunStep before
/// and one `tool_result` item for each resolved ToolCall of the Subrun (ADR 0085).
pub(crate) async fn persist_next_step(
    client: &Client,
    claim: &ClaimedAgentRun,
    retained: &CurrentPassageAssemblyRecord,
    next_position: &str,
    candidates: &[ToolResultCandidate<'_>],
) -> Result<(), CompleteAgentRunError> {
    let record =
        storyos_core::next_subrun_context(retained, Uuid::now_v7().to_string(), candidates);
    crate::create_agent_run::context::insert_assembly(
        client,
        &claim.project_scope,
        &claim.run_id,
        /*receipt_id*/ None,
        next_position,
        &record,
    )
    .await
    .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    Ok(())
}

#[cfg(test)]
#[path = "subrun_tests.rs"]
mod tests;
