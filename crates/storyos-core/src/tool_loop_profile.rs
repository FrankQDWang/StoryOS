//! The tool loop profile of ADR 0091: the Model Registrations whose request mapping declares the
//! Host Tools. The adapters send a decision request only with these mappings. The earlier
//! Registrations stay only as the identities of recorded Attempts.

use crate::{
    AGENT_PLAN_FUNCTION_CALLING_REGISTRATION, CapabilityEntries, CapabilityEvidence,
    HOST_FAKE_CAPABILITY_PROFILE, HOST_FAKE_REGISTRATION, ModelCapabilityProfile,
    ModelRegistration,
};

/// The fake model mapping of the tool loop profile. Its script follows the finish rule.
pub const HOST_FAKE_TOOL_LOOP_MAPPING_REVISION: &str = "storyos.host-fake.mapping.v2";

/// The Agent Plan mapping of the tool loop profile: the compaction mapping with Host
/// instructions that describe the Tools and the finish rule, strict functions, and the
/// `tool_choice` of the finish rule.
pub const AGENT_PLAN_TOOL_LOOP_MAPPING_REVISION: &str =
    "storyos.volcengine-agent-plan.responses.create.v5";

/// The tool loop mapping with the fourth Host Tool `read_tool_result` (ADR 0093).
pub const AGENT_PLAN_READ_RESULT_MAPPING_REVISION: &str =
    "storyos.volcengine-agent-plan.responses.create.v6";

/// The tool loop mapping of Stage 6: the mapping `v6` with the Skill section of a request and
/// the Skill paragraph of the Host instructions (ADR 0089 sections 3 and 4).
pub const AGENT_PLAN_SKILL_MAPPING_REVISION: &str =
    "storyos.volcengine-agent-plan.responses.create.v7";

/// The tool loop mapping of Stage 7: the mapping `v7` with the manuscript and Memory items and
/// Tools that the Stage 7 tickets add (ADR 0094, ADR 0095).
pub const AGENT_PLAN_CONTINUITY_MAPPING_REVISION: &str =
    "storyos.volcengine-agent-plan.responses.create.v8";

pub const HOST_FAKE_TOOL_LOOP_CAPABILITY_PROFILE: ModelCapabilityProfile = ModelCapabilityProfile {
    revision: "storyos.host-fake.capability.v2",
    entries: CapabilityEntries {
        function_calling: CapabilityEvidence::Qualified {
            evidence: "Contract proof of the Contract-Faithful Fake Destination (ADR 0091)",
        },
        ..HOST_FAKE_CAPABILITY_PROFILE.entries
    },
    ..HOST_FAKE_CAPABILITY_PROFILE
};

/// The second fake model profile.
pub const HOST_FAKE_TOOL_LOOP_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa18",
    mapping_revision: HOST_FAKE_TOOL_LOOP_MAPPING_REVISION,
    capability_profile: &HOST_FAKE_TOOL_LOOP_CAPABILITY_PROFILE,
    ..HOST_FAKE_REGISTRATION
};

/// The qualified dated version with qualified function calling and the tool loop mapping `v5`.
/// An AgentRun that pinned it keeps the mapping `v5` and the first three Host Tools.
pub const AGENT_PLAN_TOOL_LOOP_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa19",
    mapping_revision: AGENT_PLAN_TOOL_LOOP_MAPPING_REVISION,
    ..AGENT_PLAN_FUNCTION_CALLING_REGISTRATION
};

/// Registration `fa19` with the mapping `v6`. An AgentRun that pinned it keeps the mapping
/// `v6`.
pub const AGENT_PLAN_READ_RESULT_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa20",
    mapping_revision: AGENT_PLAN_READ_RESULT_MAPPING_REVISION,
    ..AGENT_PLAN_TOOL_LOOP_REGISTRATION
};

/// Registration `fa20` with the mapping `v7`. An AgentRun that pinned it keeps the mapping
/// `v7`.
pub const AGENT_PLAN_SKILL_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa21",
    mapping_revision: AGENT_PLAN_SKILL_MAPPING_REVISION,
    ..AGENT_PLAN_READ_RESULT_REGISTRATION
};

/// The current Agent Plan Registration: Registration `fa21` with the mapping `v8`. Nothing is
/// pending, so the link authorizes no probe.
pub const AGENT_PLAN_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa22",
    mapping_revision: AGENT_PLAN_CONTINUITY_MAPPING_REVISION,
    ..AGENT_PLAN_SKILL_REGISTRATION
};
