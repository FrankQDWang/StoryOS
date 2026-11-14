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

/// The current Agent Plan Registration: Registration `fa19` with the mapping `v6`. Nothing is
/// pending, so the link authorizes no probe.
pub const AGENT_PLAN_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa20",
    mapping_revision: AGENT_PLAN_READ_RESULT_MAPPING_REVISION,
    ..AGENT_PLAN_TOOL_LOOP_REGISTRATION
};
