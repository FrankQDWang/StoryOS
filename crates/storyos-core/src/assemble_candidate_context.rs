use super::{
    CONTEXT_ITEM_TOKEN_LIMIT, ContextBlockReason, ContextSourceClass, ContextSufficiency,
    CurrentPassageAssembly, CurrentPassageAssemblyRecord, assemble_current_passage_context,
};
use serde::{Deserialize, Serialize};

/// One exact pending candidate selected for a fresh producer instruction.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalCandidateTarget {
    pub proposal_id: String,
    pub operation_id: String,
    pub revision_id: String,
}

/// Assemble fresh candidate bytes with the required current Chapter under one target budget.
pub fn assemble_candidate_context(
    source: &CurrentPassageAssembly,
    target: &ProposalCandidateTarget,
    candidate_text: &str,
) -> CurrentPassageAssemblyRecord {
    let mut record = assemble_current_passage_context(source);
    let candidate = assemble_current_passage_context(&CurrentPassageAssembly {
        chapter_revision_id: Some(target.revision_id.clone()),
        chapter_body: candidate_text.to_owned(),
        ..source.clone()
    });
    let mut reasons = match &record.sufficiency {
        ContextSufficiency::Complete => Vec::new(),
        ContextSufficiency::Blocked { reasons } => reasons.clone(),
    };
    if let ContextSufficiency::Blocked { reasons: unmet } = candidate.sufficiency {
        reasons.extend(unmet);
    }
    record.considered.extend(
        candidate
            .considered
            .into_iter()
            .filter(|item| item.source_class == ContextSourceClass::WorkingTarget),
    );
    record.rejected.extend(
        candidate
            .rejected
            .into_iter()
            .filter(|item| item.source_class == ContextSourceClass::WorkingTarget),
    );
    record.selected.extend(
        candidate
            .selected
            .into_iter()
            .filter(|item| item.source_class == ContextSourceClass::WorkingTarget),
    );
    if record
        .selected
        .iter()
        .filter(|item| item.source_class == ContextSourceClass::WorkingTarget)
        .map(|item| item.token_count)
        .sum::<u64>()
        > CONTEXT_ITEM_TOKEN_LIMIT
    {
        record
            .selected
            .retain(|item| item.source_class != ContextSourceClass::WorkingTarget);
        reasons.push(ContextBlockReason::ExactRequiredOverLimit {
            source_class: ContextSourceClass::WorkingTarget,
        });
    }
    record.operation_requirement.candidate_target = Some(target.clone());
    record.sufficiency = if reasons.is_empty() {
        ContextSufficiency::Complete
    } else {
        ContextSufficiency::Blocked { reasons }
    };
    record
}
