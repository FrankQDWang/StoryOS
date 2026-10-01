use super::{
    CONTEXT_ITEM_TOKEN_LIMIT, ContextBlockReason, ContextSourceClass, ContextSufficiency,
    CurrentPassageAssembly, CurrentPassageAssemblyRecord, assemble_current_passage_context,
    count_context_item_tokens,
};
use serde::{Deserialize, Serialize};

/// One exact Chapter/base/Block binding in the admitted Working Target.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PassageContextTarget {
    pub chapter_id: String,
    pub base_authoritative_revision_id: String,
    pub manuscript_block_ids: Vec<String>,
}

/// Assemble the complete collection under the existing Working Target budget.
pub fn assemble_passage_collection(
    source: &CurrentPassageAssembly,
    targets: Vec<PassageContextTarget>,
    passages: &[CurrentPassageAssembly],
) -> CurrentPassageAssemblyRecord {
    let mut record = assemble_current_passage_context(passages.first().unwrap_or(source));
    let mut reasons = match &record.sufficiency {
        ContextSufficiency::Complete => Vec::new(),
        ContextSufficiency::Blocked { reasons } => reasons.clone(),
    };
    if targets.is_empty() || targets.len() != passages.len() {
        reasons.push(ContextBlockReason::WorkingTargetRevisionUnavailable);
    }
    let total: u64 = passages
        .iter()
        .map(|input| count_context_item_tokens(&input.chapter_body))
        .sum();
    for passage in passages.iter().skip(1) {
        let mut assembled = assemble_current_passage_context(passage);
        record.considered.extend(
            assembled
                .considered
                .into_iter()
                .filter(|item| item.source_class == ContextSourceClass::WorkingTarget),
        );
        record.rejected.extend(
            assembled
                .rejected
                .into_iter()
                .filter(|item| item.source_class == ContextSourceClass::WorkingTarget),
        );
        record.selected.extend(
            assembled
                .selected
                .drain(..)
                .filter(|item| item.source_class == ContextSourceClass::WorkingTarget),
        );
        if let ContextSufficiency::Blocked { reasons: unmet } = assembled.sufficiency {
            reasons.extend(unmet);
        }
    }
    if total > CONTEXT_ITEM_TOKEN_LIMIT {
        record
            .selected
            .retain(|item| item.source_class != ContextSourceClass::WorkingTarget);
        reasons.push(ContextBlockReason::ExactRequiredOverLimit {
            source_class: ContextSourceClass::WorkingTarget,
        });
    }
    record.operation_requirement.chapter_id = source.chapter_id.clone();
    record.operation_requirement.chapter_revision_id = source.chapter_revision_id.clone();
    record.operation_requirement.proposal_target_block_ids =
        source.proposal_target_block_ids.clone();
    record.operation_requirement.passage_targets = Some(targets);
    record.sufficiency = if reasons.is_empty() {
        ContextSufficiency::Complete
    } else {
        ContextSufficiency::Blocked { reasons }
    };
    record
}
