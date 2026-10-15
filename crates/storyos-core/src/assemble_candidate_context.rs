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
    add_working_texts(
        &mut record,
        source,
        &[(target.revision_id.clone(), candidate_text.to_owned())],
    );
    record.operation_requirement.candidate_target = Some(target.clone());
    record
}

/// Add each `(revision, text)` as a Working Target item. All Working Target items share one
/// budget, and the assembly blocks when they pass it.
pub(crate) fn add_working_texts(
    record: &mut CurrentPassageAssemblyRecord,
    source: &CurrentPassageAssembly,
    texts: &[(String, String)],
) {
    for (revision_id, text) in texts {
        let item = assemble_current_passage_context(&CurrentPassageAssembly {
            chapter_revision_id: Some(revision_id.clone()),
            chapter_body: text.clone(),
            ..source.clone()
        });
        let target = |class: &ContextSourceClass| *class == ContextSourceClass::WorkingTarget;
        record.considered.extend(
            item.considered
                .into_iter()
                .filter(|item| target(&item.source_class)),
        );
        record.rejected.extend(
            item.rejected
                .into_iter()
                .filter(|item| target(&item.source_class)),
        );
        record.selected.extend(
            item.selected
                .into_iter()
                .filter(|item| target(&item.source_class)),
        );
    }
    if record
        .considered
        .iter()
        .filter(|item| item.source_class == ContextSourceClass::WorkingTarget)
        .map(|item| item.token_count)
        .sum::<u64>()
        > CONTEXT_ITEM_TOKEN_LIMIT
    {
        record
            .selected
            .retain(|item| item.source_class != ContextSourceClass::WorkingTarget);
        block(
            record,
            ContextBlockReason::ExactRequiredOverLimit {
                source_class: ContextSourceClass::WorkingTarget,
            },
        );
    }
}

/// Add a reason to the sufficiency of the record.
pub(crate) fn block(record: &mut CurrentPassageAssemblyRecord, reason: ContextBlockReason) {
    let mut reasons = match std::mem::replace(&mut record.sufficiency, ContextSufficiency::Complete)
    {
        ContextSufficiency::Complete => Vec::new(),
        ContextSufficiency::Blocked { reasons } => reasons,
    };
    reasons.push(reason);
    record.sufficiency = ContextSufficiency::Blocked { reasons };
}
