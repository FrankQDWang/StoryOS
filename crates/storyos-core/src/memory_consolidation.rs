//! The input and the outcome of a consolidation pass (ADR 0094 sections 6 and 8): the Context
//! Assembly of its one RunStep from the facts that the store loads (ADR 0039), the outcomes, and
//! the outcome of a pass whose Run ends with no settled output call.

use crate::assemble_context::{
    CONTEXT_ITEM_TOKEN_LIMIT, ConsideredSource, ContextCause, ContextPurpose, ContextSourceClass,
    ContextSufficiency, CurrentPassageAssemblyRecord, DestinationIo, HostControlRecord,
    InstructionBindingInput, ManifestCommit, OperationRequirementRecord, ProjectionMode,
    SelectedProjection, TOKEN_COUNTING_ALGORITHM_REVISION, TOKEN_COUNTING_PROFILE_REVISION,
    count_context_item_tokens,
};
use crate::{
    CaptureRunEnd, MemoryEntryOrigin, MemoryEntryType, MemorySourceReference, PassMemoryEntry,
    canonical_json,
};

/// One observation of the input of a pass. Its number in the input is its position, from 1.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassObservation {
    pub entry_type: MemoryEntryType,
    pub title: String,
    pub text: String,
    pub source: MemorySourceReference,
}

impl PassObservation {
    /// The origin of an entry that the observation creates.
    pub fn origin(&self) -> MemoryEntryOrigin {
        match self.source {
            MemorySourceReference::Conversation { .. } => MemoryEntryOrigin::Conversation,
            MemorySourceReference::Chapter { .. } => MemoryEntryOrigin::Chapter,
        }
    }
}

/// The identities and the input of the one RunStep of a pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryConsolidationAssembly<'a> {
    pub operation_requirement_id: String,
    pub run_id: String,
    pub owner_user_id: String,
    pub project_id: String,
    pub destination_identity: String,
    /// The Memory revision that the pass pinned.
    pub memory_revision: u64,
    /// Each current entry at that revision, with the time of its last change.
    pub entries: &'a [(PassMemoryEntry, String)],
    /// Each observation that waits, the oldest first.
    pub observations: &'a [PassObservation],
}

/// The Context Assembly of a pass: the Purpose `memory_consolidation`, the cause
/// `host_schedule`, the current entries as `memory_entries` items, and the observations as
/// `memory_observations` items. Each line is one JSON object, and each item holds the lines that
/// fit the Context item limit (ADR 0052), so a large set is more than one item.
pub fn assemble_memory_consolidation_context(
    input: &MemoryConsolidationAssembly<'_>,
) -> CurrentPassageAssemblyRecord {
    let entry_lines = input.entries.iter().map(|(pass_entry, changed_at)| {
        let entry = &pass_entry.entry;
        canonical_json(&serde_json::json!({
            "id": entry.number,
            "type": entry.entry_type.as_str(),
            "title": entry.title,
            "description": entry.description,
            "text": entry.text,
            "origin": pass_entry.origin.as_str(),
            "changed_at": changed_at,
        }))
    });
    let observation_lines = (1_u64..).zip(input.observations).map(|(number, item)| {
        canonical_json(&serde_json::json!({
            "observation": number,
            "type": item.entry_type.as_str(),
            "title": item.title,
            "text": item.text,
            "source": item.source,
        }))
    });
    let selected: Vec<SelectedProjection> = items(ContextSourceClass::MemoryEntries, entry_lines)
        .chain(items(
            ContextSourceClass::MemoryObservations,
            observation_lines,
        ))
        .collect();
    CurrentPassageAssemblyRecord {
        operation_requirement: OperationRequirementRecord {
            operation_requirement_id: input.operation_requirement_id.clone(),
            input_snapshot_id: input.operation_requirement_id.clone(),
            run_id: input.run_id.clone(),
            owner_user_id: input.owner_user_id.clone(),
            project_id: input.project_id.clone(),
            purpose: ContextPurpose::MemoryConsolidation,
            cause: ContextCause::HostSchedule,
            chapter_id: None,
            chapter_revision_id: None,
            proposal_target_block_ids: None,
            passage_targets: None,
            candidate_target: None,
            selection_references: None,
            ordinary_resolution: None,
            tool_results: None,
            earlier_author_messages: None,
            skill_requirements: None,
            skill_catalog: None,
            memory_revision: Some(input.memory_revision),
            instruction: InstructionBindingInput::Absent,
            destination_identity: input.destination_identity.clone(),
            item_token_limit: CONTEXT_ITEM_TOKEN_LIMIT,
            token_counting_profile_revision: TOKEN_COUNTING_PROFILE_REVISION.to_owned(),
        },
        sufficiency: ContextSufficiency::Complete,
        considered: selected
            .iter()
            .map(|item| ConsideredSource {
                source_class: item.source_class,
                source_version: item.source_version.clone(),
                token_count: item.token_count,
                eligible: true,
            })
            .collect(),
        selected,
        rejected: Vec::new(),
        host_control: HostControlRecord {
            distinct_from_destination: true,
            destination_visible: false,
        },
        token_counting_profile_revision: TOKEN_COUNTING_PROFILE_REVISION.to_owned(),
        token_counting_algorithm_revision: TOKEN_COUNTING_ALGORITHM_REVISION.to_owned(),
        manifests: ManifestCommit {
            assembly: true,
            destination_and_outbound: false,
        },
        destination_io: DestinationIo::None,
    }
}

/// The items of `lines` in order: each item holds the next lines while its text fits the Context
/// item limit. A line is an entry or an observation, so one line fits the limit.
fn items(
    source_class: ContextSourceClass,
    lines: impl Iterator<Item = String>,
) -> impl Iterator<Item = SelectedProjection> {
    let mut texts: Vec<String> = Vec::new();
    for line in lines {
        match texts.last_mut() {
            Some(text)
                if count_context_item_tokens(&format!("{text}\n{line}"))
                    <= CONTEXT_ITEM_TOKEN_LIMIT =>
            {
                text.push('\n');
                text.push_str(&line);
            }
            _ => texts.push(line),
        }
    }
    texts.into_iter().map(move |content| SelectedProjection {
        source_class,
        source_version: format!("sha256:{}", crate::hex_sha256(content.as_bytes())),
        projection_mode: ProjectionMode::ExactRequired,
        token_count: count_context_item_tokens(&content),
        content,
    })
}

/// The outcome of a pass (ADR 0094 section 6).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsolidationOutcome {
    /// One or more operations applied.
    Applied,
    /// The settled list applied no operation.
    NoOperation,
    /// The Memory revision at the settlement is not the pinned revision, so nothing applied.
    Stale,
    /// The Run ended with no settled output call.
    Failed,
}

impl ConsolidationOutcome {
    /// The persisted name of the outcome, for `memory_consolidation_passes.outcome`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::NoOperation => "no_operation",
            Self::Stale => "stale",
            Self::Failed => "failed",
        }
    }
}

/// The failure reason of a running pass from the facts of its Run (ADR 0039). `None`: the pass
/// continues. A pass that ends with no settled `apply_memory_operations` call fails, and it
/// applies nothing.
pub fn consolidation_failure(end: CaptureRunEnd<'_>) -> Option<&str> {
    match end {
        // The one RunStep ended with no call, or with a call that the Host refused.
        CaptureRunEnd::LastRunStepEnded
        | CaptureRunEnd::Status {
            status: "completed",
            ..
        } => Some("no_operation_call"),
        CaptureRunEnd::Status {
            status: "queued" | "claimed",
            ..
        } => None,
        CaptureRunEnd::Status { status, reason } => Some(reason.unwrap_or(status)),
    }
}

#[cfg(test)]
#[path = "memory_consolidation_tests.rs"]
mod tests;
