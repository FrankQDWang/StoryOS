//! The Host Tools (ADR 0091): Tools whose Registration binds a built-in implementation that
//! StoryOS executes in its own process. A Host ToolCall has no Destination Attempt and no
//! Outbound Disclosure Event, and it gives the Agent no authority.

use crate::{
    ContextSourceClass, FINISH_TOOL_SPEC, HYBRID_SEARCH_MANUSCRIPT_TOOL_SPEC, LOAD_SKILL_TOOL_SPEC,
    ModelRegistration, PROPOSE_EDIT_TOOL_SPEC, READ_CHAPTER_TOOL_SPEC, READ_MEMORY_ENTRY_TOOL_SPEC,
    READ_SKILL_RESOURCE_TOOL_SPEC, READ_TOOL_RESULT_TOOL_SPEC,
    RECORD_MEMORY_OBSERVATIONS_TOOL_SPEC, SEARCH_MANUSCRIPT_TOOL_SPEC, SEARCH_MEMORY_TOOL_SPEC,
    SUBMIT_RESEARCH_TOOL_SPEC, ToolIntake, ToolOperation, ToolRegistrationKind, ToolResultText,
    ToolSpec, WRITE_MEMORY_TOOL_SPEC,
};

/// The Tool Registration of the Host Tools. It has no server, credential, or destination.
#[derive(Debug, Eq, PartialEq)]
pub struct HostToolRegistration {
    pub id: &'static str,
    pub revision: &'static str,
    pub kind: ToolRegistrationKind,
    pub adapter_revision: &'static str,
    pub tools: &'static [&'static ToolSpec],
}

/// The current Host Tools: the Host Tools of ADR 0091, `read_tool_result` (ADR 0093),
/// `load_skill` and `read_skill_resource` (ADR 0089 section 3), `read_chapter`,
/// `search_manuscript`, `hybrid_search_manuscript`, the Memory Tools `write_memory`,
/// `read_memory_entry`, and `search_memory`, the output Tool `record_memory_observations` of a
/// capture, the output Tool `apply_memory_operations` of a consolidation pass (ADR 0094, ADR
/// 0095), and the output Tool `record_character_intent` of an intent pass (ADR 0101). The Tools
/// are before `finish`.
/// `ExposureFacts::has_host_tool` decides which of them a RunStep declares.
pub const HOST_TOOL_REGISTRATION: HostToolRegistration = HostToolRegistration {
    id: "host_tools",
    revision: "018f0000-0000-7001-8000-0000000e0104",
    kind: ToolRegistrationKind::Host,
    adapter_revision: "storyos.host-tool.v1",
    tools: &[
        &PROPOSE_EDIT_TOOL_SPEC,
        &SUBMIT_RESEARCH_TOOL_SPEC,
        &READ_TOOL_RESULT_TOOL_SPEC,
        &LOAD_SKILL_TOOL_SPEC,
        &READ_SKILL_RESOURCE_TOOL_SPEC,
        &READ_CHAPTER_TOOL_SPEC,
        &SEARCH_MANUSCRIPT_TOOL_SPEC,
        &HYBRID_SEARCH_MANUSCRIPT_TOOL_SPEC,
        &WRITE_MEMORY_TOOL_SPEC,
        &READ_MEMORY_ENTRY_TOOL_SPEC,
        &SEARCH_MEMORY_TOOL_SPEC,
        &RECORD_MEMORY_OBSERVATIONS_TOOL_SPEC,
        &crate::APPLY_MEMORY_OPERATIONS_TOOL_SPEC,
        &crate::RECORD_CHARACTER_INTENT_TOOL_SPEC,
        &FINISH_TOOL_SPEC,
    ],
};

/// The Context source class of the settled result of the Host Tool `spec` (ADR 0095 section 1).
/// Each Host Tool of the manuscript or of the Memory has its own class, and each other Host Tool
/// gives a `tool_result` item. The output of a refused call is a `tool_result` item.
pub fn host_tool_result_class(spec: &ToolSpec) -> ContextSourceClass {
    match spec.operation {
        ToolOperation::ReadChapter => ContextSourceClass::ManuscriptReadResult,
        ToolOperation::SearchManuscript | ToolOperation::HybridSearchManuscript => {
            ContextSourceClass::ManuscriptSearchResult
        }
        ToolOperation::ReadMemoryEntry => ContextSourceClass::MemoryReadResult,
        ToolOperation::SearchMemory => ContextSourceClass::MemorySearchResult,
        ToolOperation::WebSearch
        | ToolOperation::WebFetch
        | ToolOperation::Finish
        | ToolOperation::ProposeEdit
        | ToolOperation::SubmitResearch
        | ToolOperation::ReadToolResult
        | ToolOperation::LoadSkill
        | ToolOperation::ReadSkillResource
        | ToolOperation::WriteMemory
        | ToolOperation::RecordMemoryObservations
        | ToolOperation::ApplyMemoryOperations
        | ToolOperation::RecordCharacterIntent => ContextSourceClass::ToolResult,
    }
}

impl ModelRegistration {
    /// The Host Tools that the request mapping declares: each current mapping declares the current
    /// Host Tools (ADR 0097).
    pub fn host_tools(&self) -> &'static HostToolRegistration {
        &HOST_TOOL_REGISTRATION
    }
}

/// The Host Tool of the pinned revisions of a ToolCall. `None` is a ToolCall of another
/// Registration.
pub fn host_tool(registration_revision: &str, spec_revision: &str) -> Option<&'static ToolSpec> {
    (registration_revision == HOST_TOOL_REGISTRATION.revision)
        .then_some(HOST_TOOL_REGISTRATION.tools)?
        .iter()
        .copied()
        .find(|spec| spec.revision == spec_revision)
}

/// The result of a `finish` call beside a refused call of the same response (ADR 0091 section 4).
pub const FINISH_DOES_NOT_COUNT_RESULT: &str = "Another call of this response was refused, so this \
finish call does not count and the work continues. Correct the refused call, and then call finish \
again.";

/// Executes one validated `finish` call. Its result is empty when no other call of its response
/// was refused, and otherwise it says that the work continues. The store executes each other
/// Host Tool.
pub fn execute_host_tool(intake: &ToolIntake, refused_beside: bool) -> Option<ToolResultText> {
    match intake {
        ToolIntake::Finish => Some(host_tool_result(if refused_beside {
            FINISH_DOES_NOT_COUNT_RESULT
        } else {
            ""
        })),
        // The store executes `propose_edit`, because it opens a Proposal, `submit_research`,
        // because it records a Research Artifact, `read_tool_result`, because it reads a stored
        // ToolCall, and `load_skill` and `read_skill_resource`, because they read a SkillPackage
        // Snapshot, `read_chapter`, `search_manuscript`, and `hybrid_search_manuscript`, because
        // they read the manuscript,
        // `write_memory`, because it writes a Memory Entry, `read_memory_entry`, because it
        // reads a Memory Entry and records the read, `search_memory`, because it reads the
        // Memory Entries, `record_memory_observations`, because it writes the observations, and
        // `apply_memory_operations`, because it writes the Memory Entries, and
        // `record_character_intent`, because it writes a version of a Memory Entry.
        ToolIntake::WebSearch { .. }
        | ToolIntake::WebFetch { .. }
        | ToolIntake::ProposeEdit(_)
        | ToolIntake::SubmitResearch { .. }
        | ToolIntake::ReadToolResult { .. }
        | ToolIntake::LoadSkill { .. }
        | ToolIntake::ReadSkillResource { .. }
        | ToolIntake::ReadChapter { .. }
        | ToolIntake::SearchManuscript { .. }
        | ToolIntake::HybridSearchManuscript { .. }
        | ToolIntake::WriteMemory(_)
        | ToolIntake::ReadMemoryEntry { .. }
        | ToolIntake::SearchMemory { .. }
        | ToolIntake::RecordMemoryObservations(_)
        | ToolIntake::ApplyMemoryOperations(_)
        | ToolIntake::RecordCharacterIntent { .. } => None,
    }
}

/// The result of a Host ToolCall with the text `text`.
pub fn host_tool_result(text: &str) -> ToolResultText {
    ToolResultText {
        text: text.to_owned(),
        digest: format!(
            "sha256:{}",
            crate::project_archive::hex_sha256(text.as_bytes())
        ),
        bytes: text.len(),
    }
}
