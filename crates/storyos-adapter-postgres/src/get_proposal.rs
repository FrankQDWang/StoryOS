use storyos_application::{
    BlockProposalRecord, ProjectReadError, ProjectScope, ProposalOperationRecord, ProposalReader,
};

use super::{PostgresProjectReader, read_error, set_scope};

impl ProposalReader for PostgresProjectReader {
    async fn read_proposal(
        &self,
        scope: &ProjectScope,
        proposal_id: &str,
    ) -> Result<Option<BlockProposalRecord>, ProjectReadError> {
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        let row = transaction
            .query_opt(
                "SELECT proposal.proposal_id::text, proposal.kind, revision.revision_id::text,
                        revision.generation, COALESCE(failure.validation, revision.validation), revision.closure,
                        operation.operation_id::text, operation.resolution,
                        proposal.chapter_id::text, proposal.manuscript_block_id::text,
                        revision.base_authoritative_revision_id::text,
                        operation.reservation_state, revision.candidate_text,
                        proposal.source_run_id::text, proposal.source_decision_id::text,
                        receipt.validation_receipt_id::text, receipt.result,
                        failure.conflict_id::text
                   FROM storyos.proposals AS proposal
                   JOIN storyos.proposal_heads AS head
                     ON (head.owner_user_id, head.project_id, head.proposal_id) =
                        (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
                   JOIN storyos.proposal_revisions AS revision
                     ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                         revision.revision_id) =
                        (head.owner_user_id, head.project_id, head.proposal_id,
                         head.current_revision_id)
                   JOIN LATERAL (
                     SELECT operation.operation_id, operation.resolution, operation.reservation_state
                       FROM storyos.proposal_operations AS operation
                      WHERE (operation.owner_user_id, operation.project_id, operation.proposal_id) =
                            (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
                      ORDER BY operation.operation_id
                      LIMIT 1
                   ) AS operation ON true
                   LEFT JOIN storyos.proposal_validation_conditions AS failure
                     ON (failure.owner_user_id, failure.project_id, failure.proposal_id,
                         failure.proposal_revision_id) =
                        (revision.owner_user_id, revision.project_id, revision.proposal_id, revision.revision_id)
                   LEFT JOIN storyos.validation_receipts AS receipt
                     ON (receipt.owner_user_id, receipt.project_id, receipt.proposal_id,
                         receipt.proposal_revision_id) =
                        (revision.owner_user_id, revision.project_id, revision.proposal_id,
                         revision.revision_id)
                  WHERE proposal.owner_user_id = $1::text::uuid
                    AND proposal.project_id = $2::text::uuid
                    AND proposal.proposal_id = $3::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &proposal_id,
                ],
            )
            .await
            .map_err(read_error)?;
        let latest_acceptance_refusal =
            crate::acceptance_refusal::read_latest_refusal(&transaction, scope, proposal_id)
                .await?;
        let operations = read_operations(&transaction, scope, proposal_id).await?;
        let anchors = match row.as_ref() {
            Some(row) if row.get::<_, String>(1) == "inline_edit" => {
                read_inline_anchors(&transaction, scope, proposal_id).await?
            }
            _ => Vec::new(),
        };
        transaction.commit().await.map_err(read_error)?;
        Ok(row.map(|row| BlockProposalRecord {
            project_scope: scope.clone(),
            proposal_id: row.get(0),
            kind: row.get(1),
            revision_id: row.get(2),
            generation: row.get(3),
            validation: row.get(4),
            closure: row.get(5),
            operation_id: row.get(6),
            operation_resolution: row.get(7),
            operations,
            chapter_id: row.get(8),
            manuscript_block_id: row.get(9),
            base_authoritative_revision_id: row.get(10),
            reservation_state: row.get(11),
            candidate_text: row.get(12),
            source_run_id: row.get(13),
            source_decision_id: row.get(14),
            validation_receipt_id: row.get(15),
            validation_receipt_result: row.get(16),
            condition_refs: row.get::<_, Option<String>>(17).into_iter().collect(),
            latest_acceptance_refusal,
            anchors,
        }))
    }
}

async fn read_operations(
    client: &impl tokio_postgres::GenericClient,
    scope: &ProjectScope,
    proposal_id: &str,
) -> Result<Vec<ProposalOperationRecord>, ProjectReadError> {
    let rows = client
        .query(
            "SELECT operation_id::text, manuscript_block_id::text, resolution, reservation_state
               FROM storyos.proposal_operations
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid
              ORDER BY operation_id",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &proposal_id,
            ],
        )
        .await
        .map_err(read_error)?;
    Ok(rows
        .into_iter()
        .map(|row| ProposalOperationRecord {
            operation_id: row.get(0),
            manuscript_block_id: row.get(1),
            resolution: row.get(2),
            reservation_state: row.get(3),
        })
        .collect())
}

async fn read_inline_anchors(
    client: &impl tokio_postgres::GenericClient,
    scope: &ProjectScope,
    proposal_id: &str,
) -> Result<Vec<storyos_application::ProposalAnchorRecord>, ProjectReadError> {
    let rows = client
        .query(
            "SELECT manuscript_block_id::text, base_authoritative_revision_id::text,
                    manuscript_schema_version, coordinate_profile, range_from, range_to,
                    boundary_profile, base_slice_digest
               FROM storyos.proposal_anchors
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid
              ORDER BY anchor_order",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &proposal_id,
            ],
        )
        .await
        .map_err(read_error)?;
    rows.into_iter()
        .map(|row| {
            Ok(storyos_application::ProposalAnchorRecord {
                manuscript_block_id: row.get(0),
                base_authoritative_revision_id: row.get(1),
                manuscript_schema_version: u32::try_from(row.get::<_, i32>(2))
                    .map_err(ProjectReadError::unavailable)?,
                coordinate_profile: row.get(3),
                from: u32::try_from(row.get::<_, i32>(4)).map_err(ProjectReadError::unavailable)?,
                to: u32::try_from(row.get::<_, i32>(5)).map_err(ProjectReadError::unavailable)?,
                boundary_profile: row.get(6),
                base_slice_digest: row.get(7),
            })
        })
        .collect()
}
