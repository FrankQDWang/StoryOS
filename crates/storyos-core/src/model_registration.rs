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

    pub fn parse(kind: &str) -> Option<Self> {
        [Self::HostFake, Self::VolcengineAgentPlanResponses]
            .into_iter()
            .find(|adapter| adapter.kind() == kind)
    }
}

/// One host-owned global Model Registration revision. It holds no Project data, endpoint
/// account, Credential Reference, grant, or compatibility admission.
#[derive(Debug, Eq, PartialEq)]
pub struct ModelRegistration {
    pub revision: &'static str,
    pub adapter: ModelAdapter,
    pub api_surface: &'static str,
    /// The exact provider model identifier. When the provider publishes dated versions, it is a
    /// dated version identifier, never an undated name or an alias such as "latest" (ADR 0059).
    pub provider_model_id: &'static str,
    /// The exact wire mapping revision of the adapter.
    pub mapping_revision: &'static str,
    /// The capabilities that the mapping uses in each Create request.
    pub requirement: &'static [ModelCapability],
    pub capability_profile: &'static ModelCapabilityProfile,
}

impl ModelRegistration {
    /// True when the profile qualifies each capability that the mapping uses, together.
    pub fn qualified(&self) -> bool {
        self.capability_profile.qualifies(self.requirement)
    }

    /// Whether the mapping can send the prior response reference of the Conversation.
    pub fn continues(&self) -> bool {
        self.requirement.contains(&ModelCapability::Continuation)
    }

    /// Whether the mapping can replace the prior response with a Host-managed summary.
    pub fn compacts(&self) -> bool {
        self.requirement.contains(&ModelCapability::HostCompaction)
    }

    /// Whether the Host may read a stored response of an unknown create. Either the profile
    /// qualifies retrieval, or an authorized runtime qualification examines a route that needs it.
    pub fn retrieves(&self, qualification: crate::RuntimeQualification) -> bool {
        retrieval_admitted(
            self.capability_profile
                .qualifies(&[ModelCapability::Retrieval]),
            self.requirement.contains(&ModelCapability::Retrieval),
            qualification,
        )
    }
}

/// The retrieval rule of `ModelRegistration::retrieves`: the profile qualifies retrieval, or an
/// authorized qualification examines a requirement that has retrieval. No Registration of the one
/// list needs the second branch now, but a later provider version can (ADR 0097).
fn retrieval_admitted(
    qualified: bool,
    required: bool,
    qualification: crate::RuntimeQualification,
) -> bool {
    qualified || (qualification == crate::RuntimeQualification::QualificationAuthorized && required)
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

impl CapabilityEvidence {
    fn qualified(self) -> bool {
        matches!(self, Self::Qualified { .. })
    }

    fn unknown(&self) -> bool {
        *self == Self::Unknown
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
    /// A profile without function calling evidence keeps its stored form.
    #[serde(skip_serializing_if = "CapabilityEvidence::unknown")]
    pub function_calling: CapabilityEvidence,
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
    /// The context window of the model, which input and output share, or `None` when no
    /// evidence states it. A profile with no window does not compact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<ContextWindow>,
}

/// The context window of one model, in tokens, with its evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ContextWindow {
    pub tokens: u64,
    pub evidence: CapabilityEvidence,
}

/// The capabilities that the compaction requests and the retrieval of the stored response of an
/// unknown create need together.
pub const RECOVERY_REQUIREMENT: &[ModelCapability] = &[
    ModelCapability::Transport,
    ModelCapability::NativeText,
    ModelCapability::Continuation,
    ModelCapability::Streaming,
    ModelCapability::HostCompaction,
    ModelCapability::Retrieval,
];

/// The capabilities that a streamed request with a prior response reference, a Host-managed
/// summary request, and the later request with that summary need together.
pub const COMPACTION_REQUIREMENT: &[ModelCapability] = &[
    ModelCapability::Transport,
    ModelCapability::NativeText,
    ModelCapability::Continuation,
    ModelCapability::Streaming,
    ModelCapability::HostCompaction,
];

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

const fn documented(source: &'static str) -> CapabilityEvidence {
    CapabilityEvidence::Documented { source }
}

const ARK_CREATE_RESPONSE: &str = "General Ark Create Response reference, docs.volcengine.com/docs/82379/1569618, not the \
     Agent Plan route";
const ARK_CONTEXT_CACHE: &str = "General Ark context cache, docs.volcengine.com/docs/82379/1398933, \
     not the Agent Plan route";
const ARK_MODEL_LIST_260915: &str = "Ark model list, docs.volcengine.com/docs/82379/1799865, \
     updated 2026-10-09: doubao-seed-2-1-pro-260915 has a 1024K context window, a 1024K input, and \
     a 256K answer";

/// The documented context window of `doubao-seed-2-1-pro-260915`: 1024K, 1,048,576 tokens.
const AGENT_PLAN_WINDOW: ContextWindow = ContextWindow {
    tokens: 1_048_576,
    evidence: documented(ARK_MODEL_LIST_260915),
};

const AGENT_PLAN_QUALIFICATION: CapabilityEvidence = CapabilityEvidence::Qualified {
    evidence: "Authorized qualification of doubao-seed-2-1-pro-260915 on 2026-10-14: \
               docs/research/agent-plan-pinned-version-qualification-2026-10-14.md",
};

/// The entries of the dated version `doubao-seed-2-1-pro-260915` after its runtime qualification,
/// with the function calling evidence `function_calling`. The route has no abort operation, so
/// the abort entry stays unknown (ADR 0057).
pub(crate) const fn agent_plan_entries(function_calling: CapabilityEvidence) -> CapabilityEntries {
    CapabilityEntries {
        transport: AGENT_PLAN_QUALIFICATION,
        native_text: AGENT_PLAN_QUALIFICATION,
        structured_text: documented(ARK_CREATE_RESPONSE),
        continuation: AGENT_PLAN_QUALIFICATION,
        streaming: AGENT_PLAN_QUALIFICATION,
        implicit_cache: documented(ARK_CONTEXT_CACHE),
        explicit_cache: documented(ARK_CONTEXT_CACHE),
        native_compaction: CapabilityEvidence::Unknown,
        host_compaction: AGENT_PLAN_QUALIFICATION,
        retrieval: AGENT_PLAN_QUALIFICATION,
        abort: CapabilityEvidence::Unknown,
        function_calling,
    }
}

/// The context window of the dated version. It keeps its documented evidence.
pub(crate) const AGENT_PLAN_CONTEXT_WINDOW: Option<ContextWindow> = Some(AGENT_PLAN_WINDOW);

/// The combinations of the dated version after its runtime qualification.
pub(crate) const AGENT_PLAN_COMBINATIONS: &[CapabilityCombination] = &[
    CapabilityCombination {
        members: RECOVERY_REQUIREMENT,
        evidence: AGENT_PLAN_QUALIFICATION,
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
];

/// The profile of the current Agent Plan Registration: the qualified dated version with qualified
/// function calling.
pub const AGENT_PLAN_CAPABILITY_PROFILE: ModelCapabilityProfile = ModelCapabilityProfile {
    revision: "storyos.volcengine-agent-plan.doubao-seed-2-1-pro-260915.capability.v4",
    entries: agent_plan_entries(CapabilityEvidence::Qualified {
        evidence: "Authorized function calling probe of doubao-seed-2-1-pro-260915 on \
                   2026-11-03: docs/research/agent-plan-function-calling-probe-2026-11-03.md",
    }),
    context_window: AGENT_PLAN_CONTEXT_WINDOW,
    combinations: AGENT_PLAN_COMBINATIONS,
};

/// The known Registration of a stored revision. `None` is an unknown or damaged revision. Only
/// the current Registrations and the probe fixture are known (ADR 0097).
pub fn model_registration(revision: &str) -> Option<&'static ModelRegistration> {
    // The probe fixture is in the same list for each build. No deployment path binds it.
    [
        &crate::HOST_FAKE_TOOL_LOOP_REGISTRATION,
        &crate::AGENT_PLAN_REGISTRATION,
        &crate::PROBE_FIXTURE_REGISTRATION,
    ]
    .into_iter()
    .find(|registration| registration.revision == revision)
}

#[cfg(test)]
#[path = "model_registration_tests.rs"]
mod tests;
