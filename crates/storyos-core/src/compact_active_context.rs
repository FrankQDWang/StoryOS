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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactionInstallFacts {
    pub source_restricted: bool,
    pub exact_required_satisfied: bool,
    pub staged_input_digest: String,
    pub current_input_digest: String,
}

/// Author text that asks the Host-fake path to compact before the next request.
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

/// Install only when the staged input is still current, unrestricted, and exact-required complete.
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
    Ok(())
}

impl CompactionInstallRefusal {
    /// The recorded reason of the refusal.
    pub fn code(self) -> &'static str {
        match self {
            Self::RestrictedSource => "restricted_source",
            Self::ExactRequiredUnsatisfied => "exact_required_unsatisfied",
            Self::ChangedInput => "changed_input",
        }
    }
}

/// The durable state of the active compaction record of one AgentRun.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompactionRecordState {
    Absent,
    Staged,
    Installed,
    Refused,
}

/// The next step of the active compaction phase after a settled decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompactionStep {
    /// No compaction is requested, or it is already installed or refused: the AgentRun completes.
    Complete,
    /// Stage one compaction result, then hold until the next phase.
    Stage,
    /// Install the staged result for a later request, or refuse it.
    InstallOrRefuse,
}

/// Decides the next compaction step from the author request and the record state.
pub fn next_compaction_step(author_message: &str, record: CompactionRecordState) -> CompactionStep {
    if !requests_active_compaction(author_message) {
        return CompactionStep::Complete;
    }
    match record {
        CompactionRecordState::Absent => CompactionStep::Stage,
        CompactionRecordState::Staged => CompactionStep::InstallOrRefuse,
        CompactionRecordState::Installed | CompactionRecordState::Refused => {
            CompactionStep::Complete
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

/// The facts of the submitted decision Attempt that a compaction summarizes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactionSource {
    pub model_attempt_id: String,
    pub manifest_id: String,
    /// The (source class, source version) of each selected Context item, in order.
    pub selected: Vec<(String, String)>,
    pub item_ids: Vec<String>,
}

/// The content of one staged compaction result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedCompaction {
    pub producer: &'static str,
    pub mapping_revision: &'static str,
    pub output_text: &'static str,
    pub loss_facts: Vec<&'static str>,
    pub known_inputs: Vec<CompactionKnownInput>,
    pub preserved_item_ids: Vec<String>,
}

/// The Host-managed result of the Contract-Faithful Fake Destination: a fixed bounded summary
/// whose semantic preservation is unknown. Its known inputs are the submitted Attempt, its
/// manifest, and each selected projection.
pub fn stage_host_fake_compaction(source: &CompactionSource) -> StagedCompaction {
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
    StagedCompaction {
        producer: HOST_FAKE_COMPACTION_PRODUCER,
        mapping_revision: crate::HOST_FAKE_MAPPING_REVISION,
        output_text: HOST_FAKE_COMPACTION_OUTPUT,
        loss_facts: vec![COMPACTION_LOSS_SEMANTIC_PRESERVATION_UNKNOWN],
        known_inputs,
        preserved_item_ids: source.item_ids.clone(),
    }
}

#[cfg(test)]
#[path = "compact_active_context_tests.rs"]
mod tests;
