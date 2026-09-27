use super::{
    ContinuationReferenceCondition, ExpiryRebuildBlock, ExpiryRebuildDecision,
    ExpiryRebuildDisposition, ExpiryRebuildFacts, decide_confirmed_expiry_rebuild,
};

fn ready(reference: ContinuationReferenceCondition) -> ExpiryRebuildFacts {
    ExpiryRebuildFacts {
        reference,
        prior_submissions_settled: true,
        predecessor_fenced: false,
        same_processing_boundary: true,
        current_authority: true,
        budget_covers_submission: true,
        required_input_present: true,
        covered_copy_restricted: false,
        ordinary_correction: true,
        effective_model_context_changed: true,
        predecessor_terminal: true,
    }
}

fn decide(facts: ExpiryRebuildFacts) -> ExpiryRebuildDecision {
    decide_confirmed_expiry_rebuild(&facts)
}

#[test]
fn usable_reference_does_not_rebuild_or_erase_an_ordinary_correction() {
    let decision = decide(ready(ContinuationReferenceCondition::Usable));
    assert_eq!(
        decision,
        ExpiryRebuildDecision {
            disposition: ExpiryRebuildDisposition::NotApplicable,
            new_run_step_and_invocation: false,
            lossless_provider_reconstruction: false,
            semantic_erasure: false,
            opaque_reused: false,
            covered_content_included: false,
            predecessor_stays_terminal: true,
        }
    );
}

#[test]
fn unknown_create_is_not_confirmed_expiry() {
    let decision = decide(ready(ContinuationReferenceCondition::UnknownCreate));
    assert_eq!(
        decision.disposition,
        ExpiryRebuildDisposition::UnknownCreate
    );
    assert!(!decision.opaque_reused);
    assert!(!decision.lossless_provider_reconstruction);
}

#[test]
fn confirmed_expiry_and_unusable_handles_rebuild_without_opaque_reuse() {
    for reference in [
        ContinuationReferenceCondition::ConfirmedExpired,
        ContinuationReferenceCondition::ConfirmedUnusable,
    ] {
        let decision = decide(ready(reference));
        assert_eq!(decision.disposition, ExpiryRebuildDisposition::Rebuilt);
        assert!(decision.new_run_step_and_invocation);
        assert!(decision.predecessor_stays_terminal);
        assert!(!decision.lossless_provider_reconstruction);
        assert!(!decision.semantic_erasure);
        assert!(!decision.opaque_reused);
        assert!(decision.covered_content_included);
    }
}

#[test]
fn automatic_rebuild_blocks_when_a_required_gate_fails() {
    let cases = [
        (
            ExpiryRebuildFacts {
                prior_submissions_settled: false,
                ..ready(ContinuationReferenceCondition::ConfirmedExpired)
            },
            ExpiryRebuildBlock::PriorSubmissionUnsettled,
        ),
        (
            ExpiryRebuildFacts {
                predecessor_fenced: true,
                ..ready(ContinuationReferenceCondition::ConfirmedExpired)
            },
            ExpiryRebuildBlock::FencedPredecessor,
        ),
        (
            ExpiryRebuildFacts {
                same_processing_boundary: false,
                ..ready(ContinuationReferenceCondition::ConfirmedUnusable)
            },
            ExpiryRebuildBlock::ProcessingBoundaryChanged,
        ),
        (
            ExpiryRebuildFacts {
                current_authority: false,
                ..ready(ContinuationReferenceCondition::ConfirmedExpired)
            },
            ExpiryRebuildBlock::AuthorityMissing,
        ),
        (
            ExpiryRebuildFacts {
                budget_covers_submission: false,
                ..ready(ContinuationReferenceCondition::ConfirmedExpired)
            },
            ExpiryRebuildBlock::BudgetInsufficient,
        ),
        (
            ExpiryRebuildFacts {
                required_input_present: false,
                ..ready(ContinuationReferenceCondition::ConfirmedExpired)
            },
            ExpiryRebuildBlock::RequiredInputMissing,
        ),
    ];
    for (facts, reason) in cases {
        let decision = decide(facts);
        assert_eq!(
            decision.disposition,
            ExpiryRebuildDisposition::Blocked(reason)
        );
        assert!(!decision.new_run_step_and_invocation);
        assert!(!decision.opaque_reused);
        assert!(!decision.covered_content_included);
        assert!(!decision.lossless_provider_reconstruction);
    }
}

#[test]
fn copy_restriction_omits_covered_content_and_still_rebuilds_eligible_input() {
    let decision = decide(ExpiryRebuildFacts {
        covered_copy_restricted: true,
        ..ready(ContinuationReferenceCondition::ConfirmedExpired)
    });
    assert_eq!(decision.disposition, ExpiryRebuildDisposition::Rebuilt);
    assert!(!decision.opaque_reused);
    assert!(!decision.covered_content_included);
    assert!(!decision.semantic_erasure);
}

#[test]
fn unchanged_effective_context_does_not_mint_a_new_step() {
    let decision = decide(ExpiryRebuildFacts {
        effective_model_context_changed: false,
        ..ready(ContinuationReferenceCondition::ConfirmedExpired)
    });
    assert_eq!(decision.disposition, ExpiryRebuildDisposition::Rebuilt);
    assert!(!decision.new_run_step_and_invocation);
}
