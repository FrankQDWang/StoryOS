//! The kind of an AgentRun (ADR 0094 section 5).

use crate::{RunStepLimit, ToolChoice, ToolOperation};

/// The most RunSteps of one capture. It is an initial value, and S7-D7 can change it. A Run of
/// the author keeps `MAX_RUN_STEPS` (ADR 0091 section 5).
pub const MEMORY_CAPTURE_MAX_RUN_STEPS: u64 = 8;

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
    /// One intent pass of a character (ADR 0101 section 4).
    IntentPass,
}

impl AgentRunKind {
    /// Each kind, in the order of the closed list of `agent_runs.run_kind`.
    pub const ALL: [Self; 4] = [
        Self::Author,
        Self::MemoryCapture,
        Self::MemoryConsolidation,
        Self::IntentPass,
    ];

    /// The kinds that the Worker claims and dispatches. A queued Run of another kind stays queued.
    pub const DISPATCHED: [Self; 4] = Self::ALL;

    /// The persisted name of the kind.
    pub fn name(self) -> &'static str {
        match self {
            Self::Author => "author",
            Self::MemoryCapture => "memory_capture",
            Self::MemoryConsolidation => "memory_consolidation",
            Self::IntentPass => "intent_pass",
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
            Self::MemoryCapture | Self::MemoryConsolidation | Self::IntentPass => false,
        }
    }

    /// Whether the decision request of a RunStep compacts first. Only a Run with a Conversation
    /// compacts, when the mapping of its pinned Model Registration compacts (ADR 0055, ADR 0093).
    pub fn compacts_before_decision(self, model_registration_revision: &str) -> bool {
        self.compacts_on(
            crate::model_registration(model_registration_revision)
                .map(|registration| registration.requirement),
        )
    }

    /// The gate of `compacts_before_decision` for the requirement of the Registration, or `None`
    /// for an unknown revision. Each Registration of the one list compacts now, but a later one
    /// can have a mapping that does not (ADR 0097).
    fn compacts_on(self, requirement: Option<&[crate::ModelCapability]>) -> bool {
        self.has_conversation()
            && requirement.is_some_and(crate::model_registration::requirement_compacts)
    }

    /// Whether the Memory Index and the reads of Memory of a Run of the kind give the current
    /// intent of each character. Only a Run of the author gets them (ADR 0101 section 5).
    pub fn reads_intents(self) -> bool {
        self == Self::Author
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
    /// background Run has no `finish` call and no closing request, so each request requires a
    /// call, and its output call ends it. The finish rule decides the `tool_choice` of a Run of
    /// the author (ADR 0091 section 4).
    pub fn fixed_tool_choice(self) -> Option<ToolChoice> {
        match self {
            Self::Author => None,
            Self::MemoryCapture | Self::MemoryConsolidation | Self::IntentPass => {
                Some(ToolChoice::Required)
            }
        }
    }

    /// The most RunSteps of a Run of the kind. A consolidation pass sends one request, and an
    /// intent pass sends one more request after a refused call.
    pub fn max_run_steps(self) -> u64 {
        match self {
            Self::Author => crate::MAX_RUN_STEPS,
            Self::MemoryCapture => MEMORY_CAPTURE_MAX_RUN_STEPS,
            Self::MemoryConsolidation => crate::MEMORY_CONSOLIDATION_MAX_RUN_STEPS,
            Self::IntentPass => crate::INTENT_PASS_MAX_RUN_STEPS,
        }
    }

    /// The limit of the RunStep at `decision_position` of a Run of the kind.
    pub fn run_step_limit(self, decision_position: u64) -> RunStepLimit {
        RunStepLimit::within(self.max_run_steps(), decision_position)
    }

    /// The one operation that the Host executes in the last RunStep of a Run of the kind: the
    /// output Tool of a background Run. The Host executes no call of the last RunStep of a Run of
    /// the author (ADR 0091 section 5).
    fn last_step_operation(self) -> Option<ToolOperation> {
        match self {
            Self::MemoryCapture => Some(ToolOperation::RecordMemoryObservations),
            Self::MemoryConsolidation => Some(ToolOperation::ApplyMemoryOperations),
            Self::IntentPass => Some(ToolOperation::RecordCharacterIntent),
            Self::Author => None,
        }
    }

    /// Whether the Host executes a call of `operation` in a RunStep at `limit`. Each other call
    /// of the last RunStep gets no execution.
    pub fn executes_call(self, limit: RunStepLimit, operation: ToolOperation) -> bool {
        limit.executes_calls() || self.last_step_operation() == Some(operation)
    }

    /// Whether a response of a RunStep at `limit` keeps its calls for the Tool Gateway.
    pub fn keeps_calls(self, limit: RunStepLimit) -> bool {
        limit.executes_calls() || self.last_step_operation().is_some()
    }

    /// Whether a Run of the kind gives its claim back between two RunSteps when a Run of the
    /// author is queued. Only background work gives way (ADR 0094 section 5).
    pub fn gives_way_to_author(self) -> bool {
        !self.has_conversation()
    }

    /// Whether the decision request with `record` is too long for a Run of the kind. A
    /// background Run does not compact. Each of its requests sends its full input and the Tool
    /// results of the Run. Thus the check of ADR 0056 counts each text of the request. The
    /// compaction phase checks a request of a Run of the author (ADR 0093).
    pub fn full_request_too_long(
        self,
        model_registration_revision: &str,
        budget_bounds: &str,
        record: &crate::CurrentPassageAssemblyRecord,
    ) -> bool {
        !self.has_conversation()
            && crate::later_request_too_long(
                model_registration_revision,
                budget_bounds,
                record,
                &[],
            )
    }
}

/// Whether a background Run between two RunSteps at `decision_position` gives its claim back. A
/// queued Run of the author goes first, so it waits for at most one request of background work.
/// The first RunStep follows the claim, and the claim order already puts the author first.
pub fn yields_to_author(decision_position: u64, author_run_queued: bool) -> bool {
    decision_position > 0 && author_run_queued
}

#[cfg(test)]
#[path = "agent_run_kind_tests.rs"]
mod tests;
