//! Decide whether one staged active-context compaction may install.

use crate::{canonical_json, hex_sha256};

pub const ACTIVE_COMPACTION_REQUEST_PREFIX: &str = "Compact active context between calls.";
pub const HOST_FAKE_COMPACTION_PRODUCER: &str = "host_fake_summary";
pub const HOST_FAKE_COMPACTION_OUTPUT: &str =
    "Bounded later-request summary. Semantic preservation is unknown.";
pub const COMPACTION_LOSS_SEMANTIC_PRESERVATION_UNKNOWN: &str = "semantic_preservation_unknown";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompactionInstallRefusal {
    RestrictedSource,
    ExactRequiredUnsatisfied,
    ChangedInput,
    SummaryOverItemLimit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactionInstallFacts {
    pub source_restricted: bool,
    pub exact_required_satisfied: bool,
    pub staged_input_digest: String,
    pub current_input_digest: String,
    /// Whether the summary fits the item token limit of the later request.
    pub summary_within_item_limit: bool,
}

/// Author text that asks the Host-fake path to compact before the decision request.
pub fn requests_active_compaction(author_message: &str) -> bool {
    author_message.starts_with(ACTIVE_COMPACTION_REQUEST_PREFIX)
}

/// Digest the exact author message and Working Target captured for one install check.
pub fn active_context_input_digest(
    author_message: &str,
    chapter_revision_id: &str,
    chapter_body: &str,
) -> String {
    format!(
        "sha256:{}",
        hex_sha256(
            canonical_json(&serde_json::json!({
                "author_message": author_message,
                "chapter_revision_id": chapter_revision_id,
                "chapter_body": chapter_body,
            }))
            .as_bytes(),
        )
    )
}

/// Install only when the staged input is still current, unrestricted, and exact-required complete,
/// and the summary fits one Context item.
pub fn decide_compaction_install(
    facts: &CompactionInstallFacts,
) -> Result<(), CompactionInstallRefusal> {
    if facts.source_restricted {
        return Err(CompactionInstallRefusal::RestrictedSource);
    }
    if !facts.exact_required_satisfied {
        return Err(CompactionInstallRefusal::ExactRequiredUnsatisfied);
    }
    if facts.staged_input_digest != facts.current_input_digest {
        return Err(CompactionInstallRefusal::ChangedInput);
    }
    if !facts.summary_within_item_limit {
        return Err(CompactionInstallRefusal::SummaryOverItemLimit);
    }
    Ok(())
}

impl CompactionInstallRefusal {
    /// The recorded reason of the refusal.
    pub fn code(self) -> &'static str {
        match self {
            Self::RestrictedSource => "restricted_source",
            Self::ExactRequiredUnsatisfied => "exact_required_unsatisfied",
            Self::ChangedInput => "changed_input",
            Self::SummaryOverItemLimit => "summary_over_item_limit",
        }
    }
}

/// The durable state of the active compaction of one AgentRun.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompactionRecordState {
    Absent,
    /// The summary request has a dispatch claim and no recorded result.
    SummaryClaimed,
    Staged,
    /// The install check passed, and the Context Assembly of the later request is committed.
    Accepted,
    Installed,
    Refused,
}

/// The step of the active compaction phase before the decision request of the AgentRun.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompactionStep {
    /// No compaction applies, or the install refused: send the decision request with the
    /// ordinary continuation.
    OrdinaryRequest,
    /// Send the summary request through the Model Gateway, and stage its result.
    RequestSummary,
    /// Observe the summary request that a lost claim committed. It sends nothing again.
    ReobserveSummary,
    /// Install the staged result for the decision request, or refuse it.
    InstallOrRefuse,
    /// Send the decision request as the later request: the summary and a new transport.
    LaterRequest,
}

/// Decides the next compaction step from the author request, the response chain of the
/// Conversation, and the record state. With no prior response, nothing is to be compacted.
pub fn next_compaction_step(
    author_message: &str,
    prior_response: bool,
    record: CompactionRecordState,
) -> CompactionStep {
    if !requests_active_compaction(author_message) {
        return CompactionStep::OrdinaryRequest;
    }
    match record {
        CompactionRecordState::Absent if prior_response => CompactionStep::RequestSummary,
        CompactionRecordState::Absent | CompactionRecordState::Refused => {
            CompactionStep::OrdinaryRequest
        }
        CompactionRecordState::SummaryClaimed => CompactionStep::ReobserveSummary,
        CompactionRecordState::Staged => CompactionStep::InstallOrRefuse,
        CompactionRecordState::Accepted | CompactionRecordState::Installed => {
            CompactionStep::LaterRequest
        }
    }
}

/// One known input of a compaction result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompactionKnownInput {
    ModelAttempt {
        id: String,
    },
    Manifest {
        id: String,
    },
    Projection {
        source_class: String,
        source_version: String,
    },
}

/// The facts of the prior decision Attempt of the Conversation that a compaction summarizes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactionSource {
    pub model_attempt_id: String,
    pub manifest_id: String,
    /// The (source class, source version) of each selected Context item, in order.
    pub selected: Vec<(String, String)>,
    pub item_ids: Vec<String>,
}

/// The content of one compaction result from the observation of its summary request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedCompaction {
    pub producer: &'static str,
    pub mapping_revision: String,
    pub output_text: String,
    pub loss_facts: Vec<&'static str>,
    pub known_inputs: Vec<CompactionKnownInput>,
    pub preserved_item_ids: Vec<String>,
    /// `None` when the summary request supplied a summary, otherwise the refusal reason.
    pub refusal: Option<&'static str>,
}

/// The summary that one exchange of the summary request supplied, as the destination reported.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SummaryObservation {
    pub adapter: crate::ModelAdapter,
    pub mapping_revision: String,
    /// The complete summary text, or `None` for no complete result.
    pub summary: Option<String>,
}

/// The reason of a compaction that has no summary: the request got no complete result.
pub const COMPACTION_SUMMARY_UNAVAILABLE: &str = "summary_unavailable";

/// The Host-managed result of one summary request. Its known inputs are the prior decision
/// Attempt, its manifest, and each selected projection. Its semantic preservation is unknown.
pub fn stage_compaction(
    source: &CompactionSource,
    observation: SummaryObservation,
) -> StagedCompaction {
    let mut known_inputs = vec![
        CompactionKnownInput::ModelAttempt {
            id: source.model_attempt_id.clone(),
        },
        CompactionKnownInput::Manifest {
            id: source.manifest_id.clone(),
        },
    ];
    known_inputs.extend(
        source
            .selected
            .iter()
            .map(
                |(source_class, source_version)| CompactionKnownInput::Projection {
                    source_class: source_class.clone(),
                    source_version: source_version.clone(),
                },
            ),
    );
    let producer = match observation.adapter {
        crate::ModelAdapter::HostFake => HOST_FAKE_COMPACTION_PRODUCER,
        crate::ModelAdapter::VolcengineAgentPlanResponses => AGENT_PLAN_COMPACTION_PRODUCER,
    };
    let refusal = observation
        .summary
        .is_none()
        .then_some(COMPACTION_SUMMARY_UNAVAILABLE);
    StagedCompaction {
        producer,
        mapping_revision: observation.mapping_revision,
        output_text: observation.summary.unwrap_or_default(),
        loss_facts: vec![COMPACTION_LOSS_SEMANTIC_PRESERVATION_UNKNOWN],
        known_inputs,
        preserved_item_ids: source.item_ids.clone(),
        refusal,
    }
}

/// The text of the one complete assistant message of a summary response. A refusal, a tool or
/// hosted item, or a missing or incomplete message supplies no summary.
pub fn summary_text(items: &[crate::NativeStreamItem]) -> Option<String> {
    if items
        .iter()
        .any(|item| item.refusal.is_some() || item.role != crate::StreamItemRole::Assistant)
    {
        return None;
    }
    let mut messages = items.iter().filter(|item| item.text.is_some());
    let message = messages.next()?;
    (messages.next().is_none() && message.state == crate::StreamItemState::Complete)
        .then(|| message.text.clone())
        .flatten()
        .filter(|text| !text.trim().is_empty())
}

/// The producer of a summary that the Agent Plan route made.
pub const AGENT_PLAN_COMPACTION_PRODUCER: &str = "agent_plan_summary";

/// The Context Assembly of a summary request: the Purpose of active compaction and no selected
/// project content. The request sends one Host instruction, and the continuation chain of the
/// Conversation supplies the history.
pub fn compaction_assembly_record(
    decision: &crate::CurrentPassageAssemblyRecord,
) -> crate::CurrentPassageAssemblyRecord {
    let mut record = decision.clone();
    record.operation_requirement.purpose =
        crate::assemble_context::ContextPurpose::ActiveContextCompaction;
    record.considered.clear();
    record.selected.clear();
    record.rejected.clear();
    record
}

/// Whether the summary fits the item token limit of the decision Context Assembly.
pub fn summary_fits_item(decision: &crate::CurrentPassageAssemblyRecord, summary: &str) -> bool {
    crate::assemble_context::count_context_item_tokens(summary)
        <= decision.operation_requirement.item_token_limit
}

/// The Context Assembly of the later request: the decision Context Assembly with the summary of
/// the same Conversation as the first selected item. The summary is a Derived Summary, so the
/// sufficiency of the Exact Required input does not change.
pub fn later_request_record(
    decision: &crate::CurrentPassageAssemblyRecord,
    compaction_id: &str,
    summary: &str,
) -> crate::CurrentPassageAssemblyRecord {
    let mut record = decision.clone();
    let token_count = crate::assemble_context::count_context_item_tokens(summary);
    record.considered.insert(
        /*index*/ 0,
        crate::assemble_context::ConsideredSource {
            source_class: crate::ContextSourceClass::ConversationSummary,
            source_version: compaction_id.to_owned(),
            token_count,
            eligible: true,
        },
    );
    record.selected.insert(
        /*index*/ 0,
        crate::assemble_context::SelectedProjection {
            source_class: crate::ContextSourceClass::ConversationSummary,
            source_version: compaction_id.to_owned(),
            projection_mode: crate::assemble_context::ProjectionMode::DerivedSummary,
            token_count,
            content: summary.to_owned(),
        },
    );
    record
}

#[cfg(test)]
#[path = "compact_active_context_tests.rs"]
mod tests;
