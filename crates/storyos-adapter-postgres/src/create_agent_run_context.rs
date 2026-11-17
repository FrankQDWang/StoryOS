use storyos_application::{
    AgentRunContext, CreateAgentRunError, ProjectScope, WorkingTargetAvailability,
};
use storyos_core::{
    CurrentPassageAssembly, InstructionBindingInput, assemble_current_passage_context,
    decode_assembly_record, encode_assembly_record,
};

use super::{agent_run_database_error, agent_run_parse_error};

pub(crate) struct PassageContextInput<'a> {
    pub project_scope: &'a ProjectScope,
    pub run_id: &'a str,
    pub chapter_id: &'a str,
    pub author_message: &'a str,
    pub receipt_id: &'a str,
    pub decision_position: &'a str,
    pub passage_targets: Option<&'a [storyos_core::PassageContextTarget]>,
    pub candidate_target: Option<&'a storyos_core::ProposalCandidateTarget>,
    pub selection_references: Option<&'a [storyos_core::SelectionReference]>,
    /// The Skill requirements of the Run, in the order of the Skill names (ADR 0089 section 5).
    pub skill_requirements: &'a [storyos_core::SkillRequirement],
    /// The Memory revision that the Run pinned. Each RunStep of the Run copies it (ADR 0094
    /// section 4).
    pub memory_revision: Option<u64>,
    /// The resolved ToolCalls of a `tool_result_continuation`, in item order.
    pub tool_results: Option<&'a [storyos_core::ToolResultCandidate<'a>]>,
}

pub(crate) async fn persist_current_passage_assembly(
    client: &tokio_postgres::Client,
    command: &PassageContextInput<'_>,
    destination_identity: &str,
) -> Result<AgentRunContext, CreateAgentRunError> {
    let (chapter_revision_id, chapter_body) =
        load_working_target(client, command.project_scope, command.chapter_id).await?;
    let selected = client
        .query(
            "SELECT member.manuscript_block_id::text
               FROM storyos.manuscript_revision_members AS member
              WHERE member.owner_user_id = $1::text::uuid
                AND member.project_id = $2::text::uuid
                AND member.manuscript_object_id = $3::text::uuid
                AND member.revision_id = $4::text::uuid
                AND NOT EXISTS (
                  SELECT 1 FROM storyos.proposal_operations AS reservation
                   WHERE reservation.owner_user_id = member.owner_user_id
                     AND reservation.project_id = member.project_id
                     AND reservation.manuscript_block_id = member.manuscript_block_id
                     AND reservation.reservation_state = 'unresolved'
                )
              ORDER BY member.block_order",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.chapter_id,
                &chapter_revision_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    let operation_requirement_id = uuid::Uuid::now_v7().to_string();
    let input_snapshot_id = uuid::Uuid::now_v7().to_string();
    let source = CurrentPassageAssembly {
        operation_requirement_id: operation_requirement_id.clone(),
        input_snapshot_id: input_snapshot_id.clone(),
        run_id: command.run_id.to_owned(),
        owner_user_id: command.project_scope.owner_user_id.as_ref().to_owned(),
        project_id: command.project_scope.project_id.as_ref().to_owned(),
        author_message: command.author_message.to_owned(),
        chapter_id: command.chapter_id.to_owned(),
        chapter_revision_id: chapter_revision_id.clone(),
        proposal_target_block_ids: Some(selected.into_iter().map(|row| row.get(0)).collect()),
        chapter_body,
        instruction: InstructionBindingInput::Absent,
        destination_identity: destination_identity.to_owned(),
    };
    let record = if let Some(target) = &command.candidate_target {
        let candidate = crate::candidate_revision_target::load(
            client,
            command.project_scope,
            command.chapter_id,
            target,
        )
        .await?
        .ok_or(CreateAgentRunError::BindingConflict)?;
        storyos_core::assemble_candidate_context(
            &CurrentPassageAssembly {
                proposal_target_block_ids: Some(vec![candidate.block_id]),
                ..source
            },
            target,
            &candidate.text,
        )
    } else if let Some(references) = command.selection_references {
        crate::selection_references::assemble(client, command, &source, references).await?
    } else {
        match &command.passage_targets {
            Some(targets) => {
                crate::passage_collection::assemble(client, command, &source, targets).await?
            }
            None => match storyos_core::parse_ordinary_passage_request(command.author_message) {
                None => assemble_current_passage_context(&source),
                Some(request) => {
                    let targets = match request {
                        Ok(references) => {
                            crate::ordinary_passage_targets::resolve(client, command, &references)
                                .await?
                        }
                        Err(()) => None,
                    };
                    // A request that does not resolve keeps the Current Chapter as its working
                    // text and targets, as a message with no passage request does. The model
                    // reads the message and decides what to change.
                    let (mut record, resolution) = match targets {
                        Some(targets) => (
                            crate::passage_collection::assemble(client, command, &source, &targets)
                                .await?,
                            storyos_core::OrdinaryPassageResolution::Resolved,
                        ),
                        None => (
                            assemble_current_passage_context(&source),
                            storyos_core::OrdinaryPassageResolution::Clarification,
                        ),
                    };
                    record.operation_requirement.ordinary_resolution = Some(resolution);
                    record
                }
            },
        }
    };
    let mut record = record;
    let bound = bound_instruction(client, command.project_scope, command.run_id).await?;
    storyos_core::assemble_instruction_binding(&mut record, bound.as_ref());
    let sources = crate::skill_package_snapshot::skill_instruction_sources(
        client,
        command.project_scope,
        command.skill_requirements,
    )
    .await
    .map_err(agent_run_database_error)?;
    storyos_core::assemble_skill_instructions(&mut record, sources).map_err(
        |storyos_core::SkillInstructionUnavailable| {
            CreateAgentRunError::Unavailable(Box::new(std::io::Error::other(
                "The SkillPackage Snapshot has no SKILL.md text",
            )))
        },
    )?;
    let catalog = crate::skill_package_snapshot::run_skill_catalog(
        client,
        command.project_scope,
        command.run_id,
    )
    .await
    .map_err(agent_run_database_error)?;
    storyos_core::assemble_skill_catalog(&mut record, &catalog);
    let chapters = crate::manuscript_search::read_live_chapter_blocks(
        client,
        command.project_scope,
        crate::manuscript_search::LiveChapterReadExtent::AllChapters,
    )
    .await
    .map_err(|error| CreateAgentRunError::Unavailable(Box::new(error)))?;
    storyos_core::assemble_chapter_list(
        &mut record,
        &chapters
            .into_iter()
            .map(|chapter| storyos_core::ChapterListEntry {
                volume_order: chapter.volume_order,
                volume_title: chapter.volume_title,
                chapter_id: chapter.chapter_id.as_ref().to_owned(),
                title: chapter.title,
                length: storyos_core::count_stored_texts(
                    chapter.blocks.iter().map(|block| block.text.as_str()),
                )
                .character_count,
            })
            .collect::<Vec<_>>(),
    );
    let memory = match command.memory_revision {
        Some(revision) => {
            crate::memory_entries::read_memory_index(client, command.project_scope, revision)
                .await
                .map_err(CreateAgentRunError::Unavailable)?
        }
        None => Vec::new(),
    };
    storyos_core::assemble_memory_index(&mut record, command.memory_revision, &memory);
    if let Some(candidates) = command.tool_results {
        storyos_core::assemble_tool_result_continuation(&mut record, candidates);
    }
    let assembly_manifest_id = insert_assembly(
        client,
        command.project_scope,
        command.run_id,
        Some(command.receipt_id),
        command.decision_position,
        &record,
    )
    .await?;
    Ok(AgentRunContext {
        assembly_manifest_id,
        destination_context_manifest_id: None,
        outbound_disclosure_manifest_id: None,
        working_target_availability: match chapter_revision_id {
            Some(_) => WorkingTargetAvailability::Current,
            None => WorkingTargetAvailability::Unavailable,
        },
        reference_availability: record
            .operation_requirement
            .selection_references
            .iter()
            .flatten()
            .map(|reference| match reference.selected_text {
                Some(_) => WorkingTargetAvailability::Current,
                None => WorkingTargetAvailability::Unavailable,
            })
            .collect(),
        joinable_block_ids: Vec::new(),
        record,
    })
}

/// The Project Instruction Revision that the Run `run_id` bound at its admission, with its exact
/// text, or `None` for "none configured". A later save does not change it (ADR 0094 section 2).
async fn bound_instruction(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Option<storyos_core::BoundInstruction>, CreateAgentRunError> {
    let row = client
        .query_opt(
            "SELECT revision.instruction_revision_id::text, revision.text
               FROM storyos.agent_runs AS run
               JOIN storyos.project_instruction_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id,
                     revision.instruction_revision_id) =
                    (run.owner_user_id, run.project_id, run.project_instruction_revision_id)
              WHERE run.owner_user_id = $1::text::uuid AND run.project_id = $2::text::uuid
                AND run.run_id = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    Ok(row.map(|row| storyos_core::BoundInstruction {
        revision_id: row.get(/*idx*/ 0),
        text: row.get(/*idx*/ 1),
    }))
}

pub(super) async fn load_assembled_context(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
    decision_position: &str,
) -> Result<AgentRunContext, CreateAgentRunError> {
    let row = client
        .query_opt(
            "SELECT requirement.payload::text,
                    assembly.context_assembly_manifest_id::text,
                    assembly.destination_context_manifest_id::text,
                    assembly.outbound_disclosure_manifest_id::text
               FROM storyos.context_assembly_manifests AS assembly
               JOIN storyos.operation_requirements AS requirement
                 ON (requirement.owner_user_id, requirement.project_id,
                     requirement.operation_requirement_id) =
                    (assembly.owner_user_id, assembly.project_id,
                     assembly.operation_requirement_id)
              WHERE assembly.owner_user_id = $1::text::uuid
                AND assembly.project_id = $2::text::uuid
                AND assembly.run_id = $3::text::uuid
                AND assembly.manifest_role = 'decision' AND assembly.decision_position=$4::text::numeric",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
                &decision_position,
            ],
        )
        .await
        .map_err(agent_run_database_error)?
        .ok_or_else(|| {
            CreateAgentRunError::Unavailable(Box::new(std::io::Error::other(
                "The Context Assembly Manifest is missing",
            )))
        })?;
    let payload: String = row.get(0);
    let value: serde_json::Value = serde_json::from_str(&payload).map_err(agent_run_parse_error)?;
    let record = decode_assembly_record(&value).ok_or_else(|| {
        CreateAgentRunError::Unavailable(Box::new(std::io::Error::other(
            "The Context Assembly Manifest is damaged",
        )))
    })?;
    let current_revision = match record.operation_requirement.chapter_id.as_deref() {
        Some(chapter_id) => current_chapter_payload(client, scope, chapter_id)
            .await?
            .map(|(revision_id, _)| revision_id),
        None => None,
    };
    let working_target_availability = match (
        record.operation_requirement.chapter_revision_id.as_deref(),
        current_revision.as_deref(),
    ) {
        (None, _) | (_, None) => WorkingTargetAvailability::Unavailable,
        (Some(snapshotted), Some(current)) if snapshotted == current => {
            WorkingTargetAvailability::Current
        }
        (Some(_), Some(current)) => WorkingTargetAvailability::Superseded {
            current_revision_id: current.to_owned(),
        },
    };
    let reference_availability = crate::selection_references::availability(
        client,
        scope,
        record
            .operation_requirement
            .selection_references
            .as_deref()
            .unwrap_or_default(),
    )
    .await?;
    Ok(AgentRunContext {
        record,
        reference_availability,
        joinable_block_ids: Vec::new(),
        assembly_manifest_id: row.get(1),
        destination_context_manifest_id: row.get(2),
        outbound_disclosure_manifest_id: row.get(3),
        working_target_availability,
    })
}

pub(crate) async fn load_working_target(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    chapter_id: &str,
) -> Result<(Option<String>, String), CreateAgentRunError> {
    let Some((revision_id, stored)) = current_chapter_payload(client, scope, chapter_id).await?
    else {
        return Ok((None, String::new()));
    };
    let blocks = crate::manuscript_block::load_or_upgrade_blocks(
        client,
        scope.owner_user_id.as_ref(),
        scope.project_id.as_ref(),
        chapter_id,
        &revision_id,
        &stored,
    )
    .await
    .map_err(agent_run_database_error)?;
    Ok((
        Some(revision_id),
        crate::manuscript_block::display_body_from_stored(&stored, &blocks),
    ))
}

pub(crate) async fn current_chapter_payload(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    chapter_id: &str,
) -> Result<Option<(String, String)>, CreateAgentRunError> {
    let row = client
        .query_opt(
            "SELECT revision.revision_id::text, convert_from(payload.canonical_bytes, 'UTF8')
               FROM storyos.manuscript_objects AS object
               JOIN storyos.authoritative_heads AS head
                 ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                    (object.owner_user_id, object.project_id, object.manuscript_object_id)
               JOIN storyos.authoritative_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id,
                     revision.revision_id) =
                    (head.owner_user_id, head.project_id, head.manuscript_object_id,
                     head.current_revision_id)
               JOIN storyos.authoritative_payloads AS payload
                 ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                    (revision.owner_user_id, revision.project_id, revision.payload_id)
              WHERE object.owner_user_id = $1::text::uuid
                AND object.project_id = $2::text::uuid
                AND object.manuscript_object_id = $3::text::uuid
                AND object.object_kind = 'chapter'
                AND NOT EXISTS (
                    SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                     WHERE removal.owner_user_id = object.owner_user_id
                       AND removal.project_id = object.project_id
                       AND removal.chapter_id = object.manuscript_object_id
                )",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &chapter_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    Ok(row.map(|row| (row.get(0), row.get(1))))
}

/// Writes the Operation Requirement and the Context Assembly Manifest of one RunStep, and gives
/// the manifest identifier. A background Run has no Receipt (ADR 0094 section 5).
pub(crate) async fn insert_assembly(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
    receipt_id: Option<&str>,
    decision_position: &str,
    record: &storyos_core::CurrentPassageAssemblyRecord,
) -> Result<String, CreateAgentRunError> {
    let payload = encode_assembly_record(record).to_string();
    client
        .execute(
            "INSERT INTO storyos.operation_requirements
               (owner_user_id, project_id, operation_requirement_id, run_id,
                input_snapshot_id, receipt_id, payload, decision_position)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::jsonb, $8::text::numeric)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &record.operation_requirement.operation_requirement_id,
                &run_id,
                &record.operation_requirement.input_snapshot_id,
                &receipt_id,
                &payload,
                &decision_position,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    let assembly_manifest_id = uuid::Uuid::now_v7().to_string();
    let sufficiency = match record.sufficiency {
        storyos_core::ContextSufficiency::Complete => "complete",
        storyos_core::ContextSufficiency::Blocked { .. } => "blocked",
    };
    client
        .execute(
            "INSERT INTO storyos.context_assembly_manifests
               (owner_user_id, project_id, context_assembly_manifest_id,
                operation_requirement_id, run_id, sufficiency,
                destination_context_manifest_id, outbound_disclosure_manifest_id,
                payload, receipt_id, decision_position)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid,
                     $4::text::uuid, $5::text::uuid, $6,
                     $7::text::uuid, $8::text::uuid, $9::text::jsonb, $10::text::uuid, $11::text::numeric)",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &assembly_manifest_id,
                &record.operation_requirement.operation_requirement_id,
                &run_id,
                &sufficiency,
                &None::<&str>,
                &None::<&str>,
                &payload,
                &receipt_id,
                &decision_position,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    Ok(assembly_manifest_id)
}

#[cfg(test)]
#[path = "create_agent_run_context_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "chapter_list_context_tests.rs"]
pub(crate) mod chapter_list_tests;

#[cfg(test)]
#[path = "memory_index_context_tests.rs"]
mod memory_index_tests;
