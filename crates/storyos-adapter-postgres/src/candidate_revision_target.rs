use storyos_application::{CreateAgentRunError, ProjectScope};
use storyos_core::{OpenBlockProposal, ProposalCandidateTarget, open_block_proposal};

pub(crate) struct CandidateTarget {
    pub block_id: String,
    pub base_revision_id: String,
    pub text: String,
}

pub(crate) async fn load(
    client: &tokio_postgres::Client,
    scope: &ProjectScope,
    chapter_id: &str,
    target: &ProposalCandidateTarget,
) -> Result<Option<CandidateTarget>, CreateAgentRunError> {
    let row = client.query_opt(
        "SELECT operation.manuscript_block_id::text, revision.base_authoritative_revision_id::text,
                operation.candidate_text, authoritative.current_revision_id::text
           FROM storyos.proposals AS proposal
           JOIN storyos.proposal_heads AS head USING (owner_user_id,project_id,proposal_id)
           JOIN storyos.proposal_revisions AS revision ON
             (revision.owner_user_id,revision.project_id,revision.proposal_id,revision.revision_id)=
             (head.owner_user_id,head.project_id,head.proposal_id,head.current_revision_id)
           JOIN storyos.proposal_operations AS operation ON
             (operation.owner_user_id,operation.project_id,operation.proposal_id)=
             (proposal.owner_user_id,proposal.project_id,proposal.proposal_id)
           JOIN storyos.authoritative_heads AS authoritative ON
             (authoritative.owner_user_id,authoritative.project_id,authoritative.manuscript_object_id)=
             (proposal.owner_user_id,proposal.project_id,proposal.chapter_id)
          WHERE proposal.owner_user_id=$1::text::uuid AND proposal.project_id=$2::text::uuid
            AND proposal.chapter_id=$3::text::uuid AND proposal.proposal_id=$4::text::uuid
            AND operation.operation_id=$5::text::uuid AND head.current_revision_id=$6::text::uuid
            AND proposal.kind IN ('block_edit','inline_edit') AND revision.closure='open'
            AND revision.generation IN ('ready','ready_partial')
            AND operation.resolution='pending' AND operation.reservation_state='unresolved'
            AND operation.candidate_blocks IS NULL AND revision.candidate_blocks IS NULL
            AND NOT EXISTS (SELECT 1 FROM storyos.chapter_removal_decisions AS removal
              WHERE removal.owner_user_id=proposal.owner_user_id AND removal.project_id=proposal.project_id
                AND removal.chapter_id=proposal.chapter_id)
            AND EXISTS (SELECT 1 FROM storyos.manuscript_revision_members AS member
              WHERE (member.owner_user_id,member.project_id,member.manuscript_object_id,member.revision_id)=
                (authoritative.owner_user_id,authoritative.project_id,authoritative.manuscript_object_id,authoritative.current_revision_id)
                AND member.manuscript_block_id=operation.manuscript_block_id)
          FOR UPDATE OF head, operation",
        &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref(), &chapter_id,
          &target.proposal_id, &target.operation_id, &target.revision_id],
    ).await.map_err(super::create_agent_run::agent_run_database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let loaded = CandidateTarget {
        block_id: row.get(0),
        base_revision_id: row.get(1),
        text: row.get(2),
    };
    if !matches!(
        open_block_proposal(&OpenBlockProposal {
            scope_matches: true,
            target_block_present: true,
            expected_base_revision_id: loaded.base_revision_id.clone(),
            current_base_revision_id: Some(row.get(3)),
            conflicting_reservation: false,
        }),
        storyos_core::OpenBlockProposalResult::Applied
    ) {
        return Ok(None);
    }
    Ok(Some(loaded))
}

pub(crate) async fn admitted(
    client: &tokio_postgres::Client,
    claim: &storyos_application::ClaimedAgentRun,
) -> Result<
    Option<storyos_core::CurrentPassageAssemblyRecord>,
    storyos_application::CompleteAgentRunError,
> {
    let row = client
        .query_one(
            "SELECT payload::text FROM storyos.operation_requirements
          WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid
            AND run_id=$3::text::uuid AND requirement_role='primary'",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(super::agent_run_work::complete_database_error)?;
    let payload: serde_json::Value =
        serde_json::from_str(&row.get::<_, String>(0)).map_err(|error| {
            storyos_application::CompleteAgentRunError::Unavailable(Box::new(error))
        })?;
    Ok(storyos_core::decode_assembly_record(&payload)
        .filter(|record| record.operation_requirement.candidate_target.is_some()))
}
