use storyos_application::{ClaimedAgentRun, CompleteAgentRunError, ProjectScope};
use storyos_core::{
    AppendProposalGenerationBatch, AppendProposalGenerationBatchResult, OpenBlockProposal,
    append_proposal_generation_batch, hex_sha256, open_block_proposal, stream_batch_plan,
};
use tokio_postgres::Client;
use uuid::Uuid;

use super::admitted_proposal_target::{AdmittedTarget, load_admitted_targets, load_current_target};

pub(crate) enum StreamWork {
    Hold,
    Continue,
}

pub(crate) struct LoadedGeneration {
    pub(crate) proposal_id: String,
    pub(crate) generation_id: String,
    pub(crate) revision_id: String,
    pub(crate) candidate_text: String,
    pub(crate) last_seq: u64,
    pub(crate) generation_state: String,
    pub(crate) existing_fence: bool,
    pub(crate) block_id: String,
    pub(crate) base_revision_id: String,
}

pub(crate) async fn apply_streamed_proposal(
    client: &Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
    decision_id: &str,
    author_message: &str,
) -> Result<(Option<String>, StreamWork), CompleteAgentRunError> {
    let Some(batches) = stream_batch_plan(author_message) else {
        return Ok((None, StreamWork::Continue));
    };
    let loaded = match load_generation(client, claim).await? {
        Some(current) => current,
        None => {
            let Some(first) = load_admitted_targets(client, claim, chapter_id)
                .await?
                .into_iter()
                .next()
            else {
                return Ok((None, StreamWork::Continue));
            };
            let Some(opened) =
                open_generating(client, claim, chapter_id, decision_id, &first).await?
            else {
                return Ok((None, StreamWork::Continue));
            };
            opened
        }
    };
    if loaded.generation_state != "generating" || loaded.existing_fence {
        return Ok((Some(loaded.proposal_id), StreamWork::Continue));
    }
    let Some((stream_seq, text)) = batches
        .iter()
        .copied()
        .find(|(seq, _)| *seq > loaded.last_seq)
    else {
        return Ok((Some(loaded.proposal_id), StreamWork::Continue));
    };
    let existing = existing_batch_digest(
        client,
        &claim.project_scope,
        &loaded.generation_id,
        stream_seq,
    )
    .await?;
    match append_proposal_generation_batch(&AppendProposalGenerationBatch {
        scope_matches: true,
        generation_state: loaded.generation_state.clone(),
        last_applied_stream_seq: loaded.last_seq,
        stream_seq,
        expected_previous_stream_seq: loaded.last_seq,
        expected_proposal_revision_id: loaded.revision_id.clone(),
        current_proposal_revision_id: loaded.revision_id.clone(),
        expected_candidate_digest: digest(&loaded.candidate_text),
        current_candidate_digest: digest(&loaded.candidate_text),
        batch_digest: digest(text),
        existing_batch_digest: existing,
        reservation_owns_target: true,
    }) {
        AppendProposalGenerationBatchResult::Applied => {
            persist_batch(
                client,
                claim,
                &loaded,
                stream_seq,
                text,
                !batches.iter().any(|(seq, _)| *seq > stream_seq),
            )
            .await?;
        }
        AppendProposalGenerationBatchResult::Duplicate => {
            remember_duplicate_seq(client, claim, &loaded.generation_id, stream_seq).await?;
        }
        AppendProposalGenerationBatchResult::Wait
        | AppendProposalGenerationBatchResult::Refused { .. }
        | AppendProposalGenerationBatchResult::Conflicted { .. } => {
            return Ok((Some(loaded.proposal_id), StreamWork::Continue));
        }
    }
    Ok((
        Some(loaded.proposal_id),
        if batches.iter().any(|(seq, _)| *seq > stream_seq) {
            StreamWork::Hold
        } else {
            StreamWork::Continue
        },
    ))
}

async fn open_generating(
    client: &Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
    decision_id: &str,
    first: &AdmittedTarget,
) -> Result<Option<LoadedGeneration>, CompleteAgentRunError> {
    let current = load_current_target(client, claim, chapter_id, &first.block_id).await?;
    if open_block_proposal(&OpenBlockProposal {
        scope_matches: true,
        target_block_present: current.revision_id.is_some(),
        expected_base_revision_id: first.revision_id.clone(),
        current_base_revision_id: current.revision_id,
        conflicting_reservation: current.reserved,
    })
    .validation_receipt_result()
    .is_none()
    {
        return Ok(None);
    }
    let owner = claim.project_scope.owner_user_id.as_ref();
    let project = claim.project_scope.project_id.as_ref();
    let proposal_id = Uuid::now_v7().to_string();
    let revision_id = Uuid::now_v7().to_string();
    let generation_id = Uuid::now_v7().to_string();
    client
        .execute(
            "INSERT INTO storyos.proposals
               (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                source_run_id, source_decision_id, bundle_policy)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 'block_edit',
                     $4::text::uuid, $5::text::uuid, $6::text::uuid, $7::text::uuid, 'none')",
            &[
                &owner,
                &project,
                &proposal_id,
                &chapter_id,
                &first.block_id,
                &claim.run_id,
                &decision_id,
            ],
        )
        .await
        .map_err(stream_err)?;
    client
        .execute(
            "INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 'generating',
                     'pending', 'open', '', $5::text::uuid)",
            &[
                &owner,
                &project,
                &proposal_id,
                &revision_id,
                &first.revision_id,
            ],
        )
        .await
        .map_err(stream_err)?;
    client
        .execute(
            "INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid)",
            &[&owner, &project, &proposal_id, &revision_id],
        )
        .await
        .map_err(stream_err)?;
    client
        .execute(
            "INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text, predecessor_operation_ids)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'pending', 'unresolved', '', '{}'::uuid[])",
            &[
                &owner,
                &project,
                &proposal_id,
                &Uuid::now_v7().to_string(),
                &first.block_id,
            ],
        )
        .await
        .map_err(stream_err)?;
    client
        .execute(
            "INSERT INTO storyos.proposal_generations
               (owner_user_id, project_id, generation_id, proposal_id, last_applied_stream_seq)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 0)",
            &[&owner, &project, &generation_id, &proposal_id],
        )
        .await
        .map_err(stream_err)?;
    Ok(Some(LoadedGeneration {
        proposal_id,
        generation_id,
        revision_id,
        candidate_text: String::new(),
        last_seq: 0,
        generation_state: "generating".to_owned(),
        existing_fence: false,
        block_id: first.block_id.clone(),
        base_revision_id: first.revision_id.clone(),
    }))
}

async fn persist_batch(
    client: &Client,
    claim: &ClaimedAgentRun,
    loaded: &LoadedGeneration,
    stream_seq: u64,
    text: &str,
    complete: bool,
) -> Result<(), CompleteAgentRunError> {
    let owner = claim.project_scope.owner_user_id.as_ref();
    let project = claim.project_scope.project_id.as_ref();
    let revision_id = Uuid::now_v7().to_string();
    let seq = i64::try_from(stream_seq).map_err(|error| unavailable(error.to_string()))?;
    let (generation, validation) = if complete {
        ("ready", "valid")
    } else {
        ("generating", "pending")
    };
    append_revision(
        client,
        owner,
        project,
        &loaded.proposal_id,
        &revision_id,
        generation,
        validation,
        text,
        &loaded.revision_id,
        Some(&loaded.base_revision_id),
    )
    .await?;
    client
        .execute(
            "UPDATE storyos.proposal_operations
                SET candidate_text = $4
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid AND reservation_state = 'unresolved'",
            &[&owner, &project, &loaded.proposal_id, &text],
        )
        .await
        .map_err(stream_err)?;
    client
        .execute(
            "INSERT INTO storyos.proposal_stream_events
               (owner_user_id, project_id, generation_id, stream_seq, proposal_id, revision_id,
                batch_digest, candidate_text)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4, $5::text::uuid,
                     $6::text::uuid, $7, $8)",
            &[
                &owner,
                &project,
                &loaded.generation_id,
                &seq,
                &loaded.proposal_id,
                &revision_id,
                &digest(text),
                &text,
            ],
        )
        .await
        .map_err(stream_err)?;
    client
        .execute(
            "UPDATE storyos.proposal_generations
                SET last_applied_stream_seq = $4
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND generation_id = $3::text::uuid",
            &[&owner, &project, &loaded.generation_id, &seq],
        )
        .await
        .map_err(stream_err)?;
    if complete {
        client
            .execute(
                "INSERT INTO storyos.validation_receipts
                   (owner_user_id, project_id, validation_receipt_id, proposal_id,
                    proposal_revision_id, result, base_authoritative_revision_id,
                    manuscript_block_id, candidate_text, reservation_state)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid, 'valid', $6::text::uuid, $7::text::uuid, $8, 'unresolved')",
                &[
                    &owner,
                    &project,
                    &Uuid::now_v7().to_string(),
                    &loaded.proposal_id,
                    &revision_id,
                    &loaded.base_revision_id,
                    &loaded.block_id,
                    &text,
                ],
            )
            .await
            .map_err(stream_err)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn append_revision(
    client: &Client,
    owner: &str,
    project: &str,
    proposal_id: &str,
    revision_id: &str,
    generation: &str,
    validation: &str,
    candidate_text: &str,
    parent_revision_id: &str,
    base_revision_id: Option<&str>,
) -> Result<(), CompleteAgentRunError> {
    let inserted = client
        .execute(
            "INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id, parent_revision_id)
             SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5, $6,
                    'open', $7, COALESCE($8::text::uuid, base_authoritative_revision_id),
                    $9::text::uuid
               FROM storyos.proposal_revisions
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid AND revision_id = $9::text::uuid",
            &[
                &owner,
                &project,
                &proposal_id,
                &revision_id,
                &generation,
                &validation,
                &candidate_text,
                &base_revision_id,
                &parent_revision_id,
            ],
        )
        .await
        .map_err(stream_err)?;
    if inserted != 1 {
        return Err(unavailable("Streamed Proposal revision was not appended"));
    }
    let updated = client
        .execute(
            "UPDATE storyos.proposal_heads
                SET current_revision_id = $4::text::uuid
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid AND current_revision_id = $5::text::uuid",
            &[
                &owner,
                &project,
                &proposal_id,
                &revision_id,
                &parent_revision_id,
            ],
        )
        .await
        .map_err(stream_err)?;
    if updated != 1 {
        return Err(unavailable("Streamed Proposal head did not advance"));
    }
    Ok(())
}

async fn load_generation(
    client: &Client,
    claim: &ClaimedAgentRun,
) -> Result<Option<LoadedGeneration>, CompleteAgentRunError> {
    let row = client
        .query_opt(
            "SELECT proposal.proposal_id::text, generation.generation_id::text,
                    generation.last_applied_stream_seq, head.current_revision_id::text,
                    revision.candidate_text, revision.generation,
                    fence.editor_input_fence_id IS NOT NULL,
                    proposal.manuscript_block_id::text,
                    revision.base_authoritative_revision_id::text
               FROM storyos.proposals AS proposal
               JOIN storyos.proposal_generations AS generation
                 ON (generation.owner_user_id, generation.project_id, generation.proposal_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
               JOIN storyos.proposal_heads AS head
                 ON (head.owner_user_id, head.project_id, head.proposal_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.revision_id) =
                    (head.owner_user_id, head.project_id, head.proposal_id,
                     head.current_revision_id)
               LEFT JOIN storyos.editor_input_fences AS fence
                 ON (fence.owner_user_id, fence.project_id, fence.generation_id) =
                    (generation.owner_user_id, generation.project_id, generation.generation_id)
              WHERE proposal.owner_user_id = $1::text::uuid
                AND proposal.project_id = $2::text::uuid
                AND proposal.source_run_id = $3::text::uuid
              ORDER BY generation.generation_id
              LIMIT 1",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &claim.run_id,
            ],
        )
        .await
        .map_err(stream_err)?;
    Ok(match row {
        Some(row) => Some(LoadedGeneration {
            proposal_id: row.get(0),
            generation_id: row.get(1),
            last_seq: u64::try_from(row.get::<_, i64>(2))
                .map_err(|error| unavailable(error.to_string()))?,
            revision_id: row.get(3),
            candidate_text: row.get(4),
            generation_state: row.get(5),
            existing_fence: row.get(6),
            block_id: row.get(7),
            base_revision_id: row.get(8),
        }),
        None => None,
    })
}

async fn remember_duplicate_seq(
    client: &Client,
    claim: &ClaimedAgentRun,
    generation_id: &str,
    stream_seq: u64,
) -> Result<(), CompleteAgentRunError> {
    let seq = i64::try_from(stream_seq).map_err(|error| unavailable(error.to_string()))?;
    client
        .execute(
            "UPDATE storyos.proposal_generations
                SET last_applied_stream_seq = $4
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND generation_id = $3::text::uuid AND last_applied_stream_seq < $4",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &generation_id,
                &seq,
            ],
        )
        .await
        .map_err(stream_err)?;
    Ok(())
}

async fn existing_batch_digest(
    client: &Client,
    scope: &ProjectScope,
    generation_id: &str,
    stream_seq: u64,
) -> Result<Option<String>, CompleteAgentRunError> {
    let seq = i64::try_from(stream_seq).map_err(|error| unavailable(error.to_string()))?;
    Ok(client
        .query_opt(
            "SELECT batch_digest
               FROM storyos.proposal_stream_events
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND generation_id = $3::text::uuid AND stream_seq = $4",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &generation_id,
                &seq,
            ],
        )
        .await
        .map_err(stream_err)?
        .map(|row| row.get(0)))
}

pub(crate) fn digest(text: &str) -> String {
    hex_sha256(text.as_bytes())
}

fn unavailable(message: impl Into<String>) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(std::io::Error::other(message.into())))
}

fn stream_err(error: tokio_postgres::Error) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(error))
}
