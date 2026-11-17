//! The kind of an AgentRun (ADR 0094 section 5).

/// The kind of one AgentRun. A Run of the author has its Project Conversation, its Conversation
/// Memory Settings revision, its Chapter, its author message, and the Receipt of `createAgentRun`.
/// A background Run has none of them, and no read or command of the author gets it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRunKind {
    Author,
    /// One capture of a quiet Project Conversation or of a changed Chapter (ADR 0094 section 5).
    MemoryCapture,
    /// One consolidation pass of the Memory Observations (ADR 0094 section 6).
    MemoryConsolidation,
}

impl AgentRunKind {
    /// Each kind, in the order of the closed list of `agent_runs.run_kind`.
    pub const ALL: [Self; 3] = [Self::Author, Self::MemoryCapture, Self::MemoryConsolidation];

    /// The kinds that the Worker claims and dispatches. A queued Run of another kind stays queued.
    pub const DISPATCHED: [Self; 2] = [Self::Author, Self::MemoryCapture];

    /// The persisted name of the kind.
    pub fn name(self) -> &'static str {
        match self {
            Self::Author => "author",
            Self::MemoryCapture => "memory_capture",
            Self::MemoryConsolidation => "memory_consolidation",
        }
    }

    /// The kind of the persisted name `name`.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// Whether a Run of the kind has a Project Conversation and a Chapter. The Phase decisions
    /// below come from this fact (ADR 0094 section 5).
    fn has_conversation(self) -> bool {
        match self {
            Self::Author => true,
            Self::MemoryCapture | Self::MemoryConsolidation => false,
        }
    }

    /// Whether the decision request of a RunStep compacts first. Only a Run with a Conversation
    /// compacts, when the mapping of its pinned Model Registration compacts (ADR 0055, ADR 0093).
    pub fn compacts_before_decision(self, model_registration_revision: &str) -> bool {
        self.has_conversation()
            && crate::model_registration(model_registration_revision)
                .is_some_and(crate::ModelRegistration::compacts)
    }

    /// Whether the dispatch of the Run rebuilds an expired prior response. A background Run has
    /// no Conversation, so it has no prior response (ADR 0091).
    pub fn rebuilds_expired_reference(self) -> bool {
        self.has_conversation()
    }

    /// Whether the create request reads the work of the Chapter: the declared Proposal targets
    /// and the Tool qualification probe (ADR 0049). A background Run has no Chapter.
    pub fn reads_chapter_work(self) -> bool {
        self.has_conversation()
    }

    /// Whether the next RunStep reads the steering input and the Chapter of the Run. A background
    /// Run opens its next RunStep from its input and its Tool results.
    pub fn reads_steering(self) -> bool {
        self.has_conversation()
    }

    /// The `tool_choice` of each request of a Run of the kind, when the kind decides it. A
    /// background Run has no `finish` call, so each request requires a call, and its output call
    /// ends it. The finish rule decides the `tool_choice` of a Run of the author.
    pub fn fixed_tool_choice(self) -> Option<crate::ToolChoice> {
        match self {
            Self::Author => None,
            Self::MemoryCapture | Self::MemoryConsolidation => Some(crate::ToolChoice::Required),
        }
    }
}

#[cfg(test)]
#[path = "agent_run_kind_tests.rs"]
mod tests;
