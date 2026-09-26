use storyos_application::{ApplyAuthorEditCommand, AuthorEditError};
use storyos_core::{
    PauseProposalGeneration, PauseProposalGenerationResult, pause_proposal_generation,
};
use tokio_postgres::Client;
use uuid::Uuid;

use super::author_edit::author_edit_database_error;
use super::stream_proposal_generation::{LoadedGeneration, append_revision, digest};

pub(crate) async fn pause_generating_proposals(
    client: &Client,
    command: &ApplyAuthorEditCommand,
) -> Result<(), AuthorEditError> {
    let rows = client
        .query(
            "SELECT proposal.proposal_id::text, generation.generation_id::text,
                    head.current_revision_id::text, revision.candidate_text,
                    generation.last_applied_stream_seq,
                    fence.editor_input_fence_id IS NOT NULL
               FROM storyos.proposals AS proposal
               JOIN storyos.proposal_heads AS head
                 ON (head.owner_user_id, head.project_id, head.proposal_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.revision_id) =
                    (head.owner_user_id, head.project_id, head.proposal_id,
                     head.current_revision_id)
               JOIN storyos.proposal_generation_heads AS generation_head
                 ON (generation_head.owner_user_id, generation_head.project_id,
                     generation_head.proposal_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
               JOIN storyos.proposal_generations AS generation
                 ON (generation.owner_user_id, generation.project_id, generation.generation_id) =
                    (generation_head.owner_user_id, generation_head.project_id,
                     generation_head.generation_id)
               LEFT JOIN storyos.editor_input_fences AS fence
                 ON (fence.owner_user_id, fence.project_id, fence.generation_id) =
                    (generation.owner_user_id, generation.project_id, generation.generation_id)
              WHERE proposal.owner_user_id = $1::text::uuid
                AND proposal.project_id = $2::text::uuid
                AND proposal.chapter_id = $3::text::uuid
                AND revision.generation = 'generating'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.chapter_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    for row in rows {
        persist_pause(
            client,
            command,
            &LoadedGeneration {
                proposal_id: row.get(0),
                generation_id: row.get(1),
                revision_id: row.get(2),
                candidate_text: row.get(3),
                last_seq: u64::try_from(row.get::<_, i64>(4))
                    .map_err(|error| AuthorEditError::Unavailable(Box::new(error)))?,
                generation_state: "generating".to_owned(),
                existing_fence: row.get(5),
                block_id: String::new(),
                base_revision_id: String::new(),
            },
        )
        .await?;
    }
    Ok(())
}

async fn persist_pause(
    client: &Client,
    command: &ApplyAuthorEditCommand,
    loaded: &LoadedGeneration,
) -> Result<(), AuthorEditError> {
    if !matches!(
        pause_proposal_generation(&PauseProposalGeneration {
            scope_matches: true,
            generation_state: loaded.generation_state.clone(),
            expected_proposal_revision_id: loaded.revision_id.clone(),
            current_proposal_revision_id: loaded.revision_id.clone(),
            expected_candidate_digest: digest(&loaded.candidate_text),
            current_candidate_digest: digest(&loaded.candidate_text),
            last_applied_stream_seq: loaded.last_seq,
            admitted_through_seq: loaded.last_seq,
            existing_fence: loaded.existing_fence,
        }),
        PauseProposalGenerationResult::Applied { .. }
    ) {
        return Ok(());
    }
    let owner = command.project_scope.owner_user_id.as_ref();
    let project = command.project_scope.project_id.as_ref();
    let fence_id = Uuid::now_v7().to_string();
    let revision_id = Uuid::now_v7().to_string();
    let writer = i64::try_from(command.writer_generation)
        .map_err(|error| AuthorEditError::Unavailable(Box::new(error)))?;
    let intent = i64::try_from(command.local_intent_sequence)
        .map_err(|error| AuthorEditError::Unavailable(Box::new(error)))?;
    let admitted = i64::try_from(loaded.last_seq)
        .map_err(|error| AuthorEditError::Unavailable(Box::new(error)))?;
    client
        .execute(
            "INSERT INTO storyos.editor_input_fences
               (owner_user_id, project_id, editor_input_fence_id, editor_session_id,
                writer_generation, local_intent_sequence, proposal_id, generation_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5, $6, $7::text::uuid, $8::text::uuid)",
            &[
                &owner,
                &project,
                &fence_id,
                &command.editor_session_id.as_ref(),
                &writer,
                &intent,
                &loaded.proposal_id,
                &loaded.generation_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    append_revision(
        client,
        owner,
        project,
        &loaded.proposal_id,
        &revision_id,
        "ready_partial",
        "pending",
        &loaded.candidate_text,
        &loaded.revision_id,
        None,
    )
    .await
    .map_err(|error| AuthorEditError::Unavailable(Box::new(error)))?;
    client
        .execute(
            "INSERT INTO storyos.proposal_pause_fences
               (owner_user_id, project_id, proposal_pause_fence_id, proposal_id, generation_id,
                proposal_revision_id, admitted_through_stream_seq, editor_input_fence_id,
                projection_checkpoint_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::uuid, $7, $8::text::uuid, $6::text::uuid)",
            &[
                &owner,
                &project,
                &Uuid::now_v7().to_string(),
                &loaded.proposal_id,
                &loaded.generation_id,
                &revision_id,
                &admitted,
                &fence_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    Ok(())
}
