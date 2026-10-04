use crate::{
    OriginalResultKeepReason, OriginalResultRetrievalDecision, OriginalResultRetrievalFacts,
    RetainedResponseReference, RetrievalBounds, RetrievalCapability, RetrievedOriginalResult,
    decide_original_result_retrieval,
};

fn open_facts(retrieved: RetrievedOriginalResult) -> OriginalResultRetrievalFacts {
    OriginalResultRetrievalFacts {
        capability: RetrievalCapability::Supported,
        bounds: RetrievalBounds::DeclaredReadOnly,
        reference: RetainedResponseReference::Present,
        scope_permitted: true,
        conversation_permitted: true,
        destination_permitted: true,
        mapping_permitted: true,
        retrieved,
        run_fenced: false,
    }
}

#[test]
fn retrieval_follows_the_retained_reference_and_fence() {
    assert_eq!(
        decide_original_result_retrieval(&OriginalResultRetrievalFacts {
            reference: RetainedResponseReference::Absent,
            ..open_facts(RetrievedOriginalResult::CompleteSelected)
        }),
        OriginalResultRetrievalDecision::KeepUnknown {
            reason: OriginalResultKeepReason::MissingReference,
            admit_retrieval: false,
        }
    );
    assert_eq!(
        decide_original_result_retrieval(&OriginalResultRetrievalFacts {
            capability: RetrievalCapability::Unsupported,
            ..open_facts(RetrievedOriginalResult::CompleteSelected)
        }),
        OriginalResultRetrievalDecision::KeepUnknown {
            reason: OriginalResultKeepReason::UnsupportedRetrieval,
            admit_retrieval: false,
        }
    );
    assert_eq!(
        decide_original_result_retrieval(&open_facts(RetrievedOriginalResult::Incomplete)),
        OriginalResultRetrievalDecision::KeepUnknown {
            reason: OriginalResultKeepReason::IncompleteResult,
            admit_retrieval: true,
        }
    );
    assert_eq!(
        decide_original_result_retrieval(&open_facts(RetrievedOriginalResult::CompleteSelected)),
        OriginalResultRetrievalDecision::SettleSelected
    );
    assert_eq!(
        decide_original_result_retrieval(&OriginalResultRetrievalFacts {
            run_fenced: true,
            ..open_facts(RetrievedOriginalResult::CompleteSelected)
        }),
        OriginalResultRetrievalDecision::EvidenceOnly
    );
}
