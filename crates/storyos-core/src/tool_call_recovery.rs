//! The recovery of one interrupted ToolCall (S5-08, ADR 0086): the outcome class from the
//! retained records, the evidence of a result that arrives after the fence, and the request
//! reservation. A class never comes from process memory, and no class sends a ToolCall again.

use serde_json::{Value, json};

use crate::{
    EffectOutcome, ToolAttemptSettlement, ToolCallState, ToolEffectOutcome, ToolObservation,
    ToolResultText, ToolSpec, settle_tool_call,
};

/// The outcome class of one resolved ToolCall.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolCallClass {
    /// The Host refused or cancelled it before the dispatch claim, or the exchange proved that
    /// the request did not leave the process.
    NotAttempted,
    /// A terminal answer arrived and settled it before any fence.
    Confirmed,
    /// A terminal answer arrived after the fence. It is evidence only.
    Partial,
    /// The dispatch claim committed, and no terminal answer arrived.
    Unknown,
}

impl ToolCallClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotAttempted => "not_attempted",
            Self::Confirmed => "confirmed",
            Self::Partial => "partial",
            Self::Unknown => "unknown",
        }
    }

    /// The worst-case reservation of one request. Only a class that proves the usage releases
    /// it; an unknown or partial ToolCall keeps it.
    pub fn reservation(self) -> Value {
        let released = match self {
            Self::NotAttempted | Self::Confirmed => true,
            Self::Partial | Self::Unknown => false,
        };
        json!({"kind": "worst_case", "requests": 1, "released": released})
    }
}

/// The reservation that the dispatch claim records before any I/O.
pub fn claimed_tool_reservation() -> Value {
    json!({"kind": "worst_case", "requests": 1, "released": false})
}

/// Classifies one ToolCall from its state, the settlement of its Destination Attempt, and the
/// settlement of the observation that arrived after its fence. `None` is an unresolved ToolCall.
pub fn tool_call_class(
    state: ToolCallState,
    attempt: Option<ToolAttemptSettlement>,
    late: Option<ToolAttemptSettlement>,
) -> Option<ToolCallClass> {
    use ToolAttemptSettlement::{NativeError, NotSubmitted, OutcomeUnknown, TerminalResult};
    match state {
        ToolCallState::Pending | ToolCallState::Dispatched => None,
        ToolCallState::Refused | ToolCallState::AwaitingApproval => {
            Some(ToolCallClass::NotAttempted)
        }
        ToolCallState::Settled | ToolCallState::Failed | ToolCallState::Unknown => {
            Some(match (attempt, late) {
                (None | Some(NotSubmitted), _) => ToolCallClass::NotAttempted,
                (Some(TerminalResult | NativeError), _) => ToolCallClass::Confirmed,
                (Some(OutcomeUnknown), Some(TerminalResult | NativeError)) => {
                    ToolCallClass::Partial
                }
                (Some(OutcomeUnknown), None | Some(NotSubmitted | OutcomeUnknown)) => {
                    ToolCallClass::Unknown
                }
            })
        }
    }
}

/// What an observation that arrives after the fence records. It settles the disclosure and the
/// effect evidence only: it never gives a result to the Run, a Source Snapshot, or a Decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LateToolEvidence {
    pub attempt: ToolAttemptSettlement,
    /// An effect that the late observation does not confirm stays unknown: the fence already
    /// recorded that the request may have left.
    pub outcome: ToolEffectOutcome,
    /// The validated result text, kept as evidence.
    pub result: Option<ToolResultText>,
}

/// Validates the late observation against the result contract of the ToolSpec, as a settlement
/// before the fence does.
pub fn late_tool_evidence(spec: &ToolSpec, observation: &ToolObservation) -> LateToolEvidence {
    let settlement = settle_tool_call(spec, observation);
    let kept = |outcome| match outcome {
        EffectOutcome::Confirmed => EffectOutcome::Confirmed,
        EffectOutcome::NotAttempted | EffectOutcome::Unknown => EffectOutcome::Unknown,
    };
    LateToolEvidence {
        attempt: settlement.attempt,
        outcome: ToolEffectOutcome {
            external_read: kept(settlement.outcome.external_read),
            outbound_disclosure: kept(settlement.outcome.outbound_disclosure),
        },
        result: settlement.result,
    }
}

#[cfg(test)]
#[path = "tool_call_recovery_tests.rs"]
mod tests;
