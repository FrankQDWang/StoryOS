//! The input, the start rule, and the outcome of an intent pass (ADR 0101 section 4): the Context
//! Assembly of its RunSteps from the Character File that the store loads (ADR 0039), the decision
//! of a claimed schedule row, and the outcome of a pass whose Run ends with no settled call.

use crate::assemble_context::{
    CONTEXT_ITEM_TOKEN_LIMIT, ConsideredSource, ContextCause, ContextPurpose, ContextSourceClass,
    ContextSufficiency, CurrentPassageAssemblyRecord, DestinationIo, HostControlRecord,
    InstructionBindingInput, ManifestCommit, OperationRequirementRecord, ProjectionMode,
    SelectedProjection, TOKEN_COUNTING_ALGORITHM_REVISION, TOKEN_COUNTING_PROFILE_REVISION,
    count_context_item_tokens,
};
use crate::{CaptureRefusal, CaptureRunEnd, ToolResultCandidate};

/// The most RunSteps of one intent pass: one request, and one more after a refused call.
pub const INTENT_PASS_MAX_RUN_STEPS: u64 = 2;

/// Decides the start of an intent pass with the admission rule of a capture. The scope of the
/// grant names the Purpose `intent_pass`.
pub fn admit_intent_pass(
    binding: bool,
    assistance_available: bool,
    grant_scope: Option<&str>,
) -> Result<(), CaptureRefusal> {
    crate::memory_consolidation::admit_background_purpose(
        ContextPurpose::IntentPass,
        binding,
        assistance_available,
        grant_scope,
    )
}

/// The Character File of one character: its name, the text of the part of the author when the
/// Project has one, and the text of the part of the background work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CharacterFile<'a> {
    pub name: &'a str,
    pub author_text: Option<&'a str>,
    pub manuscript_text: &'a str,
}

impl CharacterFile<'_> {
    /// The exact text of the file item: the line `{"name": name}`, then each part with text under
    /// its heading.
    pub fn text(&self) -> String {
        let name = crate::canonical_json(&serde_json::json!({ "name": self.name }));
        let parts = [
            ("Told by the author", self.author_text),
            ("From the manuscript", Some(self.manuscript_text)),
        ];
        std::iter::once(name)
            .chain(parts.into_iter().filter_map(|(heading, text)| {
                text.filter(|text| !text.is_empty())
                    .map(|text| format!("{heading}:\n{text}"))
            }))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// The identities and the input of one RunStep of an intent pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntentPassAssembly<'a> {
    pub operation_requirement_id: String,
    pub run_id: String,
    pub owner_user_id: String,
    pub project_id: String,
    pub destination_identity: String,
    pub file: CharacterFile<'a>,
}

/// The Context Assembly of an intent pass: the Purpose `intent_pass`, the cause `host_schedule`,
/// and the one `character_file` item. The pass has no Memory Index, no Chapter, and no other entry
/// (ADR 0101 section 4).
pub fn assemble_intent_pass_context(
    input: &IntentPassAssembly<'_>,
) -> CurrentPassageAssemblyRecord {
    let content = input.file.text();
    let item = SelectedProjection {
        source_class: ContextSourceClass::CharacterFile,
        source_version: format!("sha256:{}", crate::hex_sha256(content.as_bytes())),
        projection_mode: ProjectionMode::ExactRequired,
        token_count: count_context_item_tokens(&content),
        content,
    };
    CurrentPassageAssemblyRecord {
        operation_requirement: OperationRequirementRecord {
            operation_requirement_id: input.operation_requirement_id.clone(),
            input_snapshot_id: input.operation_requirement_id.clone(),
            run_id: input.run_id.clone(),
            owner_user_id: input.owner_user_id.clone(),
            project_id: input.project_id.clone(),
            purpose: ContextPurpose::IntentPass,
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
    }
}

/// The Context Assembly of the second RunStep of an intent pass: the file item of the RunStep
/// `before` and the result of each resolved call of the Run.
pub fn next_intent_pass_context(
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
    let file = |class: ContextSourceClass| class == ContextSourceClass::CharacterFile;
    record.selected.retain(|item| file(item.source_class));
    record.considered.retain(|source| file(source.source_class));
    crate::assemble_tool_result_continuation(&mut record, candidates);
    record
}

/// The outcome of an intent pass whose call settled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntentPassOutcome {
    /// The result is a version of the part of the background work.
    Recorded,
    /// The part of the background work changed after the start of the pass, so the result is not
    /// kept.
    NotKept,
}

impl IntentPassOutcome {
    /// The persisted name of the outcome, for `intent_passes.outcome`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Recorded => "recorded",
            Self::NotKept => "not_kept",
        }
    }
}

/// The failure reason of a running intent pass from the facts of its Run. `None`: the pass
/// continues. A pass that ends with no settled `record_character_intent` call fails, and it
/// records no result.
pub fn intent_pass_failure(end: CaptureRunEnd<'_>) -> Option<&str> {
    match end {
        // The last RunStep ended with no call, or with a call that the Host refused.
        CaptureRunEnd::LastRunStepEnded
        | CaptureRunEnd::Status {
            status: "completed",
            ..
        } => Some("no_intent_call"),
        CaptureRunEnd::Status {
            status: "queued" | "claimed",
            ..
        } => None,
        CaptureRunEnd::Status { status, reason } => Some(reason.unwrap_or(status)),
    }
}

/// The facts of one claimed intent pass row of a Project.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntentPassFacts {
    /// A character entry of the background work needs a pass.
    pub entry_needs_pass: bool,
    /// An intent pass of the Project is in progress.
    pub pass_in_progress: bool,
}

/// Why a claimed intent pass row started no pass. The due time becomes empty in each case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntentPassSkip {
    /// The next settlement that makes the row due sets the due time again.
    NoEntryNeedsPass,
    /// The settlement of that pass sets the due time again when an entry still needs a pass.
    PassInProgress,
    /// The admission refused the start.
    StartRefused,
}

impl IntentPassSkip {
    pub fn label(self) -> &'static str {
        match self {
            Self::NoEntryNeedsPass => "no_entry_needs_pass",
            Self::PassInProgress => "pass_in_progress",
            Self::StartRefused => "start_refused",
        }
    }
}

/// Decides whether a claimed intent pass row starts one pass. `Ok`: one pass starts, when the
/// admission of the start permits it.
pub fn decide_intent_pass(facts: &IntentPassFacts) -> Result<(), IntentPassSkip> {
    if !facts.entry_needs_pass {
        return Err(IntentPassSkip::NoEntryNeedsPass);
    }
    if facts.pass_in_progress {
        return Err(IntentPassSkip::PassInProgress);
    }
    Ok(())
}

#[cfg(test)]
#[path = "intent_pass_tests.rs"]
mod tests;
