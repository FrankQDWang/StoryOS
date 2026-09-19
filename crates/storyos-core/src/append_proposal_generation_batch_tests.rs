use super::{
    AppendProposalGenerationBatch, AppendProposalGenerationBatchConflict,
    AppendProposalGenerationBatchRefusal, AppendProposalGenerationBatchResult,
    append_proposal_generation_batch,
};

fn next_batch() -> AppendProposalGenerationBatch {
    AppendProposalGenerationBatch {
        scope_matches: true,
        generation_state: "generating".to_owned(),
        last_applied_stream_seq: 1,
        stream_seq: 2,
        expected_previous_stream_seq: 1,
        expected_proposal_revision_id: "rev-1".to_owned(),
        current_proposal_revision_id: "rev-1".to_owned(),
        expected_candidate_digest: "digest-1".to_owned(),
        current_candidate_digest: "digest-1".to_owned(),
        batch_digest: "digest-2".to_owned(),
        existing_batch_digest: None,
        reservation_owns_target: true,
    }
}

#[test]
fn applies_the_next_contiguous_canonical_batch() {
    assert_eq!(
        append_proposal_generation_batch(&next_batch()),
        AppendProposalGenerationBatchResult::Applied
    );
}

#[test]
fn returns_the_existing_outcome_for_an_exact_duplicate() {
    let mut duplicate = next_batch();
    duplicate.existing_batch_digest = Some("digest-2".to_owned());
    assert_eq!(
        append_proposal_generation_batch(&duplicate),
        AppendProposalGenerationBatchResult::Duplicate
    );
}

#[test]
fn waits_for_a_gap_and_refuses_a_late_or_closed_generation() {
    let mut gap = next_batch();
    gap.stream_seq = 3;
    gap.expected_previous_stream_seq = 1;
    assert_eq!(
        append_proposal_generation_batch(&gap),
        AppendProposalGenerationBatchResult::Wait
    );
    let mut fenced = next_batch();
    fenced.generation_state = "ready_partial".to_owned();
    assert_eq!(
        append_proposal_generation_batch(&fenced),
        AppendProposalGenerationBatchResult::Refused {
            reason: AppendProposalGenerationBatchRefusal::GenerationClosed,
        }
    );
    let mut stale = next_batch();
    stale.current_proposal_revision_id = "rev-9".to_owned();
    assert_eq!(
        append_proposal_generation_batch(&stale),
        AppendProposalGenerationBatchResult::Refused {
            reason: AppendProposalGenerationBatchRefusal::StaleHead,
        }
    );
}

#[test]
fn refuses_wrong_scope_and_conflicts_a_digest_mismatch() {
    let mut wrong_scope = next_batch();
    wrong_scope.scope_matches = false;
    assert_eq!(
        append_proposal_generation_batch(&wrong_scope),
        AppendProposalGenerationBatchResult::Refused {
            reason: AppendProposalGenerationBatchRefusal::WrongScope,
        }
    );
    let mut mismatch = next_batch();
    mismatch.existing_batch_digest = Some("other".to_owned());
    assert_eq!(
        append_proposal_generation_batch(&mismatch),
        AppendProposalGenerationBatchResult::Conflicted {
            reason: AppendProposalGenerationBatchConflict::DigestMismatch,
        }
    );
    let mut reserved = next_batch();
    reserved.reservation_owns_target = false;
    assert_eq!(
        append_proposal_generation_batch(&reserved),
        AppendProposalGenerationBatchResult::Refused {
            reason: AppendProposalGenerationBatchRefusal::ReservationMismatch,
        }
    );
}
