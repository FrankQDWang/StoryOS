//! The Tool entries of a Model Capability Profile and their author-authorized qualification
//! probes on the Agent Plan route (ADR 0049).

use serde::Serialize;

use crate::{
    AGENT_PLAN_CAPABILITY_PROFILE, AGENT_PLAN_REGISTRATION, CapabilityEntries, CapabilityEvidence,
    ModelCapabilityProfile, ModelRegistration, RuntimeQualification, model_registration,
};

/// One Tool capability of a Model Capability Profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolCapability {
    FunctionCalling,
    HostedWebSearch,
    RemoteMcp,
    MaxToolCallsBound,
}

/// The evidence of each Tool capability. A field for each capability keeps the entries complete.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ToolEntries {
    pub function_calling: CapabilityEvidence,
    pub hosted_web_search: CapabilityEvidence,
    pub remote_mcp: CapabilityEvidence,
    pub max_tool_calls_bound: CapabilityEvidence,
}

/// The Tool entries of a profile that records no Tool evidence.
pub const UNKNOWN_TOOL_ENTRIES: ToolEntries = ToolEntries {
    function_calling: CapabilityEvidence::Unknown,
    hosted_web_search: CapabilityEvidence::Unknown,
    remote_mcp: CapabilityEvidence::Unknown,
    max_tool_calls_bound: CapabilityEvidence::Unknown,
};

impl ToolEntries {
    pub fn evidence(&self, capability: ToolCapability) -> CapabilityEvidence {
        match capability {
            ToolCapability::FunctionCalling => self.function_calling,
            ToolCapability::HostedWebSearch => self.hosted_web_search,
            ToolCapability::RemoteMcp => self.remote_mcp,
            ToolCapability::MaxToolCallsBound => self.max_tool_calls_bound,
        }
    }

    /// True when no entry has evidence. Such a profile does not serialize its Tool entries.
    pub fn unknown(&self) -> bool {
        *self == UNKNOWN_TOOL_ENTRIES
    }

    /// True when documentation is the only evidence of the entry. Only a pending entry has a
    /// qualification probe.
    pub fn pending(&self, capability: ToolCapability) -> bool {
        matches!(
            self.evidence(capability),
            CapabilityEvidence::Documented { .. }
        )
    }

    pub fn any_pending(&self) -> bool {
        [
            ToolCapability::FunctionCalling,
            ToolCapability::HostedWebSearch,
            ToolCapability::RemoteMcp,
            ToolCapability::MaxToolCallsBound,
        ]
        .into_iter()
        .any(|capability| self.pending(capability))
    }
}

/// One qualification request of a pending Tool entry. The author message is its constant probe
/// input, and the request sends no Project content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolProbe {
    /// Asks for one call of the probe function `storyos_probe_echo`.
    FunctionCall,
    /// Continues the response of the latest function call probe with the output of that call.
    FunctionCallOutput,
    HostedWebSearch,
    RemoteMcp,
    /// A hosted web search task that needs two searches, with `max_tool_calls` 1.
    MaxToolCallsBound,
}

impl ToolProbe {
    const ALL: [Self; 5] = [
        Self::FunctionCall,
        Self::FunctionCallOutput,
        Self::HostedWebSearch,
        Self::RemoteMcp,
        Self::MaxToolCallsBound,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::FunctionCall => "function_call",
            Self::FunctionCallOutput => "function_call_output",
            Self::HostedWebSearch => "hosted_web_search",
            Self::RemoteMcp => "remote_mcp",
            Self::MaxToolCallsBound => "max_tool_calls_bound",
        }
    }

    /// The exact author message that asks for this probe.
    pub fn instruction(self) -> &'static str {
        match self {
            Self::FunctionCall => {
                "StoryOS Tool probe: call the function storyos_probe_echo one time with the text \
                 storyos-probe."
            }
            Self::FunctionCallOutput => {
                "StoryOS Tool probe: send the output of the storyos_probe_echo call."
            }
            Self::HostedWebSearch => {
                "StoryOS Tool probe: search the web for the release date of Rust 1.0, and cite \
                 the source."
            }
            Self::RemoteMcp => {
                "StoryOS Tool probe: use the read_wiki_structure tool for the repository \
                 volcengine/ai-app-lab, and give the first topic."
            }
            Self::MaxToolCallsBound => {
                "StoryOS Tool probe: search the web two times, first for the release date of \
                 Rust 1.0 and then for the release date of Python 3.0, and give the two dates."
            }
        }
    }

    pub fn capability(self) -> ToolCapability {
        match self {
            Self::FunctionCall | Self::FunctionCallOutput => ToolCapability::FunctionCalling,
            Self::HostedWebSearch => ToolCapability::HostedWebSearch,
            Self::RemoteMcp => ToolCapability::RemoteMcp,
            Self::MaxToolCallsBound => ToolCapability::MaxToolCallsBound,
        }
    }
}

/// The probe that an author message asks for, only when the pinned Registration has that Tool
/// entry pending and the compatibility Decision is `qualification_authorized`. Otherwise the
/// message is an ordinary request.
pub fn tool_probe(
    author_message: &str,
    model_registration_revision: &str,
    qualification: RuntimeQualification,
) -> Option<ToolProbe> {
    let tools = model_registration(model_registration_revision)?
        .capability_profile
        .entries
        .tools;
    ToolProbe::ALL.into_iter().find(|probe| {
        probe.instruction() == author_message
            && qualification == RuntimeQualification::QualificationAuthorized
            && tools.pending(probe.capability())
    })
}

const ARK_TOOL_CALLING: &str =
    "General Ark tool calling, docs.volcengine.com/docs/82379/1958524, not the Agent Plan route";
const ARK_WEB_SEARCH: &str = "General Ark web search tool, docs.volcengine.com/docs/82379/1756990, \
     read 2026-11-03: web_search with sources [\"doubao\"], url_citation, usage.tool_usage, \
     max_tool_calls 1 to 10; not the Agent Plan route";
const ARK_REMOTE_MCP: &str = "General Ark remote MCP, docs.volcengine.com/docs/82379/1827534, read \
     2026-11-03: type mcp over Streamable HTTP with the header ark-beta-mcp; not the Agent Plan \
     route";

/// The Tool entries before the qualification probes. Each entry has only documentation.
const AGENT_PLAN_PENDING_TOOL_ENTRIES: ToolEntries = ToolEntries {
    function_calling: CapabilityEvidence::Documented {
        source: ARK_TOOL_CALLING,
    },
    hosted_web_search: CapabilityEvidence::Documented {
        source: ARK_WEB_SEARCH,
    },
    remote_mcp: CapabilityEvidence::Documented {
        source: ARK_REMOTE_MCP,
    },
    max_tool_calls_bound: CapabilityEvidence::Documented {
        source: ARK_WEB_SEARCH,
    },
};

/// The profile of the dated version with its four Tool entries pending. Each other entry and
/// each combination keep their qualification.
pub const AGENT_PLAN_TOOL_PENDING_CAPABILITY_PROFILE: ModelCapabilityProfile =
    ModelCapabilityProfile {
        revision: "storyos.volcengine-agent-plan.doubao-seed-2-1-pro-260915.capability.v3",
        entries: CapabilityEntries {
            tools: AGENT_PLAN_PENDING_TOOL_ENTRIES,
            ..AGENT_PLAN_CAPABILITY_PROFILE.entries
        },
        context_window: AGENT_PLAN_CAPABILITY_PROFILE.context_window,
        combinations: AGENT_PLAN_CAPABILITY_PROFILE.combinations,
    };

/// The current Agent Plan Registration: the qualified dated version with its Tool entries
/// pending. A Decision with the authorization link dispatches their probes (ADR 0049).
pub const AGENT_PLAN_TOOL_PENDING_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa16",
    capability_profile: &AGENT_PLAN_TOOL_PENDING_CAPABILITY_PROFILE,
    ..AGENT_PLAN_REGISTRATION
};

#[cfg(test)]
#[path = "tool_capability_tests.rs"]
mod tests;
