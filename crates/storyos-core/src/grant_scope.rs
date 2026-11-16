//! The standing scope of a Project Destination Grant (ADR 0051).

use crate::assemble_context::ContextPurpose;
use crate::{ContextSourceClass, CurrentPassageAssemblyRecord};

/// The versioned scope that a Project Destination Grant records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrantScope {
    /// All current content of the Project for writing assistance on the grant destination.
    ProjectWritingAssistanceV1,
    /// The first scope, the active compaction of the current Conversation, and the Host-managed
    /// summary of the same Conversation.
    ProjectWritingAssistanceV2,
    /// The second scope and the source class `tool_result`: the projected results of the
    /// resolved ToolCalls of the Run for its continuation RunStep (S5-05).
    ProjectWritingAssistanceV3,
    /// The third scope and the Skill source classes `skill_catalog`, `skill_instruction`, and
    /// `skill_resource`: the text of the Skills that the operator of the deployment provides
    /// (ADR 0089 section 4).
    ProjectWritingAssistanceV4,
    /// The fourth scope, the Stage 7 source classes of the manuscript and of Agent Memory, and
    /// the Purposes `memory_capture` and `memory_consolidation` (ADR 0094, ADR 0095).
    ProjectWritingAssistanceV5,
    /// The research destination that `createAgentRun` binds (ADR 0090): the Purpose `research`,
    /// the source classes `agent_query` and `agent_url_selection`, no length bound, no Ambient
    /// Context. It covers no model request.
    ResearchToolV1,
}

impl GrantScope {
    /// The scope that each new grant records.
    pub const CURRENT: Self = Self::ProjectWritingAssistanceV5;

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProjectWritingAssistanceV1 => "project_writing_assistance_v1",
            Self::ProjectWritingAssistanceV2 => "project_writing_assistance_v2",
            Self::ProjectWritingAssistanceV3 => "project_writing_assistance_v3",
            Self::ProjectWritingAssistanceV4 => "project_writing_assistance_v4",
            Self::ProjectWritingAssistanceV5 => "project_writing_assistance_v5",
            Self::ResearchToolV1 => "research_tool_v1",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "project_writing_assistance_v1" => Some(Self::ProjectWritingAssistanceV1),
            "project_writing_assistance_v2" => Some(Self::ProjectWritingAssistanceV2),
            "project_writing_assistance_v3" => Some(Self::ProjectWritingAssistanceV3),
            "project_writing_assistance_v4" => Some(Self::ProjectWritingAssistanceV4),
            "project_writing_assistance_v5" => Some(Self::ProjectWritingAssistanceV5),
            "research_tool_v1" => Some(Self::ResearchToolV1),
            _ => None,
        }
    }

    /// The revision of a writing assistance scope. Each revision names each Purpose and each
    /// source class of the revision before it. The research scope names no model request.
    fn writing_revision(self) -> Option<u8> {
        match self {
            Self::ProjectWritingAssistanceV1 => Some(1),
            Self::ProjectWritingAssistanceV2 => Some(2),
            Self::ProjectWritingAssistanceV3 => Some(3),
            Self::ProjectWritingAssistanceV4 => Some(4),
            Self::ProjectWritingAssistanceV5 => Some(5),
            Self::ResearchToolV1 => None,
        }
    }

    fn names_purpose(self, purpose: ContextPurpose) -> bool {
        let first = match purpose {
            ContextPurpose::CurrentPassageAssistance => 1,
            ContextPurpose::ActiveContextCompaction => 2,
            ContextPurpose::MemoryCapture | ContextPurpose::MemoryConsolidation => 5,
        };
        self.writing_revision()
            .is_some_and(|revision| revision >= first)
    }

    fn names_source_class(self, class: ContextSourceClass) -> bool {
        let first = match class {
            ContextSourceClass::HostControl => return false,
            ContextSourceClass::AuthorInstruction
            | ContextSourceClass::WorkingTarget
            | ContextSourceClass::InstructionBinding => 1,
            ContextSourceClass::ConversationSummary => 2,
            ContextSourceClass::ToolResult => 3,
            ContextSourceClass::SkillCatalog
            | ContextSourceClass::SkillInstruction
            | ContextSourceClass::SkillResource => 4,
            ContextSourceClass::ChapterList
            | ContextSourceClass::ManuscriptReadResult
            | ContextSourceClass::ManuscriptSearchResult
            | ContextSourceClass::MemoryIndex
            | ContextSourceClass::MemoryReadResult
            | ContextSourceClass::MemorySearchResult
            | ContextSourceClass::MemoryCaptureInput
            | ContextSourceClass::MemoryDeleteMarks
            | ContextSourceClass::MemoryEntries
            | ContextSourceClass::MemoryObservations => 5,
        };
        self.writing_revision()
            .is_some_and(|revision| revision >= first)
    }
}

/// The grant that an AgentRun pinned through its use binding, and the request that it discloses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrantFacts<'a> {
    /// The Project Scope of the claimed AgentRun.
    pub owner_user_id: &'a str,
    pub project_id: &'a str,
    /// The recorded scope of the pinned grant, or `None` for a grant without a scope.
    pub grant_scope: Option<&'a str>,
    /// The Processing Destination Identity of the pinned grant.
    pub grant_identity: Option<&'a str>,
    /// The Processing Destination Identity of the pinned use binding.
    pub binding_identity: Option<&'a str>,
    /// The Context Assembly record of the request, or `None` when it does not decode.
    pub request: Option<&'a CurrentPassageAssemblyRecord>,
}

/// Admits a disclosure that the scope of the pinned grant covers, or refuses it with
/// `disclosure_outside_grant`.
pub fn admit_disclosure(facts: &GrantFacts<'_>) -> Result<GrantScope, &'static str> {
    const OUTSIDE: &str = "disclosure_outside_grant";
    let scope = facts
        .grant_scope
        .and_then(GrantScope::parse)
        .ok_or(OUTSIDE)?;
    let request = facts.request.ok_or(OUTSIDE)?;
    let requirement = &request.operation_requirement;
    let identity = facts.grant_identity.ok_or(OUTSIDE)?;
    let covered = requirement.owner_user_id == facts.owner_user_id
        && requirement.project_id == facts.project_id
        && requirement.destination_identity == identity
        && facts.binding_identity == Some(identity)
        && scope.names_purpose(requirement.purpose)
        && request
            .selected
            .iter()
            .all(|item| scope.names_source_class(item.source_class));
    if covered { Ok(scope) } else { Err(OUTSIDE) }
}

#[cfg(test)]
#[path = "grant_scope_tests.rs"]
mod tests;
