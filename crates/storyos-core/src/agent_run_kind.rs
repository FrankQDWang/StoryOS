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
    pub const DISPATCHED: [Self; 1] = [Self::Author];

    /// The persisted name of the kind.
    pub fn name(self) -> &'static str {
        match self {
            Self::Author => "author",
            Self::MemoryCapture => "memory_capture",
            Self::MemoryConsolidation => "memory_consolidation",
        }
    }
}

#[cfg(test)]
#[path = "agent_run_kind_tests.rs"]
mod tests;
