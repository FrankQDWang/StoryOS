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
    Hosted,
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
    pub hosted_report: Option<String>,
}

/// Whether the destination offers a complete output as its final answer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputPhase {
    Commentary,
    FinalAnswer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecisionCandidate {
    Advisory { text: String },
    ProseChange { text: String },
    Clarification { question: String },
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

/// Select an Agent Decision only from a valid final answer; only a selected non-question advances.
pub fn validate_agent_decision(
    output: Option<&ModelOutput>,
    declared_targets: &[(String, String, String)],
) -> AgentDecisionOutcome {
    let Some(output) = output else {
        return AgentDecisionOutcome::NoDecision;
    };
    if let Some(changes) = &output.prose_changes
        && !crate::prose_changes_match_targets(declared_targets, changes)
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
