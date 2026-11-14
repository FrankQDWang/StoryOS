//! Derive one scripted Create result from the exact request, with no process memory.

use storyos_application::CreateRequest;
use storyos_core::DeclaredTargetKind;
use storyos_core::{
    ADVISORY_TEXT, NativeStreamItem, OrdinaryPassageResolution, PASSAGE_REFERENCE_QUESTION,
    PROSE_CHANGE_TEXT, StreamItemRole, StreamItemState,
};

use crate::prose_changes::is_fake_candidate_revision_request;

const CLARIFICATION_QUESTION: &str = "Which wording should stay in this sentence?";

/// The native items of one fake Create exchange.
pub(crate) struct FakeCreate {
    pub items: Vec<NativeStreamItem>,
}

/// What the fake model does for one author message.
pub(crate) enum Intent {
    Advice,
    /// A prose change with this summary.
    ProseChange(String),
    /// A question to the author, which is a final answer text (ADR 0091).
    Question(String),
}

fn message_intent(author_message: &str, resolution: Option<OrdinaryPassageResolution>) -> Intent {
    match resolution {
        Some(OrdinaryPassageResolution::Resolved) => {
            return Intent::ProseChange(PROSE_CHANGE_TEXT.to_owned());
        }
        Some(OrdinaryPassageResolution::Clarification) => {
            return Intent::Question(PASSAGE_REFERENCE_QUESTION.to_owned());
        }
        None => {}
    }
    if author_message.starts_with("Revise this passage:")
        || author_message.starts_with("Revise these passages")
        || author_message.starts_with("Tighten this paragraph")
    {
        Intent::ProseChange(PROSE_CHANGE_TEXT.to_owned())
    } else if author_message.starts_with("Which wording should I keep?") {
        Intent::Question(CLARIFICATION_QUESTION.to_owned())
    } else {
        Intent::Advice
    }
}

/// The intent of `request`, and whether its changes come from the fake change script. The tool
/// loop fake calls `propose_edit` for a prose change.
pub(crate) fn intent(request: &CreateRequest) -> (Intent, bool) {
    let author_message = request.author_message.as_str();
    let mut intent = message_intent(author_message, request.passage_resolution);
    let candidate = request
        .declared_targets
        .iter()
        .any(|target| target.kind == DeclaredTargetKind::PendingCandidate);
    if candidate
        && is_fake_candidate_revision_request(author_message)
        && matches!(intent, Intent::Advice)
    {
        intent = Intent::ProseChange(ADVISORY_TEXT.to_owned());
    }
    let prose_change = matches!(intent, Intent::ProseChange(_));
    let produce = if candidate {
        prose_change
    } else {
        author_message.starts_with("Revise these passages")
            || (request
                .declared_targets
                .first()
                .is_some_and(|target| target.collection)
                && prose_change)
    };
    (intent, produce)
}

/// One complete assistant text.
pub(crate) fn complete(text: &str) -> FakeCreate {
    FakeCreate {
        items: vec![assistant_item("1", StreamItemState::Complete, text)],
    }
}

pub(crate) fn assistant_item(
    item_id: &str,
    state: StreamItemState,
    text: &str,
) -> NativeStreamItem {
    NativeStreamItem {
        item_id: item_id.to_owned(),
        role: StreamItemRole::Assistant,
        state,
        text: Some(text.to_owned()),
        summary: Some("host_fake_native_text".to_owned()),
        call_id: None,
        arguments: None,
        refusal: None,
        native_type: None,
        name: None,
    }
}

pub(crate) fn evidence_item(
    role: StreamItemRole,
    state: StreamItemState,
    summary: &str,
) -> NativeStreamItem {
    NativeStreamItem {
        item_id: "1".to_owned(),
        role,
        state,
        text: None,
        summary: Some(summary.to_owned()),
        call_id: None,
        arguments: None,
        refusal: None,
        native_type: None,
        name: None,
    }
}

/// The scripted outcome of a model response: a stream state, a partial tool call, or a
/// refusal.
pub(crate) fn response_outcome(author_message: &str) -> Option<FakeCreate> {
    let state = match author_message {
        "SCRIPT:partial" => StreamItemState::Provisional,
        "SCRIPT:incomplete" => StreamItemState::Incomplete,
        "SCRIPT:failed" => StreamItemState::Failed,
        "SCRIPT:cancelled" => StreamItemState::Cancelled,
        "SCRIPT:unknown" => StreamItemState::Unknown,
        "SCRIPT:tool_partial" => {
            return Some(FakeCreate {
                items: vec![NativeStreamItem {
                    call_id: Some("call-1".to_owned()),
                    arguments: Some("{\"q\"".to_owned()),
                    ..evidence_item(
                        StreamItemRole::Tool,
                        StreamItemState::Provisional,
                        "partial_tool_arguments",
                    )
                }],
            });
        }
        "SCRIPT:refusal" => {
            return Some(FakeCreate {
                items: vec![NativeStreamItem {
                    refusal: Some("host_fake_refusal".to_owned()),
                    ..evidence_item(
                        StreamItemRole::Assistant,
                        StreamItemState::Complete,
                        "host_fake_refusal",
                    )
                }],
            });
        }
        _ => return None,
    };
    Some(FakeCreate {
        items: vec![assistant_item("1", state, ADVISORY_TEXT)],
    })
}
