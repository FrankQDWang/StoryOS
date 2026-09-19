use super::{
    PauseProposalGeneration, PauseProposalGenerationRefusal, PauseProposalGenerationResult,
    pause_proposal_generation,
};

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

#[test]
fn refuses_wrong_scope_stale_heads_and_non_generating_state() {
    let mut wrong_scope = current_head();
    wrong_scope.scope_matches = false;
    assert_eq!(
        pause_proposal_generation(&wrong_scope),
        PauseProposalGenerationResult::Refused {
            reason: PauseProposalGenerationRefusal::WrongScope,
        }
    );
    let mut stale = current_head();
    stale.current_proposal_revision_id = "rev-9".to_owned();
    assert_eq!(
        pause_proposal_generation(&stale),
        PauseProposalGenerationResult::Refused {
            reason: PauseProposalGenerationRefusal::StaleHead,
        }
    );
    let mut ready = current_head();
    ready.generation_state = "ready".to_owned();
    assert_eq!(
        pause_proposal_generation(&ready),
        PauseProposalGenerationResult::Refused {
            reason: PauseProposalGenerationRefusal::NotGenerating,
        }
    );
}
