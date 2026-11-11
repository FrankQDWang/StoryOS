//! The Context Candidates of the source class `tool_result` for the continuation RunStep of a
//! durable function request Decision (ADR 0005, ADR 0084). Each resolved ToolCall of the Run is
//! one candidate; its source version is its Source Snapshot ids, or its ToolCall identity when it
//! captured no Snapshot. Its text ends with the citable identities of its Snapshots and Gaps
//! (ADR 0087). It crosses identity, Project
//! Scope, lifecycle, trust, and Bounded Projection here; the grant scope of the pinned grant
//! decides its disclosure eligibility at Create admission (ADR 0051).

use serde::{Deserialize, Serialize};

use crate::assemble_context::count_context_item_tokens;
use crate::assemble_context::{ConsideredSource, ContextCause, RejectedSource, SelectedProjection};
use crate::{
    ContextBlockReason, ContextSourceClass, ContextSufficiency, CurrentPassageAssemblyRecord,
    ProjectionMode, RejectionReason, ToolCallState, ToolResultProjection,
};

/// The trust of each `tool_result` source: data with no Instruction Authority.
pub const TOOL_RESULT_TRUST: &str = "data_only";

/// One selected `tool_result` source of a continuation, as the Operation Requirement records it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolResultSource {
    pub tool_call_id: String,
    /// The native call identifier that the output item correlates with.
    pub call_id: String,
    pub model_attempt_id: String,
    pub decision_id: String,
    /// The projected state: `ok`, `empty`, `unavailable`, or `rejected`.
    pub state: String,
    pub trust: String,
    /// The token count of the result text that the projection keeps, when the bound cut it.
    pub kept_result_tokens: Option<u64>,
    /// The Source Snapshots of the result, in result order. A ToolCall with no Snapshot, or one
    /// that a record before S5-06 names, has none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_snapshot_ids: Vec<String>,
}

impl ToolResultSource {
    /// The Context Source Version of the selected item of this source.
    pub fn source_version(&self) -> String {
        crate::tool_result_source_version(&self.tool_call_id, &self.source_snapshot_ids)
    }
}

/// One stored ToolCall of the Run, as the store reads it for the continuation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToolResultCandidate<'a> {
    pub tool_call_id: &'a str,
    pub call_id: &'a str,
    pub owner_user_id: &'a str,
    pub project_id: &'a str,
    pub run_id: &'a str,
    pub model_attempt_id: &'a str,
    pub decision_id: &'a str,
    pub state: ToolCallState,
    pub result_text: Option<&'a str>,
    /// The Source Snapshots, in result order, and then the Source Gaps, in request order, that
    /// the settlement of the ToolCall captured.
    pub sources: &'a [crate::CitableSource],
}

impl ToolResultCandidate<'_> {
    fn source_snapshot_ids(&self) -> Vec<String> {
        self.sources
            .iter()
            .filter(|source| source.cited == crate::CitedSource::Snapshot)
            .map(|source| source.id.clone())
            .collect()
    }
}

/// Makes `record` the Context Assembly of a `tool_result_continuation`: each candidate, in item
/// order, is selected with its bounded projection, or rejected. A rejected candidate blocks the
/// continuation, because each native function call needs its output.
pub fn assemble_tool_result_continuation(
    record: &mut CurrentPassageAssemblyRecord,
    candidates: &[ToolResultCandidate<'_>],
) {
    let requirement = &mut record.operation_requirement;
    requirement.cause = ContextCause::ToolResultContinuation;
    let mut sources = Vec::new();
    let mut blocked = false;
    for candidate in candidates {
        let snapshot_ids = candidate.source_snapshot_ids();
        let source_version =
            crate::tool_result_source_version(candidate.tool_call_id, &snapshot_ids);
        let same_run = candidate.owner_user_id == requirement.owner_user_id
            && candidate.project_id == requirement.project_id
            && candidate.run_id == requirement.run_id;
        let projection = same_run
            .then(|| ToolResultProjection::of(candidate.state, candidate.result_text))
            .flatten();
        let Some(projection) = projection else {
            blocked = true;
            record.considered.push(ConsideredSource {
                source_class: ContextSourceClass::ToolResult,
                source_version: source_version.clone(),
                token_count: 0,
                eligible: false,
            });
            record.rejected.push(RejectedSource {
                source_class: ContextSourceClass::ToolResult,
                source_version: source_version.clone(),
                token_count: 0,
                reason: RejectionReason::ToolResultUnavailable,
            });
            continue;
        };
        // The bound never cuts an identity: it applies to the result text only.
        let tag = crate::citable_sources_tag(candidate.call_id, candidate.sources);
        let tag_tokens = count_context_item_tokens(&tag);
        let bounded = projection.bounded_text(
            candidate.call_id,
            requirement.item_token_limit.saturating_sub(tag_tokens),
        );
        record.considered.push(ConsideredSource {
            source_class: ContextSourceClass::ToolResult,
            source_version: source_version.clone(),
            token_count: bounded.source_tokens + tag_tokens,
            eligible: true,
        });
        let content = format!("{}{tag}", bounded.text);
        record.selected.push(SelectedProjection {
            source_class: ContextSourceClass::ToolResult,
            source_version: source_version.clone(),
            projection_mode: match bounded.kept_result_tokens {
                Some(_) => ProjectionMode::DeterministicExcerpt,
                None => ProjectionMode::ExactRequired,
            },
            token_count: count_context_item_tokens(&content),
            content,
        });
        sources.push(ToolResultSource {
            tool_call_id: candidate.tool_call_id.to_owned(),
            call_id: candidate.call_id.to_owned(),
            model_attempt_id: candidate.model_attempt_id.to_owned(),
            decision_id: candidate.decision_id.to_owned(),
            state: projection.label().to_owned(),
            trust: TOOL_RESULT_TRUST.to_owned(),
            kept_result_tokens: bounded.kept_result_tokens,
            source_snapshot_ids: snapshot_ids,
        });
    }
    requirement.tool_results = Some(sources);
    if blocked {
        let reason = ContextBlockReason::ToolResultUnavailable;
        record.sufficiency =
            match std::mem::replace(&mut record.sufficiency, ContextSufficiency::Complete) {
                ContextSufficiency::Complete => ContextSufficiency::Blocked {
                    reasons: vec![reason],
                },
                ContextSufficiency::Blocked { mut reasons } => {
                    reasons.push(reason);
                    ContextSufficiency::Blocked { reasons }
                }
            };
    }
}

#[cfg(test)]
#[path = "tool_result_context_tests.rs"]
mod tests;
