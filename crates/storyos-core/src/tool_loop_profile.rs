//! The tool loop profile of ADR 0091: the current Model Registrations, whose request mapping
//! declares the Host Tools. Only these Registrations bind a new Run (ADR 0097).

use crate::{
    COMPACTION_REQUIREMENT, CapabilityCombination, CapabilityEntries, CapabilityEvidence,
    ModelAdapter, ModelCapability, ModelCapabilityProfile, ModelRegistration, RECOVERY_REQUIREMENT,
};

/// The fake model mapping of the tool loop profile. Its script follows the finish rule.
pub const HOST_FAKE_TOOL_LOOP_MAPPING_REVISION: &str = "storyos.host-fake.mapping.v2";

/// The current Agent Plan mapping: the tool loop mapping with the Host instructions of the Tools
/// and the finish rule, strict functions, and the `tool_choice` of the finish rule (ADR 0091),
/// the stored Tool results (ADR 0093), the Skill sections (ADR 0089 sections 3 and 4), the
/// manuscript and Memory items and Tools (ADR 0094, ADR 0095), the Host instructions of the
/// Memory rules of ADR 0094 sections 5 and 6, the default writing rules for prose of each
/// tool-loop request (ADR 0100 section 2), and the instructions of the two parts of a character
/// file (ADR 0101 section 3).
pub const AGENT_PLAN_CHARACTER_FILE_MAPPING_REVISION: &str =
    "storyos.volcengine-agent-plan.responses.create.v11";

const STAGE_3_PROOF: CapabilityEvidence = CapabilityEvidence::Qualified {
    evidence: "Stage 3 contract proof of the Contract-Faithful Fake Destination",
};

pub const HOST_FAKE_TOOL_LOOP_CAPABILITY_PROFILE: ModelCapabilityProfile = ModelCapabilityProfile {
    revision: "storyos.host-fake.capability.v2",
    entries: CapabilityEntries {
        transport: STAGE_3_PROOF,
        native_text: STAGE_3_PROOF,
        structured_text: CapabilityEvidence::Unknown,
        continuation: STAGE_3_PROOF,
        streaming: STAGE_3_PROOF,
        implicit_cache: CapabilityEvidence::Unknown,
        explicit_cache: CapabilityEvidence::Unknown,
        native_compaction: CapabilityEvidence::Unknown,
        host_compaction: STAGE_3_PROOF,
        retrieval: STAGE_3_PROOF,
        abort: STAGE_3_PROOF,
        function_calling: CapabilityEvidence::Qualified {
            evidence: "Contract proof of the Contract-Faithful Fake Destination (ADR 0091)",
        },
    },
    context_window: None,
    combinations: &[CapabilityCombination {
        members: &[
            ModelCapability::Transport,
            ModelCapability::NativeText,
            ModelCapability::Continuation,
            ModelCapability::Streaming,
            ModelCapability::HostCompaction,
            ModelCapability::Retrieval,
            ModelCapability::Abort,
        ],
        evidence: STAGE_3_PROOF,
    }],
};

/// The fake model Registration of the Host fake destination and of the journeys.
pub const HOST_FAKE_TOOL_LOOP_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa18",
    adapter: ModelAdapter::HostFake,
    api_surface: "storyos.host-fake",
    provider_model_id: "host-fake",
    mapping_revision: HOST_FAKE_TOOL_LOOP_MAPPING_REVISION,
    requirement: COMPACTION_REQUIREMENT,
    capability_profile: &HOST_FAKE_TOOL_LOOP_CAPABILITY_PROFILE,
};

/// The current Agent Plan Registration: the qualified dated version `doubao-seed-2-1-pro-260915`
/// with qualified function calling and the mapping `v11`. Nothing is pending, so the link
/// authorizes no probe. A later provider version is a new Registration that the author approves
/// (ADR 0059).
pub const AGENT_PLAN_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa25",
    adapter: ModelAdapter::VolcengineAgentPlanResponses,
    api_surface: "volcengine.agent-plan.responses./api/plan/v3",
    provider_model_id: "doubao-seed-2-1-pro-260915",
    mapping_revision: AGENT_PLAN_CHARACTER_FILE_MAPPING_REVISION,
    requirement: RECOVERY_REQUIREMENT,
    capability_profile: &crate::AGENT_PLAN_CAPABILITY_PROFILE,
};
