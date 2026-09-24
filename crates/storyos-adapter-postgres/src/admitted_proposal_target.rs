use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{ContextSourceClass, ContextSufficiency, decode_assembly_record};

pub(crate) struct AdmittedTarget {
    pub block_id: String,
    pub revision_id: String,
    pub block_text: String,
}

pub(crate) struct CurrentTarget {
    pub revision_id: Option<String>,
    pub reserved: bool,
}

pub(crate) async fn load_admitted_targets(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
) -> Result<Vec<AdmittedTarget>, CompleteAgentRunError> {
    let row = client
        .query_opt(
            "SELECT requirement.payload::text
               FROM storyos.operation_requirements AS requirement
              WHERE requirement.owner_user_id = $1::text::uuid
                AND requirement.project_id = $2::text::uuid
                AND requirement.run_id = $3::text::uuid",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(database_error)?;
    let Some(row) = row else {
        return Ok(Vec::new());
    };
    let payload: serde_json::Value = serde_json::from_str(&row.get::<_, String>(0))
        .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    let Some(record) = decode_assembly_record(&payload) else {
        return Ok(Vec::new());
    };
    let requirement = &record.operation_requirement;
    let Some(revision_id) = requirement.chapter_revision_id.as_deref() else {
        return Ok(Vec::new());
    };
    if requirement.run_id != claim.run_id
        || requirement.owner_user_id != claim.project_scope.owner_user_id.as_ref()
        || requirement.project_id != claim.project_scope.project_id.as_ref()
        || requirement.chapter_id != chapter_id
        || !matches!(record.sufficiency, ContextSufficiency::Complete)
        || !record.selected.iter().any(|source| {
            source.source_class == ContextSourceClass::WorkingTarget
                && source.source_version == revision_id
        })
    {
        return Ok(Vec::new());
    }
    let rows = client
        .query(
            "SELECT member.manuscript_block_id::text,
                    convert_from(payload.canonical_bytes, 'UTF8')
               FROM storyos.authoritative_revisions AS revision
               JOIN storyos.authoritative_payloads AS payload
                 ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                    (revision.owner_user_id, revision.project_id, revision.payload_id)
               JOIN storyos.manuscript_revision_members AS member
                 ON (member.owner_user_id, member.project_id, member.manuscript_object_id,
                     member.revision_id) =
                    (revision.owner_user_id, revision.project_id, revision.manuscript_object_id,
                     revision.revision_id)
              WHERE revision.owner_user_id = $1::text::uuid
                AND revision.project_id = $2::text::uuid
                AND revision.manuscript_object_id = $3::text::uuid
                AND revision.revision_id = $4::text::uuid
              ORDER BY member.block_order",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &chapter_id,
                &revision_id,
            ],
        )
        .await
        .map_err(database_error)?;
    let ids: Vec<String> = rows.iter().map(|row| row.get(0)).collect();
    let Some(first) = rows.first() else {
        return Ok(Vec::new());
    };
    let stored: String = first.get(1);
    let blocks = crate::manuscript_block::blocks_from_stored_payload(&stored, &ids);
    if blocks.len() != ids.len() {
        return Ok(Vec::new());
    }
    let all: Vec<AdmittedTarget> = ids
        .into_iter()
        .zip(blocks)
        .map(|(block_id, block)| AdmittedTarget {
            block_id,
            revision_id: revision_id.to_owned(),
            block_text: block.text,
        })
        .collect();
    let Some(selected_ids) = requirement.proposal_target_block_ids.as_ref() else {
        return Ok(if all.len() == 1 { all } else { Vec::new() });
    };
    let selected: Vec<AdmittedTarget> = all
        .into_iter()
        .filter(|target| selected_ids.contains(&target.block_id))
        .collect();
    if selected.len() != selected_ids.len()
        || !selected
            .iter()
            .map(|target| &target.block_id)
            .eq(selected_ids.iter())
    {
        return Ok(Vec::new());
    }
    Ok(selected)
}

pub(crate) async fn load_current_target(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
    block_id: &str,
) -> Result<CurrentTarget, CompleteAgentRunError> {
    let row = client
        .query_opt(
            "SELECT member.revision_id::text,
                    EXISTS (
                      SELECT 1 FROM storyos.proposal_operations AS reservation
                       WHERE reservation.owner_user_id = member.owner_user_id
                         AND reservation.project_id = member.project_id
                         AND reservation.manuscript_block_id = member.manuscript_block_id
                         AND reservation.reservation_state = 'unresolved'
                    )
               FROM storyos.authoritative_heads AS head
               JOIN storyos.manuscript_revision_members AS member
                 ON (member.owner_user_id, member.project_id, member.manuscript_object_id,
                     member.revision_id) =
                    (head.owner_user_id, head.project_id, head.manuscript_object_id,
                     head.current_revision_id)
              WHERE head.owner_user_id = $1::text::uuid
                AND head.project_id = $2::text::uuid
                AND head.manuscript_object_id = $3::text::uuid
                AND member.manuscript_block_id = $4::text::uuid
                AND NOT EXISTS (
                  SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                   WHERE removal.owner_user_id = head.owner_user_id
                     AND removal.project_id = head.project_id
                     AND removal.chapter_id = head.manuscript_object_id
                )",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &chapter_id,
                &block_id,
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(match row {
        Some(row) => CurrentTarget {
            revision_id: Some(row.get(0)),
            reserved: row.get(1),
        },
        None => CurrentTarget {
            revision_id: None,
            reserved: false,
        },
    })
}

fn database_error(error: tokio_postgres::Error) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(error))
}
