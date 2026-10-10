//! Global Model Registrations and their versioned Model Capability Profiles (ADR 0033).

use serde::Serialize;

/// The Model Provider Adapter that a Model Registration binds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelAdapter {
    HostFake,
    VolcengineAgentPlanResponses,
}

impl ModelAdapter {
    /// The stored `model_kind` of the Registrations that bind this adapter.
    pub fn kind(self) -> &'static str {
        match self {
            Self::HostFake => "host_fake",
            Self::VolcengineAgentPlanResponses => "volcengine_agent_plan_responses",
        }
    }
}

/// One host-owned global Model Registration revision. It holds no Project data, endpoint
/// account, Credential Reference, grant, or compatibility admission.
#[derive(Debug, Eq, PartialEq)]
pub struct ModelRegistration {
    pub revision: &'static str,
    pub adapter: ModelAdapter,
    pub api_surface: &'static str,
    /// The exact provider model identifier. It is never an alias such as "latest".
    pub provider_model_id: &'static str,
    pub capability_profile: &'static ModelCapabilityProfile,
}

/// One capability of a Model Capability Profile. Each capability has its own evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    Transport,
    NativeText,
    StructuredText,
    Continuation,
    Streaming,
    ImplicitCache,
    ExplicitCache,
    NativeCompaction,
    HostCompaction,
    Retrieval,
    Abort,
}

/// The evidence of one capability or one combination. Documentation is not qualification.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CapabilityEvidence {
    Unknown,
    /// A public document states the behavior. It does not prove the exact route and model.
    Documented {
        source: &'static str,
    },
    /// Attributable evidence from the exact route and model.
    Qualified {
        evidence: &'static str,
    },
}

const fn documented(source: &'static str) -> CapabilityEvidence {
    CapabilityEvidence::Documented { source }
}

impl CapabilityEvidence {
    fn qualified(self) -> bool {
        matches!(self, Self::Qualified { .. })
    }
}

/// The evidence of each capability. A field for each capability keeps the profile complete.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct CapabilityEntries {
    pub transport: CapabilityEvidence,
    pub native_text: CapabilityEvidence,
    pub structured_text: CapabilityEvidence,
    pub continuation: CapabilityEvidence,
    pub streaming: CapabilityEvidence,
    pub implicit_cache: CapabilityEvidence,
    pub explicit_cache: CapabilityEvidence,
    pub native_compaction: CapabilityEvidence,
    pub host_compaction: CapabilityEvidence,
    pub retrieval: CapabilityEvidence,
    pub abort: CapabilityEvidence,
}

/// The evidence that a set of capabilities works together. Members alone prove no combination.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct CapabilityCombination {
    pub members: &'static [ModelCapability],
    pub evidence: CapabilityEvidence,
}

/// The immutable, versioned, provider-neutral semantic envelope of one Model Registration.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct ModelCapabilityProfile {
    pub revision: &'static str,
    pub entries: CapabilityEntries,
    pub combinations: &'static [CapabilityCombination],
}

/// The capabilities that one Create request needs together.
pub const CREATE_REQUIREMENT: &[ModelCapability] =
    &[ModelCapability::Transport, ModelCapability::NativeText];

impl ModelCapabilityProfile {
    pub fn evidence(&self, capability: ModelCapability) -> CapabilityEvidence {
        let entries = &self.entries;
        match capability {
            ModelCapability::Transport => entries.transport,
            ModelCapability::NativeText => entries.native_text,
            ModelCapability::StructuredText => entries.structured_text,
            ModelCapability::Continuation => entries.continuation,
            ModelCapability::Streaming => entries.streaming,
            ModelCapability::ImplicitCache => entries.implicit_cache,
            ModelCapability::ExplicitCache => entries.explicit_cache,
            ModelCapability::NativeCompaction => entries.native_compaction,
            ModelCapability::HostCompaction => entries.host_compaction,
            ModelCapability::Retrieval => entries.retrieval,
            ModelCapability::Abort => entries.abort,
        }
    }

    /// True only when each required capability and one combination that holds all of them
    /// have qualified evidence.
    pub fn qualifies(&self, required: &[ModelCapability]) -> bool {
        required
            .iter()
            .all(|capability| self.evidence(*capability).qualified())
            && self.combinations.iter().any(|combination| {
                combination.evidence.qualified()
                    && required
                        .iter()
                        .all(|capability| combination.members.contains(capability))
            })
    }
}

const STAGE_3_PROOF: CapabilityEvidence = CapabilityEvidence::Qualified {
    evidence: "Stage 3 contract proof of the Contract-Faithful Fake Destination",
};

pub const HOST_FAKE_CAPABILITY_PROFILE: ModelCapabilityProfile = ModelCapabilityProfile {
    revision: "storyos.host-fake.capability.v1",
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
    },
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

const AGENT_PLAN_OVERVIEW: &str = "Agent Plan overview, docs.volcengine.com/docs/82379/2366394, \
     updated 2026-10-08: doubao-seed-2.1-pro is an Agent Plan text model";
const AGENT_PLAN_RESPONSES_GUIDE: &str =
    "Agent Plan Codex guide, docs.volcengine.com/docs/82379/2556054: Responses at /api/plan/v3";
const ARK_CREATE_RESPONSE: &str = "General Ark Create Response reference, docs.volcengine.com/docs/82379/1569618, not the \
     Agent Plan route";
const ARK_RESPONSE_LIFECYCLE: &str = "General Ark response lifecycle, docs.volcengine.com/docs/82379/2644693, not the Agent Plan \
     route";
const ARK_CONTEXT_CACHE: &str = "General Ark context cache, docs.volcengine.com/docs/82379/1398933, \
     not the Agent Plan route";
const ARK_RETRIEVE_RESPONSE: &str =
    "General Ark Query Response, docs.volcengine.com/docs/82379/1783709, not the Agent Plan route";

/// Each runtime entry is pending. S4-02 owns the first evidence-backed qualification.
pub const AGENT_PLAN_CAPABILITY_PROFILE: ModelCapabilityProfile = ModelCapabilityProfile {
    revision: "storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v1",
    entries: CapabilityEntries {
        transport: documented(AGENT_PLAN_RESPONSES_GUIDE),
        native_text: documented(AGENT_PLAN_OVERVIEW),
        structured_text: documented(ARK_CREATE_RESPONSE),
        continuation: documented(ARK_CREATE_RESPONSE),
        streaming: documented(ARK_RESPONSE_LIFECYCLE),
        implicit_cache: documented(ARK_CONTEXT_CACHE),
        explicit_cache: documented(ARK_CONTEXT_CACHE),
        native_compaction: CapabilityEvidence::Unknown,
        host_compaction: CapabilityEvidence::Unknown,
        retrieval: documented(ARK_RETRIEVE_RESPONSE),
        abort: CapabilityEvidence::Unknown,
    },
    combinations: &[
        CapabilityCombination {
            members: CREATE_REQUIREMENT,
            evidence: CapabilityEvidence::Unknown,
        },
        CapabilityCombination {
            members: &[
                ModelCapability::Continuation,
                ModelCapability::ExplicitCache,
            ],
            evidence: documented(ARK_CONTEXT_CACHE),
        },
        CapabilityCombination {
            members: &[
                ModelCapability::ExplicitCache,
                ModelCapability::StructuredText,
            ],
            evidence: documented(ARK_CONTEXT_CACHE),
        },
    ],
};

pub const HOST_FAKE_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa01",
    adapter: ModelAdapter::HostFake,
    api_surface: "storyos.host-fake",
    provider_model_id: "host-fake",
    capability_profile: &HOST_FAKE_CAPABILITY_PROFILE,
};

pub const AGENT_PLAN_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa02",
    adapter: ModelAdapter::VolcengineAgentPlanResponses,
    api_surface: "volcengine.agent-plan.responses./api/plan/v3",
    provider_model_id: "doubao-seed-2.1-pro",
    capability_profile: &AGENT_PLAN_CAPABILITY_PROFILE,
};

/// The known Registration of a stored revision. `None` is an unknown or damaged revision.
pub fn model_registration(revision: &str) -> Option<&'static ModelRegistration> {
    [&HOST_FAKE_REGISTRATION, &AGENT_PLAN_REGISTRATION]
        .into_iter()
        .find(|registration| registration.revision == revision)
}

#[cfg(test)]
#[path = "model_registration_tests.rs"]
mod tests;
