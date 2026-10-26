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
    ) -> Result<Option<Vec<OpenProposalListItem>>, ProjectReadError> {
        let limit = i64::try_from(limit).map_err(ProjectReadError::unavailable)?;
        let mut client = self.connect().await?;
        let transaction = client.transaction().await.map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        let chapter = transaction
            .query_opt(
                "SELECT 1 FROM storyos.manuscript_objects AS object
                  WHERE object.owner_user_id = $1::text::uuid AND object.project_id = $2::text::uuid
                    AND object.manuscript_object_id = $3::text::uuid AND object.object_kind = 'chapter'
                    AND NOT EXISTS (
                      SELECT 1 FROM storyos.chapter_removal_decisions AS removal
                       WHERE (removal.owner_user_id, removal.project_id, removal.chapter_id) =
                             (object.owner_user_id, object.project_id, object.manuscript_object_id))",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &chapter_id,
                ],
            )
            .await
            .map_err(read_error)?;
        if chapter.is_none() {
            return Ok(None);
        }
        // Each producer makes the Proposal identity with UUIDv7, so the identity order is the open order.
        let rows = transaction
            .query(
                "SELECT proposal.proposal_id::text AS proposal_id,
                        revision.revision_id::text AS revision_id, proposal.kind,
                        proposal.source_run_id::text AS run_id,
                        proposal.source_decision_id::text AS decision_id
                   FROM storyos.proposals AS proposal
                   JOIN storyos.proposal_heads AS head
                     ON (head.owner_user_id, head.project_id, head.proposal_id) =
                        (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
                   JOIN storyos.proposal_revisions AS revision
                     ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                         revision.revision_id) =
                        (head.owner_user_id, head.project_id, head.proposal_id,
                         head.current_revision_id)
                  WHERE proposal.owner_user_id = $1::text::uuid
                    AND proposal.project_id = $2::text::uuid
                    AND proposal.chapter_id = $3::text::uuid
                    AND revision.closure = 'open'
                    AND EXISTS (
                      SELECT 1 FROM storyos.proposal_operations AS operation
                       WHERE (operation.owner_user_id, operation.project_id, operation.proposal_id) =
                             (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
                         AND operation.resolution = 'pending')
                    AND proposal.source_run_id IS NOT NULL
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
                    row.get::<_, Option<String>>("run_id"),
                    row.get::<_, Option<String>>("decision_id"),
                ) {
                    (Some(run_id), Some(decision_id)) => OpenProposalSource::AgentRunDecision {
                        run_id,
                        decision_id,
                    },
                    _ => {
                        return Err(ProjectReadError::unavailable(std::io::Error::other(
                            "the Proposal source shape is invalid",
                        )));
                    }
                };
                Ok(OpenProposalListItem {
                    proposal_id: row.get("proposal_id"),
                    revision_id: row.get("revision_id"),
                    kind: row.get("kind"),
                    source,
                })
            })
            .collect::<Result<_, _>>()
            .map(Some)
    }
}
