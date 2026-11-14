//! Validate one complete model output as an Agent Decision without I/O.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamItemState {
    Provisional,
    Complete,
    Incomplete,
    Failed,
    Cancelled,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamItemRole {
    Assistant,
    Tool,
}

/// One native item of a Model Attempt stream, as the Model Provider Adapter reports it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStreamItem {
    pub item_id: String,
    pub role: StreamItemRole,
    pub state: StreamItemState,
    pub text: Option<String>,
    pub summary: Option<String>,
    pub call_id: Option<String>,
    pub arguments: Option<String>,
    pub refusal: Option<String>,
    /// The native type of a Tool item, kept as evidence on each path (ADR 0084). A request asks
    /// for no native item other than a message, a reasoning item, and the function call of a
    /// probe. `None` for an assistant item.
    pub native_type: Option<String>,
    /// The function name of a function call item.
    pub name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentDecisionKind {
    /// The final answer text of a Run (ADR 0091).
    Advisory { text: String },
    /// The complete validated function request batch, with its refused calls. Any text of the
    /// response is provisional assistant output, and the ToolCalls are the next work (ADR 0084).
    ToolRequest {
        calls: Vec<crate::RequestedToolCall>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentDecisionOutcome {
    NoDecision,
    Decision {
        kind: AgentDecisionKind,
        selected: bool,
        advances_continuation: bool,
    },
}

/// The Agent Decision of a validated function request batch with members (ADR 0084). Its
/// ordinary continuation waits for its ToolCalls. An invalid batch or a batch with no member
/// makes no Decision: the finish rule decides the outcome of a text (ADR 0091).
pub fn validate_agent_decision(batch: crate::ToolCallBatch) -> AgentDecisionOutcome {
    match batch {
        crate::ToolCallBatch::Valid(calls) if !calls.is_empty() => AgentDecisionOutcome::Decision {
            kind: AgentDecisionKind::ToolRequest { calls },
            selected: true,
            advances_continuation: false,
        },
        crate::ToolCallBatch::Valid(_) | crate::ToolCallBatch::Invalid(_) => {
            AgentDecisionOutcome::NoDecision
        }
    }
}
