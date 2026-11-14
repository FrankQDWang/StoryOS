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
    pub output: ToolResultOutput,
    pub function_name: &'a str,
    /// The canonical arguments of the call.
    pub arguments: &'a str,
    pub form: ToolResultForm,
}

/// How the continuation input gives the result of one ToolCall.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolResultOutput {
    /// The bounded projection of the result of a research Tool with its source tag and its
    /// citable sources.
    Research,
    /// The bounded projection of the result of a Host Tool. `read_tool_result` cannot read it, so
    /// it keeps its exact form.
    HostTool,
    /// The settled result of a `read_tool_result` call as it is: it already is the bounded form
    /// of the call that it read (ADR 0093). A refusal of the read is projected.
    StoredRead,
}

/// The form in which the request sends one result (ADR 0093 section 5).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolResultForm {
    /// The bounded exact form.
    Exact,
    /// A stub when the result is a research result or a read, and its exact form is larger than
    /// the stub limit. Else the exact form.
    Stub,
}

/// The text of one `tool_result` item and its token counts.
pub(crate) struct ToolResultItem {
    pub content: String,
    pub token_count: u64,
    /// The token count of the complete projection and its tag, before the bound.
    pub source_tokens: u64,
    pub kept_result_tokens: Option<u64>,
}

/// The bounded projection of a result for the ToolCall `call_id`, followed by the tag of its
/// citable sources. The bound never cuts an identity: it applies to the result text only.
pub(crate) fn project_item(
    call_id: &str,
    projection: ToolResultProjection<'_>,
    sources: &[crate::CitableSource],
    item_token_limit: u64,
) -> ToolResultItem {
    let tag = crate::citable_sources_tag(call_id, sources);
    let tag_tokens = count_context_item_tokens(&tag);
    let bounded = projection.bounded_text(call_id, item_token_limit.saturating_sub(tag_tokens));
    let content = format!("{}{tag}", bounded.text);
    ToolResultItem {
        token_count: count_context_item_tokens(&content),
        source_tokens: bounded.source_tokens + tag_tokens,
        kept_result_tokens: bounded.kept_result_tokens,
        content,
    }
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
/// continuation, because each native function call needs its output. A candidate whose citable
/// tag and minimum boundary are above the item limit is rejected (ADR 0087).
pub fn assemble_tool_result_continuation(
    record: &mut CurrentPassageAssemblyRecord,
    candidates: &[ToolResultCandidate<'_>],
) {
    let requirement = &mut record.operation_requirement;
    requirement.cause = ContextCause::ToolResultContinuation;
    let mut sources = Vec::new();
    let mut blocks = Vec::new();
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
            if !blocks.contains(&ContextBlockReason::ToolResultUnavailable) {
                blocks.push(ContextBlockReason::ToolResultUnavailable);
            }
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
        let (item, projection_mode) =
            candidate_item(candidate, projection, requirement.item_token_limit);
        let ToolResultItem {
            content,
            token_count,
            source_tokens,
            kept_result_tokens,
        } = item;
        // A tag whose identities and the minimum boundary do not fit together in the item limit
        // blocks the continuation, because a cut identity cannot be cited.
        let fits = token_count <= requirement.item_token_limit;
        record.considered.push(ConsideredSource {
            source_class: ContextSourceClass::ToolResult,
            source_version: source_version.clone(),
            token_count: source_tokens,
            eligible: fits,
        });
        if !fits {
            let reason = ContextBlockReason::ExactRequiredOverLimit {
                source_class: ContextSourceClass::ToolResult,
            };
            if !blocks.contains(&reason) {
                blocks.push(reason);
            }
            record.rejected.push(RejectedSource {
                source_class: ContextSourceClass::ToolResult,
                source_version: source_version.clone(),
                token_count,
                reason: RejectionReason::OverItemTokenLimit,
            });
            continue;
        }
        record.selected.push(SelectedProjection {
            source_class: ContextSourceClass::ToolResult,
            source_version: source_version.clone(),
            projection_mode,
            token_count,
            content,
        });
        sources.push(ToolResultSource {
            tool_call_id: candidate.tool_call_id.to_owned(),
            call_id: candidate.call_id.to_owned(),
            model_attempt_id: candidate.model_attempt_id.to_owned(),
            decision_id: candidate.decision_id.to_owned(),
            state: projection.label().to_owned(),
            trust: TOOL_RESULT_TRUST.to_owned(),
            kept_result_tokens,
            source_snapshot_ids: snapshot_ids,
        });
    }
    requirement.tool_results = Some(sources);
    if !blocks.is_empty() {
        record.sufficiency =
            match std::mem::replace(&mut record.sufficiency, ContextSufficiency::Complete) {
                ContextSufficiency::Complete => ContextSufficiency::Blocked { reasons: blocks },
                ContextSufficiency::Blocked { mut reasons } => {
                    reasons.extend(blocks);
                    ContextSufficiency::Blocked { reasons }
                }
            };
    }
}

/// The item of `candidate` and its projection mode. A candidate in the stub form gives its stub
/// when its exact item is larger than the stub limit (ADR 0093 section 5).
fn candidate_item(
    candidate: &ToolResultCandidate<'_>,
    projection: ToolResultProjection<'_>,
    item_token_limit: u64,
) -> (ToolResultItem, ProjectionMode) {
    let exact = match (candidate.output, projection) {
        // The result of a read is the stored bounded form of the call that it read.
        (ToolResultOutput::StoredRead, ToolResultProjection::Ok(text)) => ToolResultItem {
            content: text.to_owned(),
            token_count: count_context_item_tokens(text),
            source_tokens: count_context_item_tokens(text),
            kept_result_tokens: None,
        },
        (
            ToolResultOutput::StoredRead | ToolResultOutput::Research | ToolResultOutput::HostTool,
            _,
        ) => project_item(
            candidate.call_id,
            projection,
            candidate.sources,
            item_token_limit,
        ),
    };
    let exact_mode = match exact.kept_result_tokens {
        Some(_) => ProjectionMode::DeterministicExcerpt,
        None => ProjectionMode::ExactRequired,
    };
    let read_call_id = match candidate.output {
        ToolResultOutput::StoredRead => {
            let arguments: Option<serde_json::Value> =
                serde_json::from_str(candidate.arguments).ok();
            match arguments
                .as_ref()
                .and_then(|value| value.get("call_id")?.as_str())
            {
                Some(target) => Some(Some(target.to_owned())),
                None => return (exact, exact_mode),
            }
        }
        ToolResultOutput::Research => Some(None),
        ToolResultOutput::HostTool => None,
    };
    let stub_applies = candidate.form == ToolResultForm::Stub
        && matches!(projection, ToolResultProjection::Ok(_))
        && exact.token_count > crate::TOOL_RESULT_STUB_TOKEN_LIMIT;
    let Some(read_call_id) = read_call_id.filter(|_| stub_applies) else {
        return (exact, exact_mode);
    };
    let Some(content) =
        crate::tool_result_stub::tool_result_stub(&crate::tool_result_stub::StubCall {
            call_id: candidate.call_id,
            tool_name: candidate.function_name,
            arguments: candidate.arguments,
            sources: candidate.sources,
            read_call_id: read_call_id.as_deref(),
        })
    else {
        return (exact, exact_mode);
    };
    let stub = ToolResultItem {
        token_count: count_context_item_tokens(&content),
        source_tokens: exact.source_tokens,
        kept_result_tokens: None,
        content,
    };
    (stub, ProjectionMode::ToolResultStub)
}

/// Gives each selected `tool_result` item of `record` whose candidate has the stub form its stub
/// (ADR 0093 section 5). The considered source keeps the size of the exact result.
pub(crate) fn stub_selected_results(
    record: &mut CurrentPassageAssemblyRecord,
    candidates: &[ToolResultCandidate<'_>],
) {
    let limit = record.operation_requirement.item_token_limit;
    let Some(sources) = record.operation_requirement.tool_results.as_mut() else {
        return;
    };
    let items = record
        .selected
        .iter_mut()
        .filter(|item| item.source_class == ContextSourceClass::ToolResult);
    for (source, item) in sources.iter_mut().zip(items) {
        let Some(candidate) = candidates.iter().find(|candidate| {
            candidate.tool_call_id == source.tool_call_id && candidate.form == ToolResultForm::Stub
        }) else {
            continue;
        };
        let Some(projection) = ToolResultProjection::of(candidate.state, candidate.result_text)
        else {
            continue;
        };
        let (stub, ProjectionMode::ToolResultStub) = candidate_item(candidate, projection, limit)
        else {
            continue;
        };
        item.content = stub.content;
        item.token_count = stub.token_count;
        item.projection_mode = ProjectionMode::ToolResultStub;
        source.kept_result_tokens = None;
    }
}

/// The ToolCall identity of each selected `tool_result` item of `record` that is a stub, in item
/// order.
pub fn stub_tool_call_ids(record: &CurrentPassageAssemblyRecord) -> Vec<String> {
    let items = record
        .selected
        .iter()
        .filter(|item| item.source_class == ContextSourceClass::ToolResult);
    record
        .operation_requirement
        .tool_results
        .iter()
        .flatten()
        .zip(items)
        .filter(|(_, item)| item.projection_mode == ProjectionMode::ToolResultStub)
        .map(|(source, _)| source.tool_call_id.clone())
        .collect()
}

#[cfg(test)]
#[path = "tool_result_context_tests.rs"]
mod tests;
