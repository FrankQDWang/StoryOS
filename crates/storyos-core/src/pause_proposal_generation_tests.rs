use super::{PauseProposalGeneration, PauseProposalGenerationResult, pause_proposal_generation};

fn current_head() -> PauseProposalGeneration {
    PauseProposalGeneration {
        scope_matches: true,
        generation_state: "generating".to_owned(),
        expected_proposal_revision_id: "rev-1".to_owned(),
        current_proposal_revision_id: "rev-1".to_owned(),
        expected_candidate_digest: "digest-1".to_owned(),
        current_candidate_digest: "digest-1".to_owned(),
        last_applied_stream_seq: 1,
        admitted_through_seq: 1,
        existing_fence: false,
    }
}

#[test]
fn pauses_the_admitted_head_without_an_author_action() {
    assert_eq!(
        pause_proposal_generation(&current_head()),
        PauseProposalGenerationResult::Applied {
            allocates_author_action: false,
        }
    );
}

#[test]
fn retries_the_same_fence_without_fabricating_an_author_action() {
    let mut retry = current_head();
    retry.existing_fence = true;
    retry.generation_state = "ready_partial".to_owned();
    assert_eq!(
        pause_proposal_generation(&retry),
        PauseProposalGenerationResult::Duplicate {
            allocates_author_action: false,
        }
    );
}
