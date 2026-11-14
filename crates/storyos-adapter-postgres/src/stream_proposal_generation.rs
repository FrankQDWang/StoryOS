//! Writes one complete Proposal generation batch of a candidate revision (ADR 0062, ADR 0071).

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::hex_sha256;
use tokio_postgres::Client;
use uuid::Uuid;

pub(crate) struct LoadedGeneration {
    pub(crate) proposal_id: String,
    pub(crate) generation_id: String,
    pub(crate) revision_id: String,
    pub(crate) base_revision_id: String,
}

pub(crate) async fn persist_batch(
    client: &Client,
    claim: &ClaimedAgentRun,
    loaded: &LoadedGeneration,
    stream_seq: u64,
    text: &str,
    operation_id: Option<&str>,
) -> Result<(String, String), CompleteAgentRunError> {
    let owner = claim.project_scope.owner_user_id.as_ref();
    let project = claim.project_scope.project_id.as_ref();
    let revision_id = Uuid::now_v7().to_string();
    let seq = i64::try_from(stream_seq).map_err(|error| unavailable(error.to_string()))?;
    let summary = client.query_one(
        "SELECT proposal.manuscript_block_id::text,
                CASE WHEN $4::text IS NULL OR operation.operation_id=$4::text::uuid
                     THEN $5 ELSE operation.candidate_text END
           FROM storyos.proposals AS proposal
           JOIN storyos.proposal_operations AS operation USING (owner_user_id,project_id,proposal_id)
          WHERE proposal.owner_user_id=$1::text::uuid AND proposal.project_id=$2::text::uuid
            AND proposal.proposal_id=$3::text::uuid
            AND operation.manuscript_block_id=proposal.manuscript_block_id",
        &[&owner, &project, &loaded.proposal_id, &operation_id, &text],
    ).await.map_err(stream_err)?;
    let summary_text: &str = summary.get(1);
    append_revision(
        client,
        owner,
        project,
        &loaded.proposal_id,
        &revision_id,
        summary_text,
        &loaded.revision_id,
        Some(&loaded.base_revision_id),
    )
    .await?;
    client
        .execute(
            "UPDATE storyos.proposal_operations
                SET candidate_text = $4
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid AND reservation_state = 'unresolved'
                AND ($5::text IS NULL OR operation_id=$5::text::uuid)",
            &[&owner, &project, &loaded.proposal_id, &text, &operation_id],
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
    let validation_id = Uuid::now_v7().to_string();
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
                &validation_id,
                &loaded.proposal_id,
                &revision_id,
                &loaded.base_revision_id,
                &summary.get::<_, &str>(0),
                &summary_text,
            ],
        )
        .await
        .map_err(stream_err)?;
    Ok((revision_id, validation_id))
}

#[allow(clippy::too_many_arguments)]
async fn append_revision(
    client: &Client,
    owner: &str,
    project: &str,
    proposal_id: &str,
    revision_id: &str,
    candidate_text: &str,
    parent_revision_id: &str,
    base_revision_id: Option<&str>,
) -> Result<(), CompleteAgentRunError> {
    let inserted = client
        .execute(
            "INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id, parent_revision_id)
             SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 'ready',
                    'valid', 'open', $5, COALESCE($6::text::uuid, base_authoritative_revision_id),
                    $7::text::uuid
               FROM storyos.proposal_revisions
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid AND revision_id = $7::text::uuid",
            &[
                &owner,
                &project,
                &proposal_id,
                &revision_id,
                &candidate_text,
                &base_revision_id,
                &parent_revision_id,
            ],
        )
        .await
        .map_err(stream_err)?;
    if inserted != 1 {
        return Err(unavailable("Proposal revision was not appended"));
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
        return Err(unavailable("Proposal head did not advance"));
    }
    Ok(())
}

fn digest(text: &str) -> String {
    hex_sha256(text.as_bytes())
}

fn unavailable(message: impl Into<String>) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(std::io::Error::other(message.into())))
}

fn stream_err(error: tokio_postgres::Error) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(error))
}
