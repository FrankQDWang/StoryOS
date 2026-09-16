//! Classify one Contract-Faithful Fake Destination decision without I/O.

use crate::{canonical_json, hex_sha256};

pub const HOST_FAKE_EXECUTION_PROFILE: &str = "storyos.host-fake.execution.v1";
pub const HOST_FAKE_MAPPING_REVISION: &str = "storyos.host-fake.mapping.v1";
pub const ADVISORY_TEXT: &str =
    "This passage is inspectable Host-fake advice. It is not Authoritative State.";
pub const PROSE_CHANGE_TEXT: &str = "Guard the narrator voice in this passage.";
pub const CLARIFICATION_QUESTION: &str = "Which wording should stay in this sentence?";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionCapability {
    Tool,
    Mcp,
    Research,
    Embedding,
    Memory,
    Skill,
    Subrun,
    Eval,
}

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStreamItem {
    pub item_id: &'static str,
    pub role: StreamItemRole,
    pub state: StreamItemState,
    pub text: Option<&'static str>,
    pub summary: Option<&'static str>,
    pub call_id: Option<&'static str>,
    pub arguments: Option<&'static str>,
    pub refusal: Option<&'static str>,
    pub hosted_report: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoDecisionReason {
    Partial,
    Incomplete,
    Failed,
    Cancelled,
    Unknown,
    Invalid,
    PartialArguments,
    HostedEvidenceOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FakeDecisionKind {
    Advisory {
        text: &'static str,
    },
    ProseChange {
        text: &'static str,
        producer_input: &'static str,
    },
    Clarification {
        question: &'static str,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FakeAttemptOutcome {
    NoDecision {
        reason: NoDecisionReason,
    },
    Decision {
        kind: FakeDecisionKind,
        selected: bool,
        advances_continuation: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FakeDispatchPlan {
    RefuseWithoutDispatch {
        capability: ExecutionCapability,
    },
    Dispatch {
        items: Vec<NativeStreamItem>,
        outcome: FakeAttemptOutcome,
    },
}

/// Plan one fake-model result from the exact author message.
pub fn plan_fake_model_decision(author_message: &str) -> FakeDispatchPlan {
    if let Some(capability) = execution_capability(author_message) {
        return FakeDispatchPlan::RefuseWithoutDispatch { capability };
    }
    if let Some(scripted) = scripted_plan(author_message) {
        return scripted;
    }
    if author_message.starts_with("Revise this passage:")
        || author_message.starts_with("Tighten this paragraph")
    {
        return complete_decision(
            FakeDecisionKind::ProseChange {
                text: PROSE_CHANGE_TEXT,
                producer_input: PROSE_CHANGE_TEXT,
            },
            /*selected*/ true,
            /*advances_continuation*/ true,
        );
    }
    if author_message.starts_with("Which wording should I keep?") {
        return complete_decision(
            FakeDecisionKind::Clarification {
                question: CLARIFICATION_QUESTION,
            },
            /*selected*/ true,
            /*advances_continuation*/ false,
        );
    }
    complete_decision(
        FakeDecisionKind::Advisory {
            text: ADVISORY_TEXT,
        },
        /*selected*/ true,
        /*advances_continuation*/ true,
    )
}

/// Digest one non-secret Host-fake wire payload.
pub fn host_fake_wire_digest(author_message: &str, chapter_id: &str) -> String {
    format!(
        "sha256:{}",
        hex_sha256(
            canonical_json(&serde_json::json!({
                "author_message": author_message,
                "chapter_id": chapter_id,
                "mapping_revision": HOST_FAKE_MAPPING_REVISION,
            }))
            .as_bytes(),
        )
    )
}

fn complete_decision(
    kind: FakeDecisionKind,
    selected: bool,
    advances_continuation: bool,
) -> FakeDispatchPlan {
    let text = match kind {
        FakeDecisionKind::Advisory { text }
        | FakeDecisionKind::ProseChange { text, .. }
        | FakeDecisionKind::Clarification { question: text } => text,
    };
    FakeDispatchPlan::Dispatch {
        items: vec![assistant_item("1", StreamItemState::Complete, text)],
        outcome: FakeAttemptOutcome::Decision {
            kind,
            selected,
            advances_continuation,
        },
    }
}

fn assistant_item(
    item_id: &'static str,
    state: StreamItemState,
    text: &'static str,
) -> NativeStreamItem {
    NativeStreamItem {
        item_id,
        role: StreamItemRole::Assistant,
        state,
        text: Some(text),
        summary: Some("host_fake_native_text"),
        call_id: None,
        arguments: None,
        refusal: None,
        hosted_report: None,
    }
}

fn scripted_plan(author_message: &str) -> Option<FakeDispatchPlan> {
    let outcome = match author_message {
        "SCRIPT:partial" => NoDecisionReason::Partial,
        "SCRIPT:incomplete" => NoDecisionReason::Incomplete,
        "SCRIPT:failed" => NoDecisionReason::Failed,
        "SCRIPT:cancelled" => NoDecisionReason::Cancelled,
        "SCRIPT:unknown" => NoDecisionReason::Unknown,
        "SCRIPT:invalid" => NoDecisionReason::Invalid,
        "SCRIPT:unselected" => {
            return Some(complete_decision(
                FakeDecisionKind::Advisory {
                    text: ADVISORY_TEXT,
                },
                /*selected*/ false,
                /*advances_continuation*/ false,
            ));
        }
        "SCRIPT:tool_partial" => {
            return Some(FakeDispatchPlan::Dispatch {
                items: vec![NativeStreamItem {
                    item_id: "1",
                    role: StreamItemRole::Tool,
                    state: StreamItemState::Provisional,
                    text: None,
                    summary: Some("partial_tool_arguments"),
                    call_id: Some("call-1"),
                    arguments: Some("{\"q\""),
                    refusal: None,
                    hosted_report: None,
                }],
                outcome: FakeAttemptOutcome::NoDecision {
                    reason: NoDecisionReason::PartialArguments,
                },
            });
        }
        "SCRIPT:hosted" => {
            return Some(FakeDispatchPlan::Dispatch {
                items: vec![NativeStreamItem {
                    item_id: "1",
                    role: StreamItemRole::Hosted,
                    state: StreamItemState::Complete,
                    text: None,
                    summary: Some("hosted_item_evidence"),
                    call_id: None,
                    arguments: None,
                    refusal: None,
                    hosted_report: Some("host_fake_hosted_item"),
                }],
                outcome: FakeAttemptOutcome::NoDecision {
                    reason: NoDecisionReason::HostedEvidenceOnly,
                },
            });
        }
        _ => return None,
    };
    let state = match outcome {
        NoDecisionReason::Partial | NoDecisionReason::PartialArguments => {
            StreamItemState::Provisional
        }
        NoDecisionReason::Incomplete => StreamItemState::Incomplete,
        NoDecisionReason::Failed => StreamItemState::Failed,
        NoDecisionReason::Cancelled => StreamItemState::Cancelled,
        NoDecisionReason::Unknown => StreamItemState::Unknown,
        NoDecisionReason::Invalid | NoDecisionReason::HostedEvidenceOnly => {
            StreamItemState::Complete
        }
    };
    Some(FakeDispatchPlan::Dispatch {
        items: vec![assistant_item("1", state, ADVISORY_TEXT)],
        outcome: FakeAttemptOutcome::NoDecision { reason: outcome },
    })
}

fn execution_capability(author_message: &str) -> Option<ExecutionCapability> {
    let lowered = author_message.to_ascii_lowercase();
    [
        ("invoke a tool", ExecutionCapability::Tool),
        ("open an mcp", ExecutionCapability::Mcp),
        ("run a research fetch", ExecutionCapability::Research),
        ("embed this passage", ExecutionCapability::Embedding),
        ("extract conversation memory", ExecutionCapability::Memory),
        ("load a skill", ExecutionCapability::Skill),
        ("start a subrun", ExecutionCapability::Subrun),
        ("run an eval", ExecutionCapability::Eval),
    ]
    .into_iter()
    .find_map(|(needle, capability)| lowered.contains(needle).then_some(capability))
}

#[cfg(test)]
#[path = "complete_fake_decision_tests.rs"]
mod tests;
