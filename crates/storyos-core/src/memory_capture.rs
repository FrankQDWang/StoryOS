//! The start of a capture (ADR 0094 sections 5 and 8): the admission from the facts that the store
//! loads (ADR 0039), and the Context Assembly of the first RunStep. Background work uses the
//! current Model Use Binding and grant of the Project and writes neither of them.

use crate::GrantScope;
use crate::assemble_context::{
    CONTEXT_ITEM_TOKEN_LIMIT, ConsideredSource, ContextCause, ContextPurpose, ContextSourceClass,
    ContextSufficiency, CurrentPassageAssemblyRecord, DestinationIo, HostControlRecord,
    InstructionBindingInput, ManifestCommit, OperationRequirementRecord, ProjectionMode,
    SelectedProjection, TOKEN_COUNTING_ALGORITHM_REVISION, TOKEN_COUNTING_PROFILE_REVISION,
    count_context_item_tokens,
};

/// The facts of one start of a capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureAdmissionFacts<'a> {
    /// Whether the Project has a current Model Use Binding.
    pub binding: bool,
    /// Whether the assistance of the Project is available.
    pub assistance_available: bool,
    /// The scope of the grant of the current binding.
    pub grant_scope: Option<&'a str>,
    pub input: &'a str,
}

/// What ended the Run of a running capture before an output call settled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureRunEnd<'a> {
    /// Each call of the last RunStep resolved, or its response had no call.
    LastRunStepEnded,
    /// The Run got the status `status`, with the reason of its settlement.
    Status {
        status: &'a str,
        reason: Option<&'a str>,
    },
}

/// The outcome of a capture that ended with no settled output call, and the reason of a failure.
/// It stores no observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureEnd<'a> {
    pub outcome: crate::CaptureOutcome,
    pub failure_reason: Option<&'a str>,
}

/// The outcome of a running capture from the facts of its Run (ADR 0039, ADR 0094 section 5).
/// `None`: the capture continues. A Run that ends in another way fails the capture. The Host
/// starts no second capture for the same input by itself.
pub fn capture_end(end: CaptureRunEnd<'_>) -> Option<CaptureEnd<'_>> {
    let (status, reason) = match end {
        CaptureRunEnd::LastRunStepEnded => {
            return Some(CaptureEnd {
                outcome: crate::CaptureOutcome::StepLimit,
                failure_reason: None,
            });
        }
        CaptureRunEnd::Status { status, reason } => (status, reason),
    };
    let default_reason = match status {
        "queued" | "claimed" => return None,
        // A Run that completes before an output call settled made no Agent Decision.
        "completed" => "no_agent_decision",
        other => other,
    };
    Some(CaptureEnd {
        outcome: crate::CaptureOutcome::Failed,
        failure_reason: Some(reason.unwrap_or(default_reason)),
    })
}

/// Why the Host refused to start a capture. A refused start writes no row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureRefusal {
    NoModelUseBinding,
    AssistanceUnavailable,
    /// The scope of the grant does not name the Purpose `memory_capture`.
    PurposeNotGranted,
    /// The input is above the Context item limit (ADR 0052).
    InputOverItemLimit,
}

impl CaptureRefusal {
    pub fn label(self) -> &'static str {
        match self {
            Self::NoModelUseBinding => "no_model_use_binding",
            Self::AssistanceUnavailable => "assistance_unavailable",
            Self::PurposeNotGranted => "purpose_not_granted",
            Self::InputOverItemLimit => "input_over_item_limit",
        }
    }
}

/// Decides the start of a capture. The checks are in the order of the refusals.
pub fn admit_memory_capture(facts: &CaptureAdmissionFacts<'_>) -> Result<(), CaptureRefusal> {
    if !facts.binding {
        return Err(CaptureRefusal::NoModelUseBinding);
    }
    if !facts.assistance_available {
        return Err(CaptureRefusal::AssistanceUnavailable);
    }
    let granted = facts
        .grant_scope
        .and_then(GrantScope::parse)
        .is_some_and(|scope| scope.names_purpose(ContextPurpose::MemoryCapture));
    if !granted {
        return Err(CaptureRefusal::PurposeNotGranted);
    }
    if count_context_item_tokens(facts.input) > CONTEXT_ITEM_TOKEN_LIMIT {
        return Err(CaptureRefusal::InputOverItemLimit);
    }
    Ok(())
}

/// The identities and the input of the first RunStep of one capture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryCaptureAssembly {
    pub operation_requirement_id: String,
    pub input_snapshot_id: String,
    pub run_id: String,
    pub owner_user_id: String,
    pub project_id: String,
    pub input: String,
    pub destination_identity: String,
}

/// The Context Assembly of a capture: the Purpose `memory_capture`, the cause `host_schedule`,
/// and one `memory_capture_input` item with the projection mode `exact_required`. The admission
/// refused an input above the item limit, so the assembly is complete.
pub fn assemble_memory_capture_context(
    input: &MemoryCaptureAssembly,
) -> CurrentPassageAssemblyRecord {
    let token_count = count_context_item_tokens(&input.input);
    CurrentPassageAssemblyRecord {
        operation_requirement: OperationRequirementRecord {
            operation_requirement_id: input.operation_requirement_id.clone(),
            input_snapshot_id: input.input_snapshot_id.clone(),
            run_id: input.run_id.clone(),
            owner_user_id: input.owner_user_id.clone(),
            project_id: input.project_id.clone(),
            purpose: ContextPurpose::MemoryCapture,
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
            memory_revision: None,
            instruction: InstructionBindingInput::Absent,
            destination_identity: input.destination_identity.clone(),
            item_token_limit: CONTEXT_ITEM_TOKEN_LIMIT,
            token_counting_profile_revision: TOKEN_COUNTING_PROFILE_REVISION.to_owned(),
        },
        sufficiency: ContextSufficiency::Complete,
        considered: vec![ConsideredSource {
            source_class: ContextSourceClass::MemoryCaptureInput,
            source_version: input.input_snapshot_id.clone(),
            token_count,
            eligible: true,
        }],
        selected: vec![SelectedProjection {
            source_class: ContextSourceClass::MemoryCaptureInput,
            source_version: input.input_snapshot_id.clone(),
            projection_mode: ProjectionMode::ExactRequired,
            token_count,
            content: input.input.clone(),
        }],
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

/// The reference items of the first RunStep of a capture (ADR 0094 sections 4, 5, and 8).
#[derive(Clone, Copy, Debug)]
pub struct CaptureReferences<'a> {
    /// The Memory revision of the Project head at the start of the capture.
    pub memory_head: u64,
    /// The current entries at that revision.
    pub memory_index: &'a [crate::MemoryIndexEntry],
    pub chapters: &'a [crate::ChapterListEntry],
    /// The marks of the entries that the author deleted, newest first.
    pub delete_marks: &'a [crate::MemoryDeleteMark],
}

/// The source classes of the reference items of a capture, in their order after the input.
const CAPTURE_REFERENCE_CLASSES: [ContextSourceClass; 3] = [
    ContextSourceClass::MemoryIndex,
    ContextSourceClass::ChapterList,
    ContextSourceClass::MemoryDeleteMarks,
];

/// Adds the reference items after the input item: the Memory Index at the head revision of the
/// start, the Chapter list, and the delete marks. A capture always gets the Index. The
/// Conversation Memory Settings control it only for a Run of a Project Conversation.
pub fn assemble_capture_references(
    record: &mut CurrentPassageAssemblyRecord,
    references: &CaptureReferences<'_>,
) {
    let pin = crate::memory_revision_pin(/*use_enabled*/ true, references.memory_head);
    crate::assemble_memory_index(record, pin, references.memory_index);
    crate::assemble_chapter_list(record, references.chapters);
    crate::assemble_memory_delete_marks(record, references.delete_marks);
}

/// Keeps the reference items and the Memory revision of the RunStep `before` in the Context
/// Assembly of the next RunStep, so that they stay a part of the current input (ADR 0085).
pub fn keep_capture_references(
    record: &mut CurrentPassageAssemblyRecord,
    before: &CurrentPassageAssemblyRecord,
) {
    record.operation_requirement.memory_revision = before.operation_requirement.memory_revision;
    let kept = |class: &ContextSourceClass| CAPTURE_REFERENCE_CLASSES.contains(class);
    record.considered.extend(
        before
            .considered
            .iter()
            .filter(|source| kept(&source.source_class))
            .cloned(),
    );
    record.selected.extend(
        before
            .selected
            .iter()
            .filter(|item| kept(&item.source_class))
            .cloned(),
    );
}

#[cfg(test)]
#[path = "memory_capture_tests.rs"]
mod tests;
