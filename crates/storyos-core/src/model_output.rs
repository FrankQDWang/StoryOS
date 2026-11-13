//! Validate one complete model output as an Agent Decision without I/O.

use crate::ProseChangeCandidate;

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

/// Whether the destination offers a complete output as its final answer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputPhase {
    Commentary,
    FinalAnswer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecisionCandidate {
    Advisory {
        text: String,
    },
    ProseChange {
        text: String,
    },
    Clarification {
        question: String,
    },
    ResearchSynthesis {
        summary: String,
        claims: Vec<crate::ClaimCandidate>,
    },
}

/// The structured part of one complete model output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelOutput {
    pub phase: OutputPhase,
    pub candidate: DecisionCandidate,
    pub prose_changes: Option<Vec<ProseChangeCandidate>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentDecisionKind {
    Advisory {
        text: String,
    },
    ProseChange {
        text: String,
        locations: Option<Vec<ProseChangeCandidate>>,
    },
    Clarification {
        question: String,
    },
    /// The complete validated function request batch, with its refused calls. Any text of the
    /// response is provisional assistant output, and the ToolCalls are the next work (ADR 0084).
    ToolRequest {
        calls: Vec<crate::RequestedToolCall>,
    },
    /// A research synthesis whose citations each name a source that this AgentRun captured.
    ResearchSynthesis(crate::ResearchSynthesis),
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

/// Select an Agent Decision only from a valid final answer and a valid batch; only a selected
/// non-question without a ToolCall advances. An invalid batch makes no Decision. A valid batch
/// with members is the Decision, and the ordinary continuation waits for its ToolCalls. A research
/// synthesis is valid only when each of its citations names a source in `citable`. A text with
/// U+0000 makes no Decision.
pub fn validate_agent_decision(
    output: Option<&ModelOutput>,
    declared_targets: &[crate::DeclaredLocation],
    citable: &crate::CitableSources,
    batch: crate::ToolCallBatch,
) -> AgentDecisionOutcome {
    let calls = match batch {
        crate::ToolCallBatch::Invalid(_) => return AgentDecisionOutcome::NoDecision,
        crate::ToolCallBatch::Valid(calls) => calls,
    };
    if let Some(output) = output
        && let Some(changes) = &output.prose_changes
        && !crate::prose_changes_match_targets(declared_targets, changes)
    {
        return AgentDecisionOutcome::NoDecision;
    }
    if !calls.is_empty() {
        return AgentDecisionOutcome::Decision {
            kind: AgentDecisionKind::ToolRequest { calls },
            selected: true,
            advances_continuation: false,
        };
    }
    let Some(output) = output else {
        return AgentDecisionOutcome::NoDecision;
    };
    // PostgreSQL refuses U+0000 in `text` and `jsonb`; a research synthesis refuses it itself.
    let candidate_text = match &output.candidate {
        DecisionCandidate::Advisory { text } | DecisionCandidate::ProseChange { text } => text,
        DecisionCandidate::Clarification { question } => question,
        DecisionCandidate::ResearchSynthesis { .. } => "",
    };
    if candidate_text.contains('\0')
        || output
            .prose_changes
            .iter()
            .flatten()
            .any(|change| change.candidate_text.contains('\0') || change.explanation.contains('\0'))
    {
        return AgentDecisionOutcome::NoDecision;
    }
    let kind = match &output.candidate {
        DecisionCandidate::Advisory { text } => AgentDecisionKind::Advisory { text: text.clone() },
        DecisionCandidate::ProseChange { text } => AgentDecisionKind::ProseChange {
            text: text.clone(),
            locations: output.prose_changes.clone(),
        },
        DecisionCandidate::Clarification { question } => AgentDecisionKind::Clarification {
            question: question.clone(),
        },
        DecisionCandidate::ResearchSynthesis { summary, claims } => {
            match crate::validate_research_synthesis(summary, claims, citable) {
                Ok(synthesis) => AgentDecisionKind::ResearchSynthesis(synthesis),
                Err(_) => return AgentDecisionOutcome::NoDecision,
            }
        }
    };
    let selected = matches!(output.phase, OutputPhase::FinalAnswer);
    let advances_continuation =
        selected && !matches!(kind, AgentDecisionKind::Clarification { .. });
    AgentDecisionOutcome::Decision {
        kind,
        selected,
        advances_continuation,
    }
}

#[cfg(test)]
#[path = "model_output_tests.rs"]
mod tests;
