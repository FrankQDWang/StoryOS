use super::{
    CONTEXT_ITEM_TOKEN_LIMIT, ConsideredSource, ContextBlockReason, ContextSourceClass,
    ContextSufficiency, CurrentPassageAssembly, CurrentPassageAssemblyRecord, RejectedSource,
    RejectionReason, assemble_current_passage_context, count_context_item_tokens,
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
    let target_list_bound = !targets.is_empty() && targets.len() == passages.len();
    if !target_list_bound {
        reasons.push(ContextBlockReason::WorkingTargetRevisionUnavailable);
    }
    let total: u64 = passages
        .iter()
        .map(|input| count_context_item_tokens(&input.chapter_body))
        .sum();
    for passage in passages.iter().skip(1) {
        let assembled = assemble_current_passage_context(passage);
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
                .into_iter()
                .filter(|item| item.source_class == ContextSourceClass::WorkingTarget),
        );
        if let ContextSufficiency::Blocked { reasons: unmet } = assembled.sufficiency {
            reasons.extend(unmet);
        }
    }
    if total > CONTEXT_ITEM_TOKEN_LIMIT {
        let (over, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut record.selected)
            .into_iter()
            .partition(|item| item.source_class == ContextSourceClass::WorkingTarget);
        record.selected = kept;
        record
            .rejected
            .extend(over.into_iter().map(|item| RejectedSource {
                source_class: item.source_class,
                source_version: item.source_version,
                token_count: item.token_count,
                reason: RejectionReason::OverCollectionTokenLimit,
            }));
        for item in &mut record.considered {
            if item.source_class == ContextSourceClass::WorkingTarget {
                item.eligible = false;
            }
        }
        reasons.push(ContextBlockReason::ExactRequiredOverLimit {
            source_class: ContextSourceClass::WorkingTarget,
        });
    }
    if !target_list_bound {
        // The record of an unbound target list must not show the source Chapter as the Working Target.
        let is_other = |class: ContextSourceClass| class != ContextSourceClass::WorkingTarget;
        record.considered.retain(|item| is_other(item.source_class));
        record.selected.retain(|item| is_other(item.source_class));
        record.rejected.retain(|item| is_other(item.source_class));
        record.considered.push(ConsideredSource {
            source_class: ContextSourceClass::WorkingTarget,
            source_version: "unavailable".to_owned(),
            token_count: 0,
            eligible: false,
        });
        record.rejected.push(RejectedSource {
            source_class: ContextSourceClass::WorkingTarget,
            source_version: "unavailable".to_owned(),
            token_count: 0,
            reason: RejectionReason::WorkingTargetRevisionUnavailable,
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

#[cfg(test)]
#[path = "assemble_passage_collection_tests.rs"]
mod tests;
