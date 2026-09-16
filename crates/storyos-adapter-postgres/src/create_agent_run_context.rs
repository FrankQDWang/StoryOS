use storyos_application::{
    AgentRunContext, CreateAgentRunCommand, CreateAgentRunError, ProjectScope,
    WorkingTargetAvailability,
};
use storyos_core::{
    CurrentPassageAssembly, InstructionBindingInput, assemble_current_passage_context,
    decode_assembly_record, encode_assembly_record,
};

use super::{agent_run_database_error, agent_run_parse_error};

pub(super) async fn persist_current_passage_assembly(
    client: &tokio_postgres::Client,
    command: &CreateAgentRunCommand,
    destination_identity: &str,
) -> Result<AgentRunContext, CreateAgentRunError> {
    let (chapter_revision_id, chapter_body) =
        load_working_target(client, &command.project_scope, &command.chapter_id).await?;
    let operation_requirement_id = uuid::Uuid::now_v7().to_string();
    let input_snapshot_id = uuid::Uuid::now_v7().to_string();
    let record = assemble_current_passage_context(&CurrentPassageAssembly {
        operation_requirement_id: operation_requirement_id.clone(),
        input_snapshot_id: input_snapshot_id.clone(),
        run_id: command.run_id.clone(),
        owner_user_id: command.project_scope.owner_user_id.as_ref().to_owned(),
        project_id: command.project_scope.project_id.as_ref().to_owned(),
        author_message: command.author_message.clone(),
        chapter_id: command.chapter_id.clone(),
        chapter_revision_id: chapter_revision_id.clone(),
        chapter_body,
        instruction: InstructionBindingInput::Absent,
        destination_identity: destination_identity.to_owned(),
    });
    let payload = encode_assembly_record(&record).to_string();
    client
        .execute(
            "INSERT INTO storyos.operation_requirements
               (owner_user_id, project_id, operation_requirement_id, run_id,
                input_snapshot_id, receipt_id, payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7::text::jsonb)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &operation_requirement_id,
                &command.run_id,
                &input_snapshot_id,
                &command.ids.receipt_id,
                &payload,
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
                payload, receipt_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid,
                     $4::text::uuid, $5::text::uuid, $6,
                     $7::text::uuid, $8::text::uuid, $9::text::jsonb, $10::text::uuid)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &assembly_manifest_id,
                &operation_requirement_id,
                &command.run_id,
                &sufficiency,
                &None::<&str>,
                &None::<&str>,
                &payload,
                &command.ids.receipt_id,
            ],
        )
        .await
        .map_err(agent_run_database_error)?;
    Ok(AgentRunContext {
        record,
        assembly_manifest_id,
        destination_context_manifest_id: None,
        outbound_disclosure_manifest_id: None,
        working_target_availability: match chapter_revision_id {
            Some(_) => WorkingTargetAvailability::Current,
            None => WorkingTargetAvailability::Unavailable,
        },
    })
}

pub(super) async fn load_assembled_context(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    run_id: &str,
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
                AND assembly.run_id = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &run_id,
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
    let current_revision =
        current_chapter_payload(client, scope, &record.operation_requirement.chapter_id)
            .await?
            .map(|(revision_id, _)| revision_id);
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
    Ok(AgentRunContext {
        record,
        assembly_manifest_id: row.get(1),
        destination_context_manifest_id: row.get(2),
        outbound_disclosure_manifest_id: row.get(3),
        working_target_availability,
    })
}

async fn load_working_target(
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

async fn current_chapter_payload(
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
                AND object.object_kind = 'chapter'",
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
