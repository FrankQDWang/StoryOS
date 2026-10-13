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

/// The wire mapping revision of the Agent Plan Responses Create request.
pub const AGENT_PLAN_MAPPING_REVISION: &str = "storyos.volcengine-agent-plan.responses.create.v1";

/// The Agent Plan mapping that also sends the prior response reference of the Conversation.
pub const AGENT_PLAN_CONTINUATION_MAPPING_REVISION: &str =
    "storyos.volcengine-agent-plan.responses.create.v2";

/// The continuation mapping that also streams the response as typed server-sent events.
pub const AGENT_PLAN_STREAMING_MAPPING_REVISION: &str =
    "storyos.volcengine-agent-plan.responses.create.v3";

/// The streaming mapping that also sends a Host-managed compaction summary request, and then the
/// decision request with the summary and a new transport.
pub const AGENT_PLAN_COMPACTION_MAPPING_REVISION: &str =
    "storyos.volcengine-agent-plan.responses.create.v4";

/// One host-owned global Model Registration revision. It holds no Project data, endpoint
/// account, Credential Reference, grant, or compatibility admission.
#[derive(Debug, Eq, PartialEq)]
pub struct ModelRegistration {
    pub revision: &'static str,
    pub adapter: ModelAdapter,
    pub api_surface: &'static str,
    /// The exact provider model identifier. It is never an alias such as "latest".
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

    /// Whether the Host may read a stored response of an unknown create: the profile qualifies
    /// retrieval, or an authorized runtime qualification examines a route that needs it.
    pub fn retrieves(&self, qualification: crate::RuntimeQualification) -> bool {
        self.capability_profile
            .qualifies(&[ModelCapability::Retrieval])
            || (qualification == crate::RuntimeQualification::QualificationAuthorized
                && self.requirement.contains(&ModelCapability::Retrieval))
    }
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

/// The capabilities that one Create request needs together.
pub const CREATE_REQUIREMENT: &[ModelCapability] =
    &[ModelCapability::Transport, ModelCapability::NativeText];

/// The capabilities that one Create request with a prior response reference needs together.
pub const CONTINUATION_REQUIREMENT: &[ModelCapability] = &[
    ModelCapability::Transport,
    ModelCapability::NativeText,
    ModelCapability::Continuation,
];

/// The capabilities that one streamed Create request with a prior response reference needs.
pub const STREAMING_REQUIREMENT: &[ModelCapability] = &[
    ModelCapability::Transport,
    ModelCapability::NativeText,
    ModelCapability::Continuation,
    ModelCapability::Streaming,
];

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
const ARK_MODEL_LIST: &str = "Ark model list, docs.volcengine.com/docs/82379/1799865: a 256K context \
     window that input and output share";
const ARK_RETRIEVE_RESPONSE: &str =
    "General Ark Query Response, docs.volcengine.com/docs/82379/1783709, not the Agent Plan route";

/// The first profile, before the runtime qualification. Each runtime entry is pending.
pub const AGENT_PLAN_PENDING_CAPABILITY_PROFILE: ModelCapabilityProfile = ModelCapabilityProfile {
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
    context_window: None,
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

const AGENT_PLAN_RUNTIME_QUALIFICATION: CapabilityEvidence = CapabilityEvidence::Qualified {
    evidence: "Authorized runtime qualification on 2026-10-11, two admitted live requests: \
               docs/research/agent-plan-runtime-qualification-2026-10-11.md",
};

/// The profile after the runtime qualification. Only transport, native text, and their Create
/// combination are qualified. Each other entry keeps its earlier evidence.
pub const AGENT_PLAN_CREATE_CAPABILITY_PROFILE: ModelCapabilityProfile = ModelCapabilityProfile {
    revision: "storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v2",
    entries: CapabilityEntries {
        transport: AGENT_PLAN_RUNTIME_QUALIFICATION,
        native_text: AGENT_PLAN_RUNTIME_QUALIFICATION,
        ..AGENT_PLAN_PENDING_CAPABILITY_PROFILE.entries
    },
    context_window: None,
    combinations: &[
        CapabilityCombination {
            members: CREATE_REQUIREMENT,
            evidence: AGENT_PLAN_RUNTIME_QUALIFICATION,
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

/// The continuation profile before its runtime qualification. The Create combination keeps its
/// qualification, and the combination with continuation is pending.
pub const AGENT_PLAN_CONTINUATION_PENDING_CAPABILITY_PROFILE: ModelCapabilityProfile =
    ModelCapabilityProfile {
        revision: "storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v3",
        entries: AGENT_PLAN_CREATE_CAPABILITY_PROFILE.entries,
        context_window: None,
        combinations: &[
            CapabilityCombination {
                members: CREATE_REQUIREMENT,
                evidence: AGENT_PLAN_RUNTIME_QUALIFICATION,
            },
            CapabilityCombination {
                members: CONTINUATION_REQUIREMENT,
                evidence: documented(ARK_CREATE_RESPONSE),
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

const AGENT_PLAN_CONTINUATION_QUALIFICATION: CapabilityEvidence = CapabilityEvidence::Qualified {
    evidence: "Authorized continuation qualification on 2026-10-12, three admitted live requests: \
               docs/research/agent-plan-continuation-qualification-2026-10-12.md",
};

/// The profile after the continuation qualification. Transport, native text, continuation, and
/// their combination are qualified. Each other entry keeps its earlier evidence.
pub const AGENT_PLAN_CONTINUATION_CAPABILITY_PROFILE: ModelCapabilityProfile =
    ModelCapabilityProfile {
        revision: "storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v4",
        entries: CapabilityEntries {
            continuation: AGENT_PLAN_CONTINUATION_QUALIFICATION,
            ..AGENT_PLAN_CREATE_CAPABILITY_PROFILE.entries
        },
        context_window: None,
        combinations: &[
            CapabilityCombination {
                members: CONTINUATION_REQUIREMENT,
                evidence: AGENT_PLAN_CONTINUATION_QUALIFICATION,
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

/// The streaming profile before its runtime qualification. The continuation combination keeps
/// its qualification, and the combination with streaming is pending.
pub const AGENT_PLAN_STREAMING_PENDING_CAPABILITY_PROFILE: ModelCapabilityProfile =
    ModelCapabilityProfile {
        revision: "storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v5",
        entries: AGENT_PLAN_CONTINUATION_CAPABILITY_PROFILE.entries,
        context_window: None,
        combinations: &[
            CapabilityCombination {
                members: CONTINUATION_REQUIREMENT,
                evidence: AGENT_PLAN_CONTINUATION_QUALIFICATION,
            },
            CapabilityCombination {
                members: STREAMING_REQUIREMENT,
                evidence: documented(ARK_RESPONSE_LIFECYCLE),
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

const AGENT_PLAN_STREAMING_QUALIFICATION: CapabilityEvidence = CapabilityEvidence::Qualified {
    evidence: "Authorized streaming qualification on 2026-10-12, three admitted live requests: \
               docs/research/agent-plan-streaming-qualification-2026-10-12.md",
};

/// The profile after the streaming qualification. Transport, native text, continuation,
/// streaming, and their combination are qualified. Each other entry keeps its earlier evidence.
pub const AGENT_PLAN_STREAMING_CAPABILITY_PROFILE: ModelCapabilityProfile =
    ModelCapabilityProfile {
        revision: "storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v6",
        entries: CapabilityEntries {
            streaming: AGENT_PLAN_STREAMING_QUALIFICATION,
            ..AGENT_PLAN_CONTINUATION_CAPABILITY_PROFILE.entries
        },
        context_window: None,
        combinations: &[
            CapabilityCombination {
                members: STREAMING_REQUIREMENT,
                evidence: AGENT_PLAN_STREAMING_QUALIFICATION,
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

/// The compaction profile before its runtime qualification. The streaming combination keeps its
/// qualification, and the combination with Host-managed compaction is pending.
pub const AGENT_PLAN_COMPACTION_PENDING_CAPABILITY_PROFILE: ModelCapabilityProfile =
    ModelCapabilityProfile {
        revision: "storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v7",
        entries: AGENT_PLAN_STREAMING_CAPABILITY_PROFILE.entries,
        context_window: None,
        combinations: &[
            CapabilityCombination {
                members: STREAMING_REQUIREMENT,
                evidence: AGENT_PLAN_STREAMING_QUALIFICATION,
            },
            CapabilityCombination {
                members: COMPACTION_REQUIREMENT,
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

const AGENT_PLAN_COMPACTION_QUALIFICATION: CapabilityEvidence = CapabilityEvidence::Qualified {
    evidence: "Authorized compaction qualification on 2026-10-12, three admitted live requests: \
               docs/research/agent-plan-compaction-qualification-2026-10-12.md",
};

/// The profile after the compaction qualification. Host-managed compaction and its combination
/// with transport, native text, continuation, and streaming are qualified. Each other entry
/// keeps its earlier evidence.
pub const AGENT_PLAN_COMPACTION_CAPABILITY_PROFILE: ModelCapabilityProfile =
    ModelCapabilityProfile {
        revision: "storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v8",
        entries: CapabilityEntries {
            host_compaction: AGENT_PLAN_COMPACTION_QUALIFICATION,
            ..AGENT_PLAN_STREAMING_CAPABILITY_PROFILE.entries
        },
        context_window: None,
        combinations: &[
            CapabilityCombination {
                members: COMPACTION_REQUIREMENT,
                evidence: AGENT_PLAN_COMPACTION_QUALIFICATION,
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

/// The documented context window of `doubao-seed-2.1-pro`: 262,144 tokens.
const AGENT_PLAN_DOCUMENTED_WINDOW: ContextWindow = ContextWindow {
    tokens: 262_144,
    evidence: documented(ARK_MODEL_LIST),
};

/// The profile with the documented window, before the qualification of the automatic trigger.
/// The streaming combination keeps its qualification, and the compaction combination is pending.
pub const AGENT_PLAN_WINDOW_PENDING_CAPABILITY_PROFILE: ModelCapabilityProfile =
    ModelCapabilityProfile {
        revision: "storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v9",
        entries: AGENT_PLAN_COMPACTION_CAPABILITY_PROFILE.entries,
        context_window: Some(AGENT_PLAN_DOCUMENTED_WINDOW),
        combinations: &[
            CapabilityCombination {
                members: STREAMING_REQUIREMENT,
                evidence: AGENT_PLAN_STREAMING_QUALIFICATION,
            },
            CapabilityCombination {
                members: COMPACTION_REQUIREMENT,
                evidence: documented(ARK_MODEL_LIST),
            },
        ],
    };

const AGENT_PLAN_THRESHOLD_QUALIFICATION: CapabilityEvidence = CapabilityEvidence::Qualified {
    evidence: "Authorized qualification of the automatic compaction trigger on 2026-10-13: \
               docs/research/agent-plan-compaction-threshold-qualification-2026-10-13.md",
};

/// The profile after the qualification of the automatic trigger. A summary request near the
/// threshold qualified the trigger. The window and each other entry keep their earlier evidence.
pub const AGENT_PLAN_THRESHOLD_CAPABILITY_PROFILE: ModelCapabilityProfile =
    ModelCapabilityProfile {
        revision: "storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v10",
        entries: CapabilityEntries {
            host_compaction: AGENT_PLAN_THRESHOLD_QUALIFICATION,
            ..AGENT_PLAN_COMPACTION_CAPABILITY_PROFILE.entries
        },
        // The qualification examined approximately 190,000 input tokens, not the full window, so
        // the window keeps its documented evidence.
        context_window: Some(AGENT_PLAN_DOCUMENTED_WINDOW),
        combinations: &[
            CapabilityCombination {
                members: COMPACTION_REQUIREMENT,
                evidence: AGENT_PLAN_THRESHOLD_QUALIFICATION,
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

/// The profile with the retrieval of a stored response pending. Each other entry and the
/// compaction combination keep their qualification.
pub const AGENT_PLAN_RECOVERY_PENDING_CAPABILITY_PROFILE: ModelCapabilityProfile =
    ModelCapabilityProfile {
        revision: "storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v11",
        entries: AGENT_PLAN_THRESHOLD_CAPABILITY_PROFILE.entries,
        context_window: Some(AGENT_PLAN_DOCUMENTED_WINDOW),
        combinations: &[
            CapabilityCombination {
                members: COMPACTION_REQUIREMENT,
                evidence: AGENT_PLAN_THRESHOLD_QUALIFICATION,
            },
            CapabilityCombination {
                members: RECOVERY_REQUIREMENT,
                evidence: documented(ARK_RETRIEVE_RESPONSE),
            },
        ],
    };

const AGENT_PLAN_RECOVERY_QUALIFICATION: CapabilityEvidence = CapabilityEvidence::Qualified {
    evidence: "Authorized qualification of the retrieval of a stored response on 2026-10-13: \
               docs/research/agent-plan-recovery-qualification-2026-10-13.md",
};

/// The profile after the qualification of retrieval. The route has no abort operation, so the
/// abort entry stays unknown (ADR 0057).
pub const AGENT_PLAN_CAPABILITY_PROFILE: ModelCapabilityProfile = ModelCapabilityProfile {
    revision: "storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v12",
    entries: CapabilityEntries {
        retrieval: AGENT_PLAN_RECOVERY_QUALIFICATION,
        ..AGENT_PLAN_THRESHOLD_CAPABILITY_PROFILE.entries
    },
    context_window: Some(AGENT_PLAN_DOCUMENTED_WINDOW),
    combinations: &[
        CapabilityCombination {
            members: RECOVERY_REQUIREMENT,
            evidence: AGENT_PLAN_RECOVERY_QUALIFICATION,
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
    mapping_revision: crate::HOST_FAKE_MAPPING_REVISION,
    requirement: COMPACTION_REQUIREMENT,
    capability_profile: &HOST_FAKE_CAPABILITY_PROFILE,
};

/// The first Agent Plan Registration, with the pending profile. An AgentRun that pinned it
/// refuses with Registration drift.
pub const AGENT_PLAN_PENDING_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa02",
    adapter: ModelAdapter::VolcengineAgentPlanResponses,
    api_surface: "volcengine.agent-plan.responses./api/plan/v3",
    provider_model_id: "doubao-seed-2.1-pro",
    mapping_revision: AGENT_PLAN_MAPPING_REVISION,
    requirement: CREATE_REQUIREMENT,
    capability_profile: &AGENT_PLAN_PENDING_CAPABILITY_PROFILE,
};

/// The Agent Plan Registration with the qualified Create profile and no continuation.
pub const AGENT_PLAN_CREATE_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa03",
    capability_profile: &AGENT_PLAN_CREATE_CAPABILITY_PROFILE,
    ..AGENT_PLAN_PENDING_REGISTRATION
};

/// The continuation mapping with its pending profile. An AgentRun that pinned it refuses with
/// Registration drift.
pub const AGENT_PLAN_CONTINUATION_PENDING_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa04",
    mapping_revision: AGENT_PLAN_CONTINUATION_MAPPING_REVISION,
    requirement: CONTINUATION_REQUIREMENT,
    capability_profile: &AGENT_PLAN_CONTINUATION_PENDING_CAPABILITY_PROFILE,
    ..AGENT_PLAN_PENDING_REGISTRATION
};

/// The continuation mapping with the qualified profile and no streaming.
pub const AGENT_PLAN_CONTINUATION_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa05",
    capability_profile: &AGENT_PLAN_CONTINUATION_CAPABILITY_PROFILE,
    ..AGENT_PLAN_CONTINUATION_PENDING_REGISTRATION
};

/// The streaming mapping with its pending profile. An AgentRun that pinned it refuses with
/// Registration drift.
pub const AGENT_PLAN_STREAMING_PENDING_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa06",
    mapping_revision: AGENT_PLAN_STREAMING_MAPPING_REVISION,
    requirement: STREAMING_REQUIREMENT,
    capability_profile: &AGENT_PLAN_STREAMING_PENDING_CAPABILITY_PROFILE,
    ..AGENT_PLAN_PENDING_REGISTRATION
};

/// The Agent Plan Registration with the streaming mapping and the qualified profile.
pub const AGENT_PLAN_STREAMING_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa07",
    capability_profile: &AGENT_PLAN_STREAMING_CAPABILITY_PROFILE,
    ..AGENT_PLAN_STREAMING_PENDING_REGISTRATION
};

/// The compaction mapping with its pending profile. An AgentRun that pinned it refuses with
/// Registration drift.
pub const AGENT_PLAN_COMPACTION_PENDING_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa08",
    mapping_revision: AGENT_PLAN_COMPACTION_MAPPING_REVISION,
    requirement: COMPACTION_REQUIREMENT,
    capability_profile: &AGENT_PLAN_COMPACTION_PENDING_CAPABILITY_PROFILE,
    ..AGENT_PLAN_STREAMING_REGISTRATION
};

/// The compaction mapping with the qualified profile and no window. It does not compact by itself.
pub const AGENT_PLAN_COMPACTION_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa09",
    capability_profile: &AGENT_PLAN_COMPACTION_CAPABILITY_PROFILE,
    ..AGENT_PLAN_COMPACTION_PENDING_REGISTRATION
};

/// The compaction mapping with the documented window and the pending automatic trigger. An
/// AgentRun that pinned it refuses with Registration drift.
pub const AGENT_PLAN_WINDOW_PENDING_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa10",
    capability_profile: &AGENT_PLAN_WINDOW_PENDING_CAPABILITY_PROFILE,
    ..AGENT_PLAN_COMPACTION_REGISTRATION
};

/// The compaction mapping with the documented window and the qualified automatic trigger.
pub const AGENT_PLAN_THRESHOLD_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa11",
    capability_profile: &AGENT_PLAN_THRESHOLD_CAPABILITY_PROFILE,
    ..AGENT_PLAN_COMPACTION_REGISTRATION
};

/// The same mapping that also retrieves the stored response of an unknown create, with the
/// pending profile. An AgentRun that pinned it refuses with Registration drift.
pub const AGENT_PLAN_RECOVERY_PENDING_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa12",
    requirement: RECOVERY_REQUIREMENT,
    capability_profile: &AGENT_PLAN_RECOVERY_PENDING_CAPABILITY_PROFILE,
    ..AGENT_PLAN_THRESHOLD_REGISTRATION
};

/// The current Agent Plan Registration: the compaction mapping with qualified retrieval.
pub const AGENT_PLAN_REGISTRATION: ModelRegistration = ModelRegistration {
    revision: "018f0000-0000-7001-8000-00000000fa13",
    capability_profile: &AGENT_PLAN_CAPABILITY_PROFILE,
    ..AGENT_PLAN_RECOVERY_PENDING_REGISTRATION
};

/// The known Registration of a stored revision. `None` is an unknown or damaged revision.
pub fn model_registration(revision: &str) -> Option<&'static ModelRegistration> {
    [
        &HOST_FAKE_REGISTRATION,
        &AGENT_PLAN_PENDING_REGISTRATION,
        &AGENT_PLAN_CREATE_REGISTRATION,
        &AGENT_PLAN_CONTINUATION_PENDING_REGISTRATION,
        &AGENT_PLAN_CONTINUATION_REGISTRATION,
        &AGENT_PLAN_STREAMING_PENDING_REGISTRATION,
        &AGENT_PLAN_STREAMING_REGISTRATION,
        &AGENT_PLAN_COMPACTION_PENDING_REGISTRATION,
        &AGENT_PLAN_COMPACTION_REGISTRATION,
        &AGENT_PLAN_WINDOW_PENDING_REGISTRATION,
        &AGENT_PLAN_THRESHOLD_REGISTRATION,
        &AGENT_PLAN_RECOVERY_PENDING_REGISTRATION,
        &AGENT_PLAN_REGISTRATION,
    ]
    .into_iter()
    .find(|registration| registration.revision == revision)
}

#[cfg(test)]
#[path = "model_registration_tests.rs"]
mod tests;
