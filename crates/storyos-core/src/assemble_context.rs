//! Pure Core classification for one current-passage Context Assembly.

use super::count_stored_text;

pub const TOKEN_COUNTING_PROFILE_REVISION: &str = "storyos.token-counting.unicode-scalar.v1";
pub const TOKEN_COUNTING_ALGORITHM_REVISION: &str = "storyos.statistics.unicode-16.0.0.v1";
pub const CONTEXT_ITEM_TOKEN_LIMIT: u64 = 10_000;

/// Count one StoryOS-injected Context item under the admitted Token Counting Profile.
pub fn count_context_item_tokens(text: &str) -> u64 {
    count_stored_text(text).character_count
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentPassageAssembly {
    pub operation_requirement_id: String,
    pub input_snapshot_id: String,
    pub run_id: String,
    pub owner_user_id: String,
    pub project_id: String,
    pub author_message: String,
    pub chapter_id: String,
    pub chapter_revision_id: Option<String>,
    pub chapter_body: String,
    pub instruction: InstructionBindingInput,
    pub destination_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstructionBindingInput {
    Absent,
    RequiredRevision {
        revision_id: String,
        available: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextPurpose {
    CurrentPassageAssistance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextCause {
    AuthorRequest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextSourceClass {
    HostControl,
    AuthorInstruction,
    WorkingTarget,
    InstructionBinding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionMode {
    ExactRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DestinationIo {
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextSufficiency {
    Complete,
    Blocked { reasons: Vec<ContextBlockReason> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextBlockReason {
    ExactRequiredOverLimit { source_class: ContextSourceClass },
    RequiredInstructionRevisionUnavailable,
    WorkingTargetRevisionUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RejectionReason {
    OverItemTokenLimit,
    RequiredRevisionUnavailable,
    WorkingTargetRevisionUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationRequirementRecord {
    pub operation_requirement_id: String,
    pub input_snapshot_id: String,
    pub run_id: String,
    pub owner_user_id: String,
    pub project_id: String,
    pub purpose: ContextPurpose,
    pub cause: ContextCause,
    pub chapter_id: String,
    pub chapter_revision_id: Option<String>,
    pub instruction: InstructionBindingInput,
    pub destination_identity: String,
    pub item_token_limit: u64,
    pub token_counting_profile_revision: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsideredSource {
    pub source_class: ContextSourceClass,
    pub source_version: String,
    pub token_count: u64,
    pub eligible: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedProjection {
    pub source_class: ContextSourceClass,
    pub source_version: String,
    pub projection_mode: ProjectionMode,
    pub token_count: u64,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectedSource {
    pub source_class: ContextSourceClass,
    pub source_version: String,
    pub token_count: u64,
    pub reason: RejectionReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostControlRecord {
    pub distinct_from_destination: bool,
    pub destination_visible: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestCommit {
    pub assembly: bool,
    pub destination_and_outbound: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentPassageAssemblyRecord {
    pub operation_requirement: OperationRequirementRecord,
    pub sufficiency: ContextSufficiency,
    pub considered: Vec<ConsideredSource>,
    pub selected: Vec<SelectedProjection>,
    pub rejected: Vec<RejectedSource>,
    pub host_control: HostControlRecord,
    pub token_counting_profile_revision: String,
    pub token_counting_algorithm_revision: String,
    pub manifests: ManifestCommit,
    pub destination_io: DestinationIo,
}

/// Classify one current-passage Context Assembly without destination I/O.
pub fn assemble_current_passage_context(
    input: &CurrentPassageAssembly,
) -> CurrentPassageAssemblyRecord {
    let author_tokens = count_context_item_tokens(&input.author_message);
    let target_available = input.chapter_revision_id.is_some();
    let target_tokens = if target_available {
        count_context_item_tokens(&input.chapter_body)
    } else {
        0
    };
    let target_version = input
        .chapter_revision_id
        .clone()
        .unwrap_or_else(|| "unavailable".to_owned());
    let (instruction_version, instruction_available) = match &input.instruction {
        InstructionBindingInput::Absent => ("absent".to_owned(), true),
        InstructionBindingInput::RequiredRevision {
            revision_id,
            available,
        } => (revision_id.clone(), *available),
    };
    let author_over_limit = author_tokens > CONTEXT_ITEM_TOKEN_LIMIT;
    let target_over_limit = target_tokens > CONTEXT_ITEM_TOKEN_LIMIT;
    let mut rejected = Vec::new();
    let mut selected = Vec::new();
    let mut reasons = Vec::new();
    if !author_over_limit {
        selected.push(SelectedProjection {
            source_class: ContextSourceClass::AuthorInstruction,
            source_version: input.input_snapshot_id.clone(),
            projection_mode: ProjectionMode::ExactRequired,
            token_count: author_tokens,
            content: input.author_message.clone(),
        });
    } else {
        rejected.push(RejectedSource {
            source_class: ContextSourceClass::AuthorInstruction,
            source_version: input.input_snapshot_id.clone(),
            token_count: author_tokens,
            reason: RejectionReason::OverItemTokenLimit,
        });
        reasons.push(ContextBlockReason::ExactRequiredOverLimit {
            source_class: ContextSourceClass::AuthorInstruction,
        });
    }
    if !target_available {
        rejected.push(RejectedSource {
            source_class: ContextSourceClass::WorkingTarget,
            source_version: target_version.clone(),
            token_count: 0,
            reason: RejectionReason::WorkingTargetRevisionUnavailable,
        });
        reasons.push(ContextBlockReason::WorkingTargetRevisionUnavailable);
    } else if !target_over_limit {
        selected.push(SelectedProjection {
            source_class: ContextSourceClass::WorkingTarget,
            source_version: target_version.clone(),
            projection_mode: ProjectionMode::ExactRequired,
            token_count: target_tokens,
            content: input.chapter_body.clone(),
        });
    } else {
        rejected.push(RejectedSource {
            source_class: ContextSourceClass::WorkingTarget,
            source_version: target_version.clone(),
            token_count: target_tokens,
            reason: RejectionReason::OverItemTokenLimit,
        });
        reasons.push(ContextBlockReason::ExactRequiredOverLimit {
            source_class: ContextSourceClass::WorkingTarget,
        });
    }
    if !instruction_available {
        rejected.push(RejectedSource {
            source_class: ContextSourceClass::InstructionBinding,
            source_version: instruction_version.clone(),
            token_count: 0,
            reason: RejectionReason::RequiredRevisionUnavailable,
        });
        reasons.push(ContextBlockReason::RequiredInstructionRevisionUnavailable);
    }
    let complete = reasons.is_empty();
    CurrentPassageAssemblyRecord {
        operation_requirement: OperationRequirementRecord {
            operation_requirement_id: input.operation_requirement_id.clone(),
            input_snapshot_id: input.input_snapshot_id.clone(),
            run_id: input.run_id.clone(),
            owner_user_id: input.owner_user_id.clone(),
            project_id: input.project_id.clone(),
            purpose: ContextPurpose::CurrentPassageAssistance,
            cause: ContextCause::AuthorRequest,
            chapter_id: input.chapter_id.clone(),
            chapter_revision_id: input.chapter_revision_id.clone(),
            instruction: input.instruction.clone(),
            destination_identity: input.destination_identity.clone(),
            item_token_limit: CONTEXT_ITEM_TOKEN_LIMIT,
            token_counting_profile_revision: TOKEN_COUNTING_PROFILE_REVISION.to_owned(),
        },
        sufficiency: if complete {
            ContextSufficiency::Complete
        } else {
            ContextSufficiency::Blocked { reasons }
        },
        considered: vec![
            ConsideredSource {
                source_class: ContextSourceClass::HostControl,
                source_version: input.operation_requirement_id.clone(),
                token_count: 0,
                eligible: true,
            },
            ConsideredSource {
                source_class: ContextSourceClass::AuthorInstruction,
                source_version: input.input_snapshot_id.clone(),
                token_count: author_tokens,
                eligible: !author_over_limit,
            },
            ConsideredSource {
                source_class: ContextSourceClass::WorkingTarget,
                source_version: target_version,
                token_count: target_tokens,
                eligible: target_available && !target_over_limit,
            },
            ConsideredSource {
                source_class: ContextSourceClass::InstructionBinding,
                source_version: instruction_version,
                token_count: 0,
                eligible: instruction_available,
            },
        ],
        selected,
        rejected,
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

#[path = "assemble_context_codec.rs"]
mod assemble_context_codec;
pub use assemble_context_codec::{decode_assembly_record, encode_assembly_record};

#[cfg(test)]
#[path = "assemble_context_tests.rs"]
mod tests;
