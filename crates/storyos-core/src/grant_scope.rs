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
    /// The research destination that `createAgentRun` binds (ADR 0090): the Purpose `research`,
    /// the source classes `agent_query` and `agent_url_selection`, no length bound, no Ambient
    /// Context. It covers no model request.
    ResearchToolV1,
}

impl GrantScope {
    /// The scope that each new grant records.
    pub const CURRENT: Self = Self::ProjectWritingAssistanceV3;

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProjectWritingAssistanceV1 => "project_writing_assistance_v1",
            Self::ProjectWritingAssistanceV2 => "project_writing_assistance_v2",
            Self::ProjectWritingAssistanceV3 => "project_writing_assistance_v3",
            Self::ResearchToolV1 => "research_tool_v1",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "project_writing_assistance_v1" => Some(Self::ProjectWritingAssistanceV1),
            "project_writing_assistance_v2" => Some(Self::ProjectWritingAssistanceV2),
            "project_writing_assistance_v3" => Some(Self::ProjectWritingAssistanceV3),
            "research_tool_v1" => Some(Self::ResearchToolV1),
            _ => None,
        }
    }

    fn names_purpose(self, purpose: ContextPurpose) -> bool {
        match (self, purpose) {
            (
                Self::ProjectWritingAssistanceV1
                | Self::ProjectWritingAssistanceV2
                | Self::ProjectWritingAssistanceV3,
                ContextPurpose::CurrentPassageAssistance,
            )
            | (
                Self::ProjectWritingAssistanceV2 | Self::ProjectWritingAssistanceV3,
                ContextPurpose::ActiveContextCompaction,
            ) => true,
            (Self::ProjectWritingAssistanceV1, ContextPurpose::ActiveContextCompaction)
            | (
                Self::ResearchToolV1,
                ContextPurpose::CurrentPassageAssistance | ContextPurpose::ActiveContextCompaction,
            ) => false,
        }
    }

    fn names_source_class(self, class: ContextSourceClass) -> bool {
        match (self, class) {
            (
                Self::ProjectWritingAssistanceV1
                | Self::ProjectWritingAssistanceV2
                | Self::ProjectWritingAssistanceV3,
                ContextSourceClass::AuthorInstruction
                | ContextSourceClass::WorkingTarget
                | ContextSourceClass::InstructionBinding,
            )
            | (
                Self::ProjectWritingAssistanceV2 | Self::ProjectWritingAssistanceV3,
                ContextSourceClass::ConversationSummary,
            )
            | (Self::ProjectWritingAssistanceV3, ContextSourceClass::ToolResult) => true,
            (
                Self::ProjectWritingAssistanceV1
                | Self::ProjectWritingAssistanceV2
                | Self::ProjectWritingAssistanceV3,
                ContextSourceClass::HostControl,
            )
            | (Self::ProjectWritingAssistanceV1, ContextSourceClass::ConversationSummary)
            | (
                Self::ProjectWritingAssistanceV1 | Self::ProjectWritingAssistanceV2,
                ContextSourceClass::ToolResult,
            )
            | (
                Self::ResearchToolV1,
                ContextSourceClass::AuthorInstruction
                | ContextSourceClass::WorkingTarget
                | ContextSourceClass::InstructionBinding
                | ContextSourceClass::ConversationSummary
                | ContextSourceClass::HostControl
                | ContextSourceClass::ToolResult,
            ) => false,
        }
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
