use storyos_application::{
    OpenProposalListItem, OpenProposalListReader, ProjectReadError, ProjectScope,
};
use storyos_contracts::OpenProposalSource;

use super::{PostgresProjectReader, read_error, set_scope};

impl OpenProposalListReader for PostgresProjectReader {
    async fn read_open_proposals(
        &self,
        scope: &ProjectScope,
        chapter_id: &str,
        limit: usize,
    ) -> Result<Vec<OpenProposalListItem>, ProjectReadError> {
        let limit = i64::try_from(limit).map_err(ProjectReadError::unavailable)?;
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        // Each producer makes the Proposal identity with UUIDv7, so the identity order is the open order.
        let rows = transaction
            .query(
                "SELECT proposal.proposal_id::text, revision.revision_id::text, proposal.kind,
                        proposal.source_run_id::text, proposal.source_decision_id::text,
                        proposal.source_draft_id::text, proposal.source_draft_revision_id::text
                   FROM storyos.proposals AS proposal
                   JOIN storyos.proposal_heads AS head
                     ON (head.owner_user_id, head.project_id, head.proposal_id) =
                        (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
                   JOIN storyos.proposal_revisions AS revision
                     ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                         revision.revision_id) =
                        (head.owner_user_id, head.project_id, head.proposal_id,
                         head.current_revision_id)
                   LEFT JOIN storyos.draft_artifacts AS source_draft
                     ON (source_draft.owner_user_id, source_draft.project_id, source_draft.draft_id) =
                        (proposal.owner_user_id, proposal.project_id, proposal.source_draft_id)
                  WHERE proposal.owner_user_id = $1::text::uuid
                    AND proposal.project_id = $2::text::uuid
                    AND proposal.chapter_id = $3::text::uuid
                    AND revision.closure = 'open'
                    AND EXISTS (
                      SELECT 1 FROM storyos.proposal_operations AS operation
                       WHERE (operation.owner_user_id, operation.project_id, operation.proposal_id) =
                             (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
                         AND operation.resolution = 'pending')
                    AND (proposal.source_run_id IS NOT NULL
                      OR source_draft.retention_state = 'retained')
                  ORDER BY proposal.proposal_id
                  LIMIT $4",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &chapter_id,
                    &limit,
                ],
            )
            .await
            .map_err(read_error)?;
        transaction.commit().await.map_err(read_error)?;
        rows.iter()
            .map(|row| {
                let source = match (
                    row.get::<_, Option<String>>(3),
                    row.get::<_, Option<String>>(4),
                    row.get::<_, Option<String>>(5),
                    row.get::<_, Option<String>>(6),
                ) {
                    (Some(run_id), Some(decision_id), None, None) => {
                        OpenProposalSource::AgentRunDecision {
                            run_id,
                            decision_id,
                        }
                    }
                    (None, None, Some(draft_id), Some(draft_revision_id)) => {
                        OpenProposalSource::RefusedEditDraft {
                            draft_id,
                            draft_revision_id,
                        }
                    }
                    _ => {
                        return Err(ProjectReadError::unavailable(std::io::Error::other(
                            "the Proposal source shape is invalid",
                        )));
                    }
                };
                Ok(OpenProposalListItem {
                    proposal_id: row.get(0),
                    revision_id: row.get(1),
                    kind: row.get(2),
                    source,
                })
            })
            .collect()
    }
}
