use super::{
    CompleteReadyPartialProposal, CompleteReadyPartialProposalRefusal, ContinueProposalGeneration,
    ContinueProposalGenerationRefusal, ProposalGenerationConflict, TransitionOutcome,
    complete_ready_partial_proposal, continue_proposal_generation,
};

fn completable() -> CompleteReadyPartialProposal {
    CompleteReadyPartialProposal {
        revision_current: true,
        closure_open: true,
        generation_state: "ready_partial".to_owned(),
        generation_id_matches: true,
        candidate_digest_matches: true,
        stream_seq_matches: true,
        expected_target_matches_head: true,
    }
}

fn continuable() -> ContinueProposalGeneration {
    ContinueProposalGeneration {
        revision_current: true,
        closure_open: true,
        generation_state: "ready".to_owned(),
        expected_generation_state: "ready".to_owned(),
        generation_id_matches: true,
        candidate_digest_matches: true,
        selected_operations_pending: true,
        selection_duplicate_free: true,
        expected_target_matches_head: true,
    }
}

#[test]
fn a_current_ready_partial_candidate_completes() {
    assert_eq!(
        complete_ready_partial_proposal(&completable()),
        TransitionOutcome::Applied(())
    );
}

#[test]
fn completion_conflicts_when_the_chapter_head_changed() {
    let changed = CompleteReadyPartialProposal {
        expected_target_matches_head: false,
        ..completable()
    };
    assert_eq!(
        complete_ready_partial_proposal(&changed),
        TransitionOutcome::Conflicted(ProposalGenerationConflict::ChangedHead)
    );
}

#[test]
fn completion_refuses_a_stale_closed_or_changed_candidate_before_the_head_check() {
    let refusals = [
        CompleteReadyPartialProposal {
            revision_current: false,
            closure_open: false,
            ..completable()
        },
        CompleteReadyPartialProposal {
            closure_open: false,
            generation_state: "ready".to_owned(),
            ..completable()
        },
        CompleteReadyPartialProposal {
            generation_state: "ready".to_owned(),
            generation_id_matches: false,
            ..completable()
        },
        CompleteReadyPartialProposal {
            generation_id_matches: false,
            stream_seq_matches: false,
            ..completable()
        },
        CompleteReadyPartialProposal {
            stream_seq_matches: false,
            expected_target_matches_head: false,
            ..completable()
        },
    ]
    .map(|command| complete_ready_partial_proposal(&command));
    assert_eq!(
        refusals,
        [
            CompleteReadyPartialProposalRefusal::StaleProposalRevision,
            CompleteReadyPartialProposalRefusal::NotEligible,
            CompleteReadyPartialProposalRefusal::NotReadyPartial,
            CompleteReadyPartialProposalRefusal::StaleGeneration,
            CompleteReadyPartialProposalRefusal::StaleCandidate,
        ]
        .map(TransitionOutcome::Refused)
    );
}

#[test]
fn a_current_ready_candidate_starts_a_new_generation() {
    assert_eq!(
        continue_proposal_generation(&continuable()),
        TransitionOutcome::Applied(())
    );
}

#[test]
fn continuation_conflicts_when_the_chapter_head_changed() {
    let changed = ContinueProposalGeneration {
        expected_target_matches_head: false,
        ..continuable()
    };
    assert_eq!(
        continue_proposal_generation(&changed),
        TransitionOutcome::Conflicted(ProposalGenerationConflict::ChangedHead)
    );
}

#[test]
fn continuation_refuses_a_stale_closed_or_invalid_selection_before_the_head_check() {
    let refusals = [
        ContinueProposalGeneration {
            revision_current: false,
            closure_open: false,
            ..continuable()
        },
        ContinueProposalGeneration {
            closure_open: false,
            expected_generation_state: "ready_partial".to_owned(),
            ..continuable()
        },
        ContinueProposalGeneration {
            generation_state: "generating".to_owned(),
            expected_generation_state: "generating".to_owned(),
            generation_id_matches: false,
            ..continuable()
        },
        ContinueProposalGeneration {
            generation_id_matches: false,
            candidate_digest_matches: false,
            ..continuable()
        },
        ContinueProposalGeneration {
            candidate_digest_matches: false,
            selection_duplicate_free: false,
            ..continuable()
        },
        ContinueProposalGeneration {
            selection_duplicate_free: false,
            selected_operations_pending: false,
            ..continuable()
        },
        ContinueProposalGeneration {
            selected_operations_pending: false,
            expected_target_matches_head: false,
            ..continuable()
        },
    ]
    .map(|command| continue_proposal_generation(&command));
    assert_eq!(
        refusals,
        [
            ContinueProposalGenerationRefusal::StaleProposalRevision,
            ContinueProposalGenerationRefusal::NotEligible,
            ContinueProposalGenerationRefusal::NotContinuable,
            ContinueProposalGenerationRefusal::StaleGeneration,
            ContinueProposalGenerationRefusal::StaleCandidate,
            ContinueProposalGenerationRefusal::DuplicateIdentities,
            ContinueProposalGenerationRefusal::OperationNotPending,
        ]
        .map(TransitionOutcome::Refused)
    );
}
