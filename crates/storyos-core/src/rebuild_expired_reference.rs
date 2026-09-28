//! Decide whether a confirmed expired reference may rebuild eligible context.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContinuationReferenceCondition {
    Usable,
    ConfirmedExpired,
    ConfirmedUnusable,
    UnknownCreate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpiryRebuildBlock {
    PriorSubmissionUnsettled,
    FencedPredecessor,
    ProcessingBoundaryChanged,
    AuthorityMissing,
    BudgetInsufficient,
    RequiredInputMissing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpiryRebuildDisposition {
    NotApplicable,
    UnknownCreate,
    Blocked(ExpiryRebuildBlock),
    Rebuilt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpiryRebuildFacts {
    pub reference: ContinuationReferenceCondition,
    pub prior_submissions_settled: bool,
    pub predecessor_fenced: bool,
    pub same_processing_boundary: bool,
    pub current_authority: bool,
    pub budget_covers_submission: bool,
    pub required_input_present: bool,
    pub covered_copy_restricted: bool,
    pub ordinary_correction: bool,
    pub effective_model_context_changed: bool,
    pub predecessor_terminal: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpiryRebuildDecision {
    pub disposition: ExpiryRebuildDisposition,
    pub new_run_step_and_invocation: bool,
    pub lossless_provider_reconstruction: bool,
    pub semantic_erasure: bool,
    pub opaque_reused: bool,
    pub covered_content_included: bool,
    pub predecessor_stays_terminal: bool,
}

/// Admit one fresh assembly, or block, without treating unknown create as expiry.
pub fn decide_confirmed_expiry_rebuild(facts: &ExpiryRebuildFacts) -> ExpiryRebuildDecision {
    let idle = ExpiryRebuildDecision {
        disposition: ExpiryRebuildDisposition::NotApplicable,
        new_run_step_and_invocation: false,
        lossless_provider_reconstruction: false,
        semantic_erasure: match facts.ordinary_correction {
            true | false => false,
        },
        opaque_reused: false,
        covered_content_included: false,
        predecessor_stays_terminal: facts.predecessor_terminal,
    };
    match facts.reference {
        ContinuationReferenceCondition::Usable => idle,
        ContinuationReferenceCondition::UnknownCreate => ExpiryRebuildDecision {
            disposition: ExpiryRebuildDisposition::UnknownCreate,
            ..idle
        },
        ContinuationReferenceCondition::ConfirmedExpired
        | ContinuationReferenceCondition::ConfirmedUnusable => {
            let reason = if !facts.prior_submissions_settled {
                Some(ExpiryRebuildBlock::PriorSubmissionUnsettled)
            } else if facts.predecessor_fenced {
                Some(ExpiryRebuildBlock::FencedPredecessor)
            } else if !facts.same_processing_boundary {
                Some(ExpiryRebuildBlock::ProcessingBoundaryChanged)
            } else if !facts.current_authority {
                Some(ExpiryRebuildBlock::AuthorityMissing)
            } else if !facts.budget_covers_submission {
                Some(ExpiryRebuildBlock::BudgetInsufficient)
            } else if !facts.required_input_present {
                Some(ExpiryRebuildBlock::RequiredInputMissing)
            } else {
                None
            };
            match reason {
                Some(reason) => ExpiryRebuildDecision {
                    disposition: ExpiryRebuildDisposition::Blocked(reason),
                    ..idle
                },
                None => ExpiryRebuildDecision {
                    disposition: ExpiryRebuildDisposition::Rebuilt,
                    new_run_step_and_invocation: facts.effective_model_context_changed,
                    covered_content_included: !facts.covered_copy_restricted,
                    ..idle
                },
            }
        }
    }
}

#[cfg(test)]
#[path = "rebuild_expired_reference_tests.rs"]
mod tests;
