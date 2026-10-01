use super::agent_run_work::complete_database_error as database_error;
use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_contracts::{ProseChangeLocationInspect, ProseChangeLocationOutcome};
use storyos_core::{
    AppendProposalGenerationBatch, AppendProposalGenerationBatchResult,
    CurrentPassageAssemblyRecord, ProseChangeCandidate, append_proposal_generation_batch,
    hex_sha256,
};

pub(crate) async fn apply(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    record: &CurrentPassageAssemblyRecord,
    candidates: &[ProseChangeCandidate],
) -> Result<super::open_block_proposal::ProseOpening, CompleteAgentRunError> {
    let target = record
        .operation_requirement
        .candidate_target
        .as_ref()
        .expect("admitted candidate");
    let mut result = super::open_block_proposal::ProseOpening {
        proposal_id: None,
        locations: Some(Vec::new()),
    };
    let Some(candidate) = candidates.first() else {
        return Ok(result);
    };
    let loaded = super::candidate_revision_target::load(
        client,
        &claim.project_scope,
        &candidate.chapter_id,
        target,
    )
    .await
    .map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    let outcome = if let Some(loaded) = loaded {
        let previous = record
            .selected
            .iter()
            .find(|source| source.source_version == target.revision_id)
            .map(|source| source.content.as_str())
            .unwrap_or_default();
        let generation_id = uuid::Uuid::now_v7().to_string();
        let owner = claim.project_scope.owner_user_id.as_ref();
        let project = claim.project_scope.project_id.as_ref();
        if !matches!(
            append_proposal_generation_batch(&AppendProposalGenerationBatch {
                scope_matches: true,
                generation_state: "generating".to_owned(),
                last_applied_stream_seq: 0,
                stream_seq: 1,
                expected_previous_stream_seq: 0,
                expected_proposal_revision_id: target.revision_id.clone(),
                current_proposal_revision_id: target.revision_id.clone(),
                expected_candidate_digest: hex_sha256(previous.as_bytes()),
                current_candidate_digest: hex_sha256(loaded.text.as_bytes()),
                batch_digest: hex_sha256(candidate.candidate_text.as_bytes()),
                existing_batch_digest: None,
                reservation_owns_target: loaded.block_id == candidate.manuscript_block_id
                    && loaded.base_revision_id == candidate.base_authoritative_revision_id,
            }),
            AppendProposalGenerationBatchResult::Applied
        ) {
            ProseChangeLocationOutcome::Refused {
                reason: "stale_candidate".to_owned(),
            }
        } else {
            client.execute(
                "INSERT INTO storyos.proposal_generations
                 (owner_user_id,project_id,generation_id,proposal_id,generation_state,last_applied_stream_seq,run_id)
                 VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,'generating',0,$5::text::uuid)",
                &[&owner,&project,&generation_id,&target.proposal_id,&claim.run_id],
            ).await.map_err(database_error)?;
            client.execute(
                "INSERT INTO storyos.proposal_generation_heads (owner_user_id,project_id,proposal_id,generation_id)
                 VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid)
                 ON CONFLICT (owner_user_id,project_id,proposal_id) DO UPDATE SET generation_id=EXCLUDED.generation_id",
                &[&owner,&project,&target.proposal_id,&generation_id],
            ).await.map_err(database_error)?;
            let (revision_id, validation_id) = super::stream_proposal_generation::persist_batch(
                client,
                claim,
                &super::stream_proposal_generation::LoadedGeneration {
                    proposal_id: target.proposal_id.clone(),
                    generation_id: generation_id.clone(),
                    revision_id: target.revision_id.clone(),
                    candidate_text: loaded.text,
                    last_seq: 0,
                    generation_state: "generating".to_owned(),
                    existing_fence: false,
                    block_id: loaded.block_id,
                    base_revision_id: loaded.base_revision_id,
                },
                1,
                &candidate.candidate_text,
                /*complete*/ true,
                Some(&target.operation_id),
            )
            .await?;
            client.execute("UPDATE storyos.proposal_generations SET generation_state='ready',last_applied_stream_seq=1
                WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid AND generation_id=$3::text::uuid",
                &[&owner,&project,&generation_id]).await.map_err(database_error)?;
            result.proposal_id = Some(target.proposal_id.clone());
            ProseChangeLocationOutcome::Revised {
                proposal_id: target.proposal_id.clone(),
                operation_id: target.operation_id.clone(),
                revision_id,
                prior_revision_id: target.revision_id.clone(),
                validation_receipt_id: validation_id.expect("complete validation"),
            }
        }
    } else {
        ProseChangeLocationOutcome::Refused {
            reason: "stale_candidate".to_owned(),
        }
    };
    result.locations = Some(vec![ProseChangeLocationInspect {
        chapter_id: candidate.chapter_id.clone(),
        manuscript_block_id: candidate.manuscript_block_id.clone(),
        base_authoritative_revision_id: candidate.base_authoritative_revision_id.clone(),
        candidate_text: candidate.candidate_text.clone(),
        explanation: candidate.explanation.clone(),
        outcome,
        current: None,
    }]);
    Ok(result)
}
