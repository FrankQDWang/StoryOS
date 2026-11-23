//! The Context Assembly of the RunSteps of a Subrun (ADR 0103 section 3A).

use crate::assemble_context::{
    CONTEXT_ITEM_TOKEN_LIMIT, ConsideredSource, ContextCause, ContextPurpose, ContextSourceClass,
    ContextSufficiency, CurrentPassageAssemblyRecord, DestinationIo, HostControlRecord,
    InstructionBindingInput, ManifestCommit, OperationRequirementRecord, ProjectionMode,
    SelectedProjection, TOKEN_COUNTING_ALGORITHM_REVISION, TOKEN_COUNTING_PROFILE_REVISION,
    count_context_item_tokens,
};
use crate::{BoundInstruction, ChapterListEntry, MemoryIndexEntry, ToolResultCandidate};

/// The classes of the items of the first RunStep of a Subrun. Each later RunStep keeps them.
const SUBRUN_INPUT_CLASSES: [ContextSourceClass; 4] = [
    ContextSourceClass::InstructionBinding,
    ContextSourceClass::SubrunBrief,
    ContextSourceClass::ChapterList,
    ContextSourceClass::MemoryIndex,
];

/// The identities and the input of the first RunStep of one Subrun. The Project Instruction and
/// the Memory facts are the facts of its parent Run.
#[derive(Clone, Copy, Debug)]
pub struct SubrunAssembly<'a> {
    pub operation_requirement_id: &'a str,
    pub run_id: &'a str,
    pub owner_user_id: &'a str,
    pub project_id: &'a str,
    pub destination_identity: &'a str,
    pub brief: &'a str,
    pub instruction: Option<&'a BoundInstruction>,
    pub chapters: &'a [ChapterListEntry],
    /// The Memory revision that the parent Run pinned, or `None` when it has no Memory use.
    pub memory_pin: Option<u64>,
    /// The Memory Index at `memory_pin`, with no Character Intent.
    pub memory_index: &'a [MemoryIndexEntry],
}

/// The Context Assembly of the first RunStep of a Subrun: the Purpose `subrun`, the Project
/// Instruction of the parent Run when it has one, the `subrun_brief` item, the Chapter list, and
/// the Memory Index of the pin of the parent Run.
pub fn assemble_subrun_context(input: &SubrunAssembly<'_>) -> CurrentPassageAssemblyRecord {
    let content = input.brief.to_owned();
    let item = SelectedProjection {
        source_class: ContextSourceClass::SubrunBrief,
        source_version: format!("sha256:{}", crate::hex_sha256(content.as_bytes())),
        projection_mode: ProjectionMode::ExactRequired,
        token_count: count_context_item_tokens(&content),
        content,
    };
    let mut record = CurrentPassageAssemblyRecord {
        operation_requirement: OperationRequirementRecord {
            operation_requirement_id: input.operation_requirement_id.to_owned(),
            input_snapshot_id: input.operation_requirement_id.to_owned(),
            run_id: input.run_id.to_owned(),
            owner_user_id: input.owner_user_id.to_owned(),
            project_id: input.project_id.to_owned(),
            purpose: ContextPurpose::Subrun,
            cause: ContextCause::AuthorRequest,
            chapter_id: None,
            chapter_revision_id: None,
            proposal_target_block_ids: None,
            passage_targets: None,
            candidate_target: None,
            selection_references: None,
            ordinary_resolution: None,
            tool_results: None,
            earlier_author_messages: None,
            earlier_turns: None,
            skill_requirements: None,
            skill_catalog: None,
            memory_revision: None,
            instruction: InstructionBindingInput::Absent,
            destination_identity: input.destination_identity.to_owned(),
            item_token_limit: CONTEXT_ITEM_TOKEN_LIMIT,
            token_counting_profile_revision: TOKEN_COUNTING_PROFILE_REVISION.to_owned(),
        },
        sufficiency: ContextSufficiency::Complete,
        considered: vec![ConsideredSource {
            source_class: item.source_class,
            source_version: item.source_version.clone(),
            token_count: item.token_count,
            eligible: true,
        }],
        selected: vec![item],
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
    };
    crate::assemble_instruction_binding(&mut record, input.instruction);
    crate::assemble_chapter_list(&mut record, input.chapters);
    crate::assemble_memory_index(&mut record, input.memory_pin, input.memory_index);
    record
}

/// The Context Assembly of a later RunStep of a Subrun: the items of the first RunStep, which the
/// RunStep `before` keeps, and the result of each resolved call of the Subrun (ADR 0085).
pub fn next_subrun_context(
    before: &CurrentPassageAssemblyRecord,
    operation_requirement_id: String,
    candidates: &[ToolResultCandidate<'_>],
) -> CurrentPassageAssemblyRecord {
    let mut record = before.clone();
    let requirement = &mut record.operation_requirement;
    requirement
        .input_snapshot_id
        .clone_from(&operation_requirement_id);
    requirement.operation_requirement_id = operation_requirement_id;
    let first = |class: ContextSourceClass| SUBRUN_INPUT_CLASSES.contains(&class);
    record.selected.retain(|item| first(item.source_class));
    record
        .considered
        .retain(|source| first(source.source_class));
    crate::assemble_tool_result_continuation(&mut record, candidates);
    record
}

#[cfg(test)]
#[path = "subrun_tests.rs"]
mod tests;
