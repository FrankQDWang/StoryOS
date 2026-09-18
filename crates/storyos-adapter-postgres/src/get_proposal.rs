use storyos_application::{BlockProposalRecord, ProjectReadError, ProjectScope, ProposalReader};

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
                        receipt.validation_receipt_id::text, receipt.result
                   FROM storyos.proposals AS proposal
                   JOIN storyos.proposal_heads AS head
                     ON (head.owner_user_id, head.project_id, head.proposal_id) =
                        (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
                   JOIN storyos.proposal_revisions AS revision
                     ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                         revision.revision_id) =
                        (head.owner_user_id, head.project_id, head.proposal_id,
                         head.current_revision_id)
                   JOIN storyos.proposal_operations AS operation
                     ON (operation.owner_user_id, operation.project_id, operation.proposal_id) =
                        (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
                   LEFT JOIN LATERAL (
                     SELECT receipt.result_payload->>'proposal_validation' AS validation
                       FROM storyos.acceptance_receipts AS acceptance
                       JOIN storyos.domain_receipts AS receipt
                         ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                            (acceptance.owner_user_id, acceptance.project_id, acceptance.acceptance_receipt_id)
                      WHERE (acceptance.owner_user_id, acceptance.project_id, acceptance.proposal_id,
                             acceptance.proposal_revision_id) =
                            (revision.owner_user_id, revision.project_id, revision.proposal_id, revision.revision_id)
                        AND receipt.result_payload->>'proposal_validation' IN ('invalid', 'conflicted')
                      ORDER BY receipt.created_at, receipt.receipt_id LIMIT 1
                   ) AS failure ON true
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
            chapter_id: row.get(8),
            manuscript_block_id: row.get(9),
            base_authoritative_revision_id: row.get(10),
            reservation_state: row.get(11),
            candidate_text: row.get(12),
            source_run_id: row.get(13),
            source_decision_id: row.get(14),
            validation_receipt_id: row.get(15),
            validation_receipt_result: row.get(16),
        }))
    }
}
